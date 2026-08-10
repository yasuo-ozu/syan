# `syan-bench` — nom vs chumsky vs syan, one grammar

A like-for-like benchmark of four parser backends over the same arithmetic-expression grammar:

```text
expr   := term (('+' | '-') term)*      -- left-associative
term   := atom (('*' | '/') atom)*      -- binds tighter
atom   := int | '(' expr ')'
```

| backend | crate | input model |
|---|---|---|
| `nom` | nom 8 | `&str`, hand-written precedence climbing |
| `chumsky` | chumsky 0.13 | `&str`, `recursive` + `foldl`, `Rich` errors |
| `syan-char` | this repo | `Atom = WithSpan<char, Span>` via `source::string` |
| `syan-token` | this repo | `Atom = proc_macro2::TokenTree` via `source::proc_macro2` |

Each syan backend is instantiated under **both** decycle engines from a single grammar definition
(one `macro_rules!` per atom model), so the engine is the only variable:

- `ranked` — `#[recurse]`, the default: rank ladder + thread-local re-entry registry.
- `structural` — `#[recurse(structural)]`: compile-time unroll with a `#[repr(transparent)]`
  terminator, no runtime registry.

`tests/agree.rs::the_two_engines_agree_exactly` pins that they are externally indistinguishable.

## Fairness rules

These are what make or break a parser comparison, so they are enforced rather than asserted:

- **Same output.** All four produce the same `ast::Expr`. `tests/agree.rs` gates it on `eval()`,
  node count, and accept/reject agreement over hand-written *and* generated inputs, plus pinned
  precedence/associativity values so the four cannot be "consistent" by all being wrong together.
- **Same starting point.** Every backend starts from `&str`. `syan-token` therefore reports
  `lex+parse` (comparable) as its headline; `parse-only` and `lex-only` are broken out so
  proc-macro2's lexer can be separated from syan's parser, and are *not* comparable with the others.
- **Full consumption required.** Every backend rejects trailing input, so nobody wins by stopping
  early. This matters more than it sounds: syan's `#[group]` does **not** check that group content
  was consumed (`GroupParen<Integer, _>` happily accepts `( 1 2 3 )` and drops `2 3` — see
  `../error-design-vs-chumsky.md` §0.1), so `syan_token::parse_pretokenised` checks explicitly.
- **Both syan atom models and both decycle engines**, because both change the asymptotics — see below.

## Running

```sh
cargo test  -p syan-bench                                  # correctness gate — run this first
cargo run   -p syan-bench --release --bin allocs           # allocations per parse (exact, reproducible)
cargo bench -p syan-bench                                  # wall time (criterion)

# quicker wall-time pass
cargo bench -p syan-bench --bench expr -- \
    --warm-up-time 0.2 --measurement-time 0.6 --sample-size 20
```

Prefer the **allocation** table for design arguments: it is exact, machine-independent, and for these
four backends it explains most of the time difference. Wall time is included because it is what
users feel, but it is noisy.

### ⚠ effect on the workspace's `--no-default-features` check

`syan-bench` depends on `syan` with default features (it needs `proc_macro2` for `syan-token`), and
feature unification means adding it to the workspace **silently re-enables `proc_macro2` for the
whole workspace**. `cargo test --workspace --no-default-features` went from 209 tests to 360 —
i.e. it stopped testing the no-default path at all. Use one of:

```sh
cargo test -p syan --no-default-features                              # 209
cargo test --workspace --exclude syan-bench --no-default-features     # 209
```

## Measured results

`rustc 1.90.0`, release, single machine. **One criterion pass** at
`--warm-up-time 0.3 --measurement-time 1.0 --sample-size 20`, so every cell below is comparable with
every other. Reproduce, don't trust.

**Token-based numbers exclude tokenisation.** Turning text into a `TokenStream` is a separate stage —
in a proc macro the compiler has already done it — so the token column is `parse-only`. The
tokenisation cost is shown separately in the `(lex)` column for context; it is 1–4% of the parse.

### Wall time — all four backends, both engines

