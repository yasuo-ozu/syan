# `syan-bench` — nom vs chumsky vs combine vs syan, one grammar

A like-for-like benchmark of five parser backends over the same arithmetic-expression grammar:

```text
expr   := term (('+' | '-') term)*      -- left-associative
term   := atom (('*' | '/') atom)*      -- binds tighter
atom   := int | '(' expr ')'
```

| backend | crate | input model |
|---|---|---|
| `nom` | nom 8 | `&str`, hand-written precedence climbing |
| `chumsky` | chumsky 0.13 | `&str`, `recursive` + `foldl`, `Rich` errors |
| `combine` | combine 4.6 | `&str`, Parsec-style `chainl1`, zero-copy `take_while1` |
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

`rustc 1.90.0`, release, single machine, `taskset -c 6`. **One criterion pass** at
`--warm-up-time 0.3 --measurement-time 1.0 --sample-size 20` (2026-08-11), so every cell below is
comparable with every other — and **not** with figures from any other run: absolute times on this
machine move ±35% with background load. Reproduce, don't trust.

**Token-based numbers exclude tokenisation.** Turning text into a `TokenStream` is a separate stage —
in a proc macro the compiler has already done it — so the token column is `parse-only`. The
tokenisation cost is shown separately in the `(lex)` column for context; it is 1–4% of the parse.

### Wall time — all four backends, both engines

| shape | case | nom | chumsky | char rk | char st | token rk | token st | (lex) | best syan / nom |
|---|---|---|---|---|---|---|---|---|---|
| flat | 4ops | **103 ns** | 649 ns | 7.16 µs | 1.30 µs | 7.14 µs | 1.90 µs | 297 ns | 13× |
| flat | 16ops | **546 ns** | 2.28 µs | 26.4 µs | 5.52 µs | 27.6 µs | 8.26 µs | 1.37 µs | 10× |
| flat | 64ops | **2.39 µs** | 9.02 µs | 109.6 µs | 24.8 µs | 111.5 µs | 33.9 µs | 5.90 µs | 10× |
| flat | 256ops | **9.45 µs** | 35.7 µs | 425.7 µs | 95.3 µs | 453.3 µs | 136.8 µs | 23.1 µs | 10× |
| nested | depth1 | **94 ns** | 581 ns | 8.19 µs | 1.56 µs | 8.05 µs | 1.68 µs | 190 ns | 17× |
| nested | depth8 | **287 ns** | 2.06 µs | 29.9 µs | 6.85 µs | 29.7 µs | 5.45 µs | 659 ns | 19× |
| nested | depth32 | **1.03 µs** | 7.24 µs | 104.2 µs | 25.0 µs | 104.3 µs | 17.8 µs | 2.85 µs | 17× |
| nested | depth128 | **3.98 µs** | 28.3 µs | 397.4 µs | 92.8 µs | 402.1 µs | 66.6 µs | 11.7 µs | 17× |
| tree | depth2 | **209 ns** | 1.25 µs | 19.2 µs | 4.09 µs | 19.1 µs | 4.08 µs | 439 ns | 20× |
| tree | depth4 | **1.03 µs** | 5.53 µs | 86.0 µs | 19.2 µs | 85.4 µs | 18.8 µs | 2.07 µs | 18× |
| tree | depth6 | **4.27 µs** | 22.5 µs | 347.9 µs | 79.0 µs | 355.6 µs | 79.5 µs | 9.94 µs | 18× |
| tree | depth8 | **19.5 µs** | 103.5 µs | 1.58 ms | 338.7 µs | 1.63 ms | 370.6 µs | 39.8 µs | 17× |
| error | 4ops | **278 ns** | 999 ns | 9.57 µs | 1.75 µs | 9.28 µs | 2.39 µs | 430 ns | 6× |
| error | 64ops | **2.63 µs** | 9.99 µs | 113.9 µs | 22.5 µs | 122.4 µs | 36.7 µs | 6.68 µs | 9× |

`rk` = `#[recurse]` (ranked), `st` = `#[recurse(structural)]`. "best syan" is the fastest of the four
syan configurations for that row.

Ratios against nom, across the whole table:

| configuration | vs nom | vs chumsky |
|---|---|---|
| chumsky | 3.6–7.2× | — |
| **syan char + structural** (best) | **6–24×** | **1.7–3.5×** |
| syan token + structural | 9–20× | 2.4–3.8× |
| syan token + ranked | 33–103× | 9.3–15.8× |
| syan char + ranked | 34–104× | 9.6–15.6× |

### Allocations per parse

Exact and machine-independent — and **identical between the two engines at every input**, because
they differ in how the cyclic obligation is discharged, not in the generated parse bodies.

Unit: **allocations per AST node** (calls to the global allocator; a `realloc` counts as one).

| input | nodes | nom | chumsky | combine | char (rk = st) | token (rk = st) |
|---|---|---|---|---|---|---|
| `flat/4ops` | 7 | **0.86** | 2.29 | **0.86** | 6.14 | 8.86 |
| `flat/64ops` | 127 | **0.99** | 2.02 | **0.99** | 5.80 | 8.55 |
| `flat/256ops` | 511 | **1.00** | 2.00 | **1.00** | 5.77 | 8.52 |
| `tree/depth4` | 31 | **0.97** | 3.00 | **0.97** | 17.90 | 15.39 |
| `tree/depth8` | 511 | **1.00** | 3.00 | **1.00** | 18.00 | 15.49 |

