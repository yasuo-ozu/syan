# `Dup` / `&mut` growth — minimal reproducers

Two SEPARATE defects, each reproduced in ~15 lines, each with a DIFFERENT error.
Neither can be a `#[test]`: both fail at compile time.

They are kept out of `tests/` deliberately. Wiring them into trybuild would
record the errors as *expected*, which is the wrong signal for defects that are
meant to be fixed. Run them by hand:

    cp dup_nesting.rs ../zz.rs && cargo test --test zz ; rm ../zz.rs

## 1. `dup_nesting.rs` — the `Dup<…>` wrapper (growth source 1)

`dup` hands the closure `Dup<&mut Self>`, and `Dup` is itself a `ParseStream`,
so dup-in-dup grows the stream type once per level:

    error[E0275]: overflow evaluating the requirement
      `&mut syan::source::string::Stream: ParseStream`
      = note: required for `Dup<&mut Dup<&mut Dup<&mut Dup<&mut ...>>>>`
              to implement `ParseStream`
      = note: 64 redundant requirements hidden

Trait-solving overflow. `#[recurse]`'s engine bottoms this out for CYCLIC
grammars (see CLAUDE.md); ordinary non-recursive nesting depth is not covered.

## 2. `mut_flood.rs` — the `&mut` flood (growth source 2)

INDEPENDENT of `Dup` — there is no `dup` in this file. `Parse::parse` takes
`impl IntoParseStream`, a generic parameter, which MOVES rather than reborrows,
so each descent adds a `&mut` layer:

    error: reached the recursion limit while instantiating
      `descend::<&mut &mut &mut &mut &mut ...>`

A MONOMORPHIZATION limit, not a trait-solving one. Fixing only `Dup` leaves this
in place — which is why the two are filed separately.

## Why the real crate hits both at once

A consumer grammar composes enum-inside-repetition-inside-enum for dozens of
levels; structs add a `&mut` without a `Dup`, which is why observed type names
show runs of `&mut &mut &mut` BETWEEN the `Dup<`s. Measured in
`rustyfi-syntax`: names up to 5,129 chars, `.debug_str` 1.145 GB of a 2.4 GB
rlib, and `cargo build` failing below `recursion_limit = 76`.