| shape | case | nom | chumsky | char rk | char st | token rk | token st | (lex) | best syan / nom |
|---|---|---|---|---|---|---|---|---|---|
| flat | 4ops | **211 ns** | 1.2 µs | 20.9 µs | 5.4 µs | 18.0 µs | 6.5 µs | 694 ns | 26× |
| flat | 16ops | **1.0 µs** | 4.2 µs | 67.0 µs | 19.3 µs | 63.7 µs | 25.8 µs | 2.6 µs | 19× |
| flat | 64ops | **4.2 µs** | 16.6 µs | 302.6 µs | 78.5 µs | 274.0 µs | 104.5 µs | 12.6 µs | 19× |
| flat | 256ops | **20.2 µs** | 78.5 µs | 1.30 ms | 319.2 µs | 1.39 ms | 393.1 µs | 39.0 µs | 16× |
| nested | depth1 | **132 ns** | 900 ns | 18.4 µs | 5.5 µs | 14.5 µs | 4.3 µs | 287 ns | 33× |
| nested | depth8 | **496 ns** | 3.2 µs | 84.4 µs | 45.9 µs | 56.1 µs | 17.5 µs | 922 ns | 35× |
| nested | depth32 | **1.9 µs** | 11.4 µs | 635.6 µs | 548.9 µs | 241.1 µs | 89.5 µs | 5.3 µs | 47× |
| nested | depth128 | **9.5 µs** | 43.8 µs | 6.16 ms | 5.54 ms | 731.5 µs | 198.3 µs | 15.7 µs | 21× |
| tree | depth2 | **363 ns** | 2.2 µs | 55.1 µs | 14.7 µs | 33.2 µs | 10.0 µs | 693 ns | 28× |
| tree | depth4 | **1.5 µs** | 7.7 µs | 278.1 µs | 131.1 µs | 236.7 µs | 45.6 µs | 3.0 µs | 31× |
| tree | depth6 | **6.4 µs** | 31.4 µs | 1.10 ms | 660.8 µs | 658.4 µs | 192.2 µs | 13.1 µs | 30× |
| tree | depth8 | **24.7 µs** | 137.2 µs | 5.18 ms | 3.34 ms | 2.53 ms | 798.1 µs | 61.1 µs | 32× |
| error | 4ops | **0.4 µs** | 1.3 µs | 16.3 µs | 5.4 µs | 17.1 µs | 5.7 µs | 0.6 µs | 13× |
| error | 64ops | **4.0 µs** | 14.1 µs | 208.9 µs | 64.0 µs | 207.8 µs | 74.8 µs | 9.1 µs | 16× |

`rk` = `#[recurse]` (ranked), `st` = `#[recurse(structural)]`. "best syan" is the fastest of the four
syan configurations for that row.

Ratios against nom, across the whole table:

| configuration | vs nom | vs chumsky |
|---|---|---|
| chumsky | 4–9× | — |
| **syan token + structural** (best) | **13–47×** | 3–10× |
| syan char + structural | 26–583× | 6–126× |
| syan token + ranked | 51–139× | 12–30× |
| syan char + ranked | 99–648× | 17–141× |

### Allocations per parse

Exact and machine-independent — and **identical between the two engines at every input**, because
they differ in how the cyclic obligation is discharged, not in the generated parse bodies.

| input | nodes | nom | chumsky | char (rk = st) | token (rk = st) |
|---|---|---|---|---|---|
| `flat/4ops` | 7 | **0.86** | 2.29 | 11.29 | 13.29 |
| `flat/64ops` | 127 | **0.99** | 2.02 | 9.87 | 11.87 |
| `flat/256ops` | 511 | **1.00** | 2.00 | 9.78 | 11.78 |
| `tree/depth4` | 31 | **0.97** | 3.00 | 36.39 | 28.90 |
| `tree/depth8` | 511 | **1.00** | 3.00 | 36.50 | 28.99 |

`nested/depthN` holds 3 AST nodes at every depth, so per-node is meaningless; per **nesting level**:

| input | nom (total) | chumsky | char | token |
|---|---|---|---|---|
| `nested/depth8` | 2 | 23 | 441 | 287 |
| `nested/depth32` | 2 | 71 | 1 597 | 983 |
| `nested/depth128` | 2 | 263 | 6 209 | 3 767 |
| → per paren level | 0.02 | 2.1 | **48.5** | 29.4 |

### Ranked vs structural

Structural is faster on 13 of the 14 rows, by 1.2–4.4×. The exception is deep char-source recursion:

| input | ranked | structural | |
|---|---|---|---|
| `token nested/depth32` | 241.1 µs | 89.5 µs | **2.7× faster** |
| `token nested/depth128` | 731.5 µs | 198.3 µs | **3.7× faster** |
| `token tree/depth8` | 2.53 ms | 798.1 µs | **3.2× faster** |
| `char flat/256ops` | 1.30 ms | 319.2 µs | **4.1× faster** |
| `char nested/depth32` | 635.6 µs | 548.9 µs | 1.16× faster |
| `char nested/depth128` | 6.16 ms | 5.54 ms | 1.11× faster |