**combine matches nom byte for byte at every input** — same allocation count *and* same bytes
(144 / 3 024 / 12 240 B on the flat cases). It hits the inherent floor for this AST exactly: the
output `Box`es and nothing else. On the failure path it is even below nom (7 vs 9 allocations at
`error/4ops`, 127 vs 129 at `error/64ops`), since it allocates nothing for the error itself.

That is worth more than one more row. Before combine, the floor was nom alone, and it was arguable
that nom's is a hand-written precedence climber rather than a combinator library. combine **is** a
combinator library — Parsec lineage, `chainl1`, full backtracking machinery — and still allocates
nothing per token. So "combinator library ⇒ per-token allocation" is refuted, and chumsky's 2–3 per
node is a *choice* (the `Rich` error type), not a structural cost of the style.

`nested/depthN` holds 3 AST nodes at every depth, so per-node is meaningless; per **nesting level**:

| input | nom (total) | chumsky | combine | char | token |
|---|---|---|---|---|---|
| `nested/depth8` | 2 | 23 | 2 | 204 | 120 |
| `nested/depth32` | 2 | 71 | 2 | 736 | 384 |
| `nested/depth128` | 2 | 263 | 2 | 2 852 | 1 440 |
| → per paren level | 0.02 | 2.1 | **0.02** | **22.3** | 11.3 |

### Ranked vs structural

Structural is faster on **all 14 rows**, by 4.2–5.5× (char) and 3.3–6.0× (token):

| input | ranked | structural | |
|---|---|---|---|
| `char flat/256ops` | 425.7 µs | 95.3 µs | **4.5× faster** |
| `char nested/depth32` | 104.2 µs | 25.0 µs | **4.2× faster** |
| `char nested/depth128` | 397.4 µs | 92.8 µs | **4.3× faster** |
| `token nested/depth32` | 104.3 µs | 17.8 µs | **5.9× faster** |
| `token nested/depth128` | 402.1 µs | 66.6 µs | **6.0× faster** |
| `token tree/depth8` | 1.63 ms | 370.6 µs | **4.4× faster** |

An earlier revision of this file called deep-char "a wash" (1.11–1.16×, with one separately-sampled
run putting structural 17% *behind*). That is void, and the reason is instructive: at the time both
engines were dominated by an O(depth²) trait-object tower that `#[recurse]` built around every
recursive call. With that removed the engines differ only in how the obligation is discharged, and
the difference is uniform.

Since allocations are equal, the gap is not memory traffic. It is ranked's re-entry registry — a
thread-local lookup keyed on `type_name`, per recursive call — which structural's layout cast does
not need. `../perf-measurements.md` §3b instruments it: 574–612 registrations per parse, 146–183 ns
each, ~⅔ of that the 276-byte average key hashed twice.

**Not measured: compile time**, which is the dimension the engines are designed to trade on.
Isolating it needs one crate per engine, not two modules in one.

## What the numbers say

1. **The atom model matters less than it looked, and the sign flips with shape.** char+structural is
   *faster* than token+structural on flat and error input (0.61–0.73×) and slower only on deep
   nesting (1.26–1.40× at `nested/depth32`–`depth128`), where proc-macro2 collapses `( … )` into one
   `TokenTree::Group` and syan walks far fewer atoms. An earlier revision of this file reported token
   beating char by **28×** at `nested/depth128`; that gap was the erasure tower, which char paid at
   `3·depth + 3` layers and token paid at 4, and it is gone. Pick the atom model that fits the input,
   not for speed.

2. **Engine choice is worth 3.3–6.0× at zero cost in results**, and structural is the better default
   for throughput. What it costs is scope — see the limitation below.

3. **The best syan configuration is 6–24× slower than nom and 1.7–3.5× slower than chumsky**, and the
   worst is 104× and 15.8×. Both ends improved by roughly an order of magnitude over the previous
   revision (13–47× / 3–10× best, 648× / 141× worst), from two changes: deleting the erasure tower
   and making `ParseError` an enum that renders nothing until printed.

4. **Allocation is still the floor, and it is still per grammar node** — 5.8/node (char) and
   8.6/node (token) against nom's ~1 and chumsky's ~2–3. Halving it by deleting the error strings
   moved wall time by about as much, which is the confirmation that the model is right. The
   remaining per-node cost is a `dup` checkpoint plus the leaf parsers' own `to_string`s; see
   `../perf-measurements.md` §7d, where one atom of lookahead removes 41–87% of the checkpoints.

5. **Tokenisation is cheap** — 1–4% of parse time throughout. Any intuition that the token source
   pays for proc-macro2's lexer is wrong.

6. **The error path is not disproportionately expensive** (9.3–12.3× chumsky vs 9.6–15.8× for success
   on comparable input), which bounds what further error-handling work can buy on *throughput* — the
   case for it is diagnostic quality, not speed.

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
src/combine_impl.rs combine 4.6
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
