# `&mut` growth — minimal reproducer

There were TWO separate defects here, each reproduced in ~15 lines, each with a
DIFFERENT error. **One is fixed**; this directory now holds only the other.

## FIXED — the `Dup<…>` wrapper (growth source 1)

`dup` used to hand the closure `Dup<&mut Self>`, and `Dup` was itself a
`ParseStream`, so dup-in-dup grew the stream type once per level:

    error[E0275]: overflow evaluating the requirement
      `&mut syan::source::string::Stream: ParseStream`
      = note: required for `Dup<&mut Dup<&mut Dup<&mut Dup<&mut ...>>>>`
              to implement `ParseStream`
      = note: 64 redundant requirements hidden

`dup` is now built on the `checkpoint_raw`/`rollback_raw`/`commit_raw` trio and
hands the closure `&mut Self`, so there is no wrapper and no growth. The old
`dup_nesting.rs` reproducer is a passing test now — see
`dup_nests_to_arbitrary_depth_generically` in `../dup_transaction.rs`.

## OPEN — `mut_flood.rs`, the `&mut` flood (growth source 2)

Always was INDEPENDENT of `Dup` — there is no `dup` in this file. `Parse::parse`
takes `impl IntoParseStream`, a generic parameter, which MOVES rather than
reborrows, so each descent adds a `&mut` layer:

    error: reached the recursion limit while instantiating
      `descend::<&mut &mut &mut &mut &mut ...>`

A MONOMORPHIZATION limit, not a trait-solving one — which is why the two were
filed separately, and why fixing `Dup` left this one in place.

`syan::parse::erase` is the workaround: `#[recurse]` wraps every recursive
field-parse call's stream in it, pinning the callee's stream type to one fixed
`&mut dyn ParseStream` layer. `erased_descent_does_not_accumulate_references` in
`../dup_transaction.rs` pins that it works. Removing the need for `erase`
entirely means changing `Parse::parse` to take `&mut S` and reborrow at call
sites — a signature change to every `Parse` impl, not attempted here.

This file cannot be a `#[test]`: it fails at compile time. It is kept out of
`tests/` deliberately — wiring it into trybuild would record the error as
*expected*, which is the wrong signal for a defect that is meant to be fixed.
Run it by hand:

    cp mut_flood.rs ../zz.rs && cargo test --test zz ; rm ../zz.rs

## Why the real crate hit both at once

A consumer grammar composes enum-inside-repetition-inside-enum for dozens of
levels; structs added a `&mut` without a `Dup`, which is why observed type names
showed runs of `&mut &mut &mut` BETWEEN the `Dup<`s. Measured in
`rustyfi-syntax`: names up to 5,129 chars, `.debug_str` 1.145 GB of a 2.4 GB
rlib, and `cargo build` failing below `recursion_limit = 76`.