(An earlier, separately-sampled run had char/depth128 going the other way — structural 17% *slower*,
with non-overlapping CIs. In this single consistent pass it is 11% faster. Treat the deep-char case as
**a wash**; the two engines are within noise of each other there, and only there.)

Since allocations are equal, the gap is not memory traffic. The plausible mechanism is ranked's
re-entry registry — a thread-local lookup keyed on `type_name`, performed per recursive call — which
structural's layout-cast does not need. That would explain why the advantage is largest where
recursion is densest per unit of input (token/nested: one `#[group]` descent per atom) and smallest
where each level does the most non-recursive work (char/nested: ~4 atoms of delimiter and whitespace
handling per level). **The bench establishes the effect, not the mechanism.**

**Not measured: compile time**, which is the dimension the engines are designed to trade on.
Isolating it needs one crate per engine, not two modules in one.

## What the numbers say

1. **The atom model matters more than anything else for recursive input.** token+structural beats
   char+structural by **28×** at `nested/depth128` (198 µs vs 5.54 ms) and by 4.2× at `tree/depth8`.
   proc-macro2 collapses `( … )` into a single `TokenTree::Group`, so syan walks far fewer atoms and
   recurses through `#[group]`. On flat input, with nothing to collapse, the two are within 1.2×.
   So "char or token?" has no single answer — it depends on how bracketed the input is, and the
   crossover is steep.

2. **Engine choice is worth 2–4× at zero cost in results**, and structural is the better default for
   throughput. What it costs is scope — see the limitation below.

3. **The best syan configuration is 13–47× slower than nom and 3–10× slower than chumsky.** The worst
   (char + ranked, deep recursion) is 648× and 141×. The distance between syan's own configurations
   is larger than the distance between nom and chumsky.

4. **The floor is allocation.** ~10–12 per grammar node against nom's ~1 and chumsky's ~2–3,
   consistent with `../error-design-vs-chumsky.md` §R5: the cost is per grammar *node* (a `dup`
   checkpoint plus a `ParseError`), not per atom. No engine choice changes this; only reducing
   per-node allocation will.

5. **Tokenisation is cheap** — 1–4% of parse time throughout. Any intuition that the token source
   pays for proc-macro2's lexer is wrong; it more than recovers that by having fewer atoms.

6. **The error path is not disproportionately expensive** (13–16× vs 16–47× for success on comparable
   input), which bounds what the error-handling redesign can buy: error construction is a slice of a
   large per-node constant, not the constant itself.

## A `structural` scope limit found while writing this

`#[recurse(structural)]` does not tolerate an **acyclic** type declared inside the module. The
grammar originally kept its `AddOp`/`MulOp` operator enums (pure leaves, not part of the
`Expr`/`Term`/`Atom`/`AddTail`/`MulTail` cycle) next to the cycle members. Under `#[recurse]` that
compiles; under `#[recurse(structural)]` it does not:

```
error[E0277]: `?` couldn't convert the error to `<g::AddOp as Parse<__SyanMacro_Atom>>::Error`
   = help: the trait `From<ParseError>` is not implemented for `<g::AddOp as Parse<..>>::Error`
error[E0308]: expected `Vec<ParseError>`, found `Vec<<AddOp as Parse<..>>::Error>`
```

The non-cycle member's `Parse::Error` associated type is left unpinned, so every cycle member whose
body `?`-propagates through it fails to type-check. Moving the two enums out of the module fixes it
(and is better practice regardless — they do not need the attribute). Worth either supporting or
diagnosing: today the error names generated internals and does not say "move this type out".

## Layout

```
src/ast.rs          the shared AST + eval/nodes (the correctness oracle)
src/input.rs        deterministic generators: flat, nested, tree, bad_at
src/alloc.rs        counting global allocator
src/nom_impl.rs     nom 8
src/chumsky_impl.rs chumsky 0.13
src/syan_char.rs    syan, Atom = WithSpan<char, Span>      — `ranked` + `structural`
src/syan_token.rs   syan, Atom = proc_macro2::TokenTree    — `ranked` + `structural`
src/bin/allocs.rs   allocation table
benches/expr.rs     criterion wall-time benches
tests/agree.rs      the fairness gate
```

Two things `syan_char.rs` has to supply that the char source does not, both written out rather than
hidden because a benchmark of a grammar nobody can write is worthless: a hand-written multi-digit
`Int` leaf (`impl_parse_for_char!` only gives single literal chars), and an explicit `Ws<T>` wrapper
for whitespace (`source::string::Stream::skip_sep()` returns `true` unconditionally, so
`#[joint]`/`#[alone]` cannot express padding).
