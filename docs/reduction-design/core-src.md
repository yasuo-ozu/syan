## Slice `core-src` — detailed design

Execution order: 8 dead-code removals first (steps 1–8, each a pub-API removal — verify against the whole
workspace incl. `rust/` and doc-tests), then the literal-family dup-code folds (steps 9–11: parse fold,
display dedup, unparse dedup — display before unparse, since the unparse fold relies on Display parity),
then the formatting fix (step 12) and comment/doc trims (steps 13–15). All 15 findings verified against the
worktree `../syan2-reduce`; **0 rejected**, 3 revised in detail (step 9: 8/11 impls fold, savings revised
UP to ~225; step 10: 96 idents not 100; step 11: Str/ByteStr in the cited range are untouched).
**Total estimated savings: ~804 lines** (plan said ~778; the delta is the macro-fold revision).
Baseline verification after every step: `cargo build -p syan && cargo test --workspace` in the worktree
(`cargo test --workspace` covers the `rust/` crate's tests and all doc-tests, which is required for the
pub-API steps 1–8).

### 1. Delete `core/src/nested/choice.rs` (whole module) — ~143 lines [dead-code]

Delete the file (141 lines: `Choice<HList>` type, the `_Choice`/`Choice!` macro, five `unsafe transmute`s)
plus the two hookup lines in `core/src/nested.rs`: line 2 `pub mod choice;` and line 9
`pub use choice::Choice;`.

Verified (grep re-run, whole worktree incl. tests, macro crate, `rust/`, doc fences): the only `Choice`
references outside `choice.rs` are `nested.rs:2` and `nested.rs:9`. The `Choice!` macro is unusable by
construction — its transcriber (choice.rs:15) contains `$($t:ty),*`; a fragment specifier in transcriber
position emits literal `: ty` tokens, so `Choice!(T, U)` expands to `@impl T : ty , U : ty`, which no
`@impl` rule matches (independently reproduced by the plan's verifier with a compiled mimic). Only the
useless zero-arg form expands. Fields of `Choice` are private, so no working downstream usage can exist.
No other companion edits: no doc-comment elsewhere links `choice`/`Choice`.

- Evidence (condensed): worktree-wide grep hits only choice.rs itself + nested.rs:2,9; macro front door is
  a guaranteed compile error for any non-empty invocation.
- Risk: **low** (pub-API removal, but provably unusable; semver-irrelevant for this unpublished 0.1.0
  workspace).
- Verify: `cargo build -p syan && cargo test --workspace`

### 2. Delete the `Map<S>` trait from `core/src/span.rs` — ~95 lines [dead-code]

Delete `pub trait Map<S>` (span.rs:26–33, method `fn map(self, replacement: impl FnMut(Self::Span) -> S)
-> Self::Output`) and all seven impl sites, keeping the `Spanned` halves of each: `WithSpan` (80–89),
`Vec` (146–161), `VecDeque` (163–177), `Option` (190–202), `Result` (218–230), `[T; N]` (256–268), and the
Map half of the `impl_for_tup!` macro body (287–297: the `impl<S: Span$(,$A: Map<S,...>)*> Map<S> for
($($A,)*)` block and its `type Output`).

Verified (grep re-run): zero references to `Map` outside span.rs anywhere in core/tests/macro/rust except
two prose comments using "Map" as a verb (`macro/recurse.rs:132`, `macro/visitor.rs:145`). No
`use ...span::Map` or `span::*` glob import exists, so no `.map()` call site can resolve to the trait;
the macro crate never emits a `Map` ident. The plan's verifier additionally compile-checked the exact
deletion (`cargo check --workspace --all-targets` clean).

- Evidence (condensed): trait + 7 impls only in span.rs; no import path by which the trait method is
  reachable anywhere in the workspace.
- Risk: **medium** (pub trait — possibly a planned span-remapping feature; owner call, as the plan notes).
- Verify: `cargo build -p syan && cargo test --workspace` (pub-API removal — workspace run covers `rust/`
  + doc-tests)

### 3. Delete `impl_for_map!` from `core/src/parse.rs` — ~64 lines [dead-code]

Delete the `impl_for_map!` macro definition (parse.rs:75+) and its single invocation for
`HashMap<K, V>` / `BTreeMap<K, V>` (ends at EOF, ~line 139) — the Parse ("repeat k,v pairs"), Unparse,
and Spanned impls for std map types. Leave the sibling `HashSet`/`BTreeSet` entries in
`impl_for_collection!` (lines 71–72) alone, per the plan (2 lines, not worth the churn).

Verified (grep re-run): every `HashMap`/`BTreeMap` mention in core/tests, `rust/src`, `rust/tests` is
unrelated — `core/tests/symbol.rs:207` uses `HashMap` as ident *tokens* inside a `Symbol![...]` doctest
string, and `spike_real_parsestream.rs` uses a test-local registry. No AST field anywhere is map-typed;
the macro crate never emits map types. Plan's verifier compile-checked the deletion clean.

- Evidence (condensed): no map-typed AST field, test, or macro emission in the workspace; range 75–139 is
  self-contained.
- Risk: **low** (pub trait impls on std types — unpublished workspace).
- Verify: `cargo build -p syan && cargo test --workspace`

### 4. `core/src/error.rs` cleanup — ~18 lines [dead-code]

(a) Delete `add_sub_errors` (error.rs:54–59) — grep re-run: sole hit is the definition; the singular
`add_sub_error` (49–52) is used and stays. (b) Delete `pub type Result<T>` (line 80) — grep for
`error::Result`: zero hits; macro-generated code emits `::core::result::Result<_,
::syan::error::ParseError>` (macro/recurse/emit.rs), and every `Result` in macro sources is `syn::Result`.
(c) Replace the hand-written `impl Clone for ParseError` (62–70) with `#[derive(Clone)]` on the struct
(line 33) and delete the three commented-out `// span: ...` historical lines (35, 43, 65) — verified the
struct's only fields are `message: String` and `sub_errors: Vec<Self>`, both `Clone`, and the manual impl
is the exact field-wise clone, so the derive is behavior-identical. The manual impl existed only for the
long-removed `Box<dyn Span>` field those comments memorialize (`ParseError::new`'s `_span: impl Span`
parameter still documents the intent and stays).

- Evidence (condensed): `add_sub_errors`/`error::Result` definition-only; fields all-Clone so derive ≡
  manual impl.
- Risk: **low** (pub items in `pub mod error`, but zero users; compile-verified by the plan's verifier).
- Verify: `cargo build -p syan && cargo test --workspace`

### 5. Delete `GroupAngle` + angle punct aliases — ~13 lines [dead-code]

Delete `pub type GroupAngle<T, S>` (`core/src/nested/group.rs:79`) and the four backing items in
`core/src/symbol.rs` 115–124: `pub type OpenAngle = Lt;` (116), `pub const OpenAngle` (119),
`pub type CloseAngle = Gt;` (121), `pub const CloseAngle` (124), together with their `///` doc lines and
`#[allow(non_upper_case_globals)]` attrs (the whole 115–124 block). **Companion edit:** trim the now-stale
parenthetical in the group.rs:13–16 block comment — the sentence "(`GroupAngle` has no proc-macro
delimiter, hence no impl.)" at lines 15–16.

Verified (grep re-run, `--exclude rust_old`): the only hits are the five definitions plus the group.rs:15
comment. The users live exclusively in `rust_old/` (not a workspace member, never compiled). The
underlying `Lt`/`Gt` symbols come from the table macro ending at symbol.rs:113 and are untouched.
Keep-alternative (flagged by the plan): `rust/` is an incremental rebuild of `rust_old` and its generics
port will likely re-need angle groups — but the 13 lines are trivially recreatable from git history.

- Evidence (condensed): all references outside definitions are in non-member `rust_old/`; `Lt`/`Gt` and
  `impl_group_unparse_tt!` (Paren/Brace/Bracket only) unaffected.
- Risk: **medium** (likely re-needed by a future `rust/` generics port; deletion is cheap to reverse).
- Verify: `cargo build -p syan && cargo test --workspace`
- Ordering note: do this **before** step 13 — deleting symbol.rs:115–124 shifts the doc-trim line numbers
  in step 13 down by ~10; step 13 should anchor on heading text, not line numbers.

### 6. Delete the `Joint!` type macro from `core/src/nested/joint.rs` — ~12 lines [dead-code]

Delete lines 17–28: the `mod _joint_impl { ... }` block (containing the `#[macro_export]
macro_rules! _joint_impl` and `pub use _joint_impl as Joint;`) plus the trailing `#[doc(inline)]
pub use _joint_impl::*;`. The `Joint` **struct** (defined above, re-exported by nested.rs
`pub use joint::Joint;`) is untouched — only the macro-namespace `Joint` goes.

Verified (grep re-run): zero invocations of `Joint!` anywhere in the worktree; every other `Joint`
reference is the struct (the `symbol!` proc-macro emits the type path directly:
`#syan_path::nested::Joint<(...)>` at macro/symbol.rs:129,144). The macro can never have worked: its
transcriber (joint.rs:22) references `$xrate`, which is not declared in the matcher at line 21 — a `$`
followed by a non-metavariable ident transcribes literally, so every invocation fails with "expected type,
found `$`" (empirically reproduced by the plan's verifier). It is also `#[doc(hidden)]`.

- Evidence (condensed): zero `Joint!` invocations; unbound `$xrate` makes any invocation a guaranteed
  expansion-site error.
- Risk: **low**.
- Verify: `cargo build -p syan && cargo test --workspace`

### 7. Delete `Attempt::into_inner` — ~6 lines [dead-code]

Delete the whole `impl<T> Attempt<T> { ... }` block at `core/src/nested/attempt.rs:18–23` (doc comment +
`pub fn into_inner(self) -> T { self.0 }`).

Verified (grep re-run): `into_inner` has exactly five hits — this definition, `Unordered::into_inner`
(unordered.rs:31, a different type, used by `nested_unordered.rs:43`), and two `Cell::into_inner` calls in
macro/visitor.rs:383–384. No caller of `Attempt::into_inner` exists; `nested_attempt.rs` and the OptView
impl use `.0`/Deref/`.attempt()` only. Since `.0` is `pub`, the method is pure sugar — no capability is
lost.

- Evidence (condensed): zero call sites; `.0` is pub so the method is redundant sugar.
- Risk: **low**.
- Verify: `cargo build -p syan && cargo test --workspace`

### 8. Drop `type Error` from `IntoParseStream` — ~4 lines [dead-code]

In `core/src/parse/into_parse_stream.rs`: delete `type Error;` (line 5), loosen the bound on line 6 from
`type Output: ParseStream<Atom = Self::Atom, Error = Self::Error>;` to
`type Output: ParseStream<Atom = Self::Atom>;`, and delete `type Error = T::Error;` from the blanket impl
(line 17). In the two concrete impls delete the corresponding lines: `core/src/source/string.rs:103`
(`type Error = Infallible;` — the `Infallible` import stays, it is still used elsewhere in string.rs, per
the plan's compile check) and `core/src/source/proc_macro2.rs:109`
(`type Error = core::convert::Infallible;`).

Verified (grep re-run): every `IntoParseStream` use site in core, macro-emitted code, `rust/`, and tests
is the APIT form `impl IntoParseStream<Atom = ...>` — such a type cannot be named, so `::Error` can never
be projected; `Self::Error` in parse signatures is `Parse::Error`, a different trait. The plan's verifier
ran the full workspace test suite (incl. 27 trybuild UI tests and doc-tests) clean on the exact change.

- Evidence (condensed): no `IntoParseStream::Error` projection anywhere; all bounds are `<Atom = ...>`
  only.
- Risk: **low** (compile+test verified; pub trait shape change in an unpublished workspace).
- Verify: `cargo build -p syan && cargo test --workspace`
- Ordering note: do **before** steps 10 and 14 (removing string.rs:103 / proc_macro2.rs:109 shifts their
  later-step line references by 1).

### 9. THE BIG ONE — fold the `Parse` impls in `literal/parse_impl.rs` — revised ~225 lines [dup-code]

`core/src/source/proc_macro2/literal/parse_impl.rs` (453 lines, 11 impls). All 11 impls were read and
classified. The `Some(token) => { stream.push(token); Err(...) } / None => Err(...)` tail is
**byte-identical in all 11** (hand-diffed: lines 22-26/66-70/111-115/171-175/222-226/256-260/297-301/
330-334/372-376/404-408/445-449).

**Classification — the honest answer is 8/11 fold, not 11:**

| Type | Lines | Matches on | Verdict |
|---|---|---|---|
| `Bool` | 3–30 | `TokenTree::Ident` | **stays hand-written** — wrong token kind for the scaffold |
| `ByteChar` | 32–74 | `TokenTree::Literal` | **stays hand-written** (uses shared `unescape`) — see caveat |
| `Char` | 76–119 | `TokenTree::Literal` | **stays hand-written** (uses shared `unescape`) — see caveat |
| `Integer` | 121–179 | `TokenTree::Literal` | folds |
| `Float` | 181–230 | `TokenTree::Literal` | folds |
| `Str` | 232–264 | `TokenTree::Literal` | folds |
| `StrRaw` | 266–305 | `TokenTree::Literal` | folds (via `parse_raw`) |
| `ByteStr` | 307–338 | `TokenTree::Literal` | folds |
| `ByteStrRaw` | 340–380 | `TokenTree::Literal` | folds (via `parse_raw`) |
| `CStr` | 382–412 | `TokenTree::Literal` | folds |
| `CStrRaw` | 414–453 | `TokenTree::Literal` | folds (via `parse_raw`) |

**Design: a plain generic fn owns the scaffold; a thin `macro_rules!` owns only the impl boilerplate.**
A fn is strictly better than a monolithic macro here because nothing *token-shaped* varies per type — only
a closure value — so the fn gets real type-checking, and the macro is reduced to the impl header +
`type Error` + fn signature, which is where `macro_rules!` actually earns its keep.

```rust
/// Shared scaffold for all `Literal`-based `Parse` impls: read one literal token,
/// hand its string form to `f`; on `None` (or a non-literal token / EOF), restore
/// the stream and fail — mirrors the existing push-back-on-failure behavior.
fn parse_lit<T>(
    stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
    f: impl FnOnce(&str) -> Option<T>,
) -> Result<T, ParseError> {
    let mut stream = stream.into_parse_stream();
    match stream.next() {
        Some(proc_macro2::TokenTree::Literal(lit)) => {
            let lit_str = lit.to_string();
            match f(&lit_str) {
                Some(value) => Ok(value),
                None => {
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(ParseError::new(Span::default(), "parse failed"))
                }
            }
        }
        Some(token) => {
            stream.push(token);
            Err(ParseError::new(Span::default(), "parse failed"))
        }
        None => Err(ParseError::new(Span::default(), "parse failed")),
    }
}

macro_rules! impl_parse_lit {
    ($Ty:ident, $body:expr) => {
        impl Parse<proc_macro2::TokenTree> for $Ty {
            type Error = ParseError;

            fn parse(
                stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
            ) -> Result<Self, Self::Error> {
                parse_lit(stream, $body)
            }
        }
    };
}
```

**Shared helper (a): escape table for `ByteChar`/`Char`** — the two 6-arm escape matches (ByteChar 49–57
vs Char 94–102) are equivalent (all values ASCII, so ByteChar's `as u8` cast is lossless):

```rust
/// Shared escape table for ByteChar/Char (all-ASCII values; `c as u8` is lossless).
fn unescape(rest: &str) -> Option<char> {
    match rest {
        "n" => Some('\n'),
        "t" => Some('\t'),
        "r" => Some('\r'),
        "\\" => Some('\\'),
        "'" => Some('\''),
        "0" => Some('\0'),
        _ => None,
    }
}
```

**Shared helper (b): raw-string hash/quote logic** — triplicated verbatim in StrRaw/ByteStrRaw/CStrRaw
modulo the prefix, which the caller strips (`strip_prefix("r"/"br"/"cr")`) before calling:

```rust
/// Shared by StrRaw/ByteStrRaw/CStrRaw after the caller strips the "r"/"br"/"cr" prefix.
/// NOTE: intentionally reproduces the current hash-counting loop bit-for-bit — the
/// `while let Some('#') = chars.next()` loop consumes the first non-'#' char (the
/// opening quote) via the failed match arm, so `remaining` never starts with '"' and
/// every raw-string literal currently FAILS to parse, at any hash count including zero.
/// Pre-existing latent bug, identical in all three impls today (tests tolerate it via
/// `if let Ok`); this fold preserves it — do NOT "fix" it while applying this design.
fn parse_raw(rest: &str) -> Option<(String, usize)> {
    let mut hash_count = 0;
    let mut chars = rest.chars();
    while let Some('#') = chars.next() {
        hash_count += 1;
    }
    let remaining: String = chars.collect();
    if remaining.starts_with('"') && remaining.ends_with('"') {
        Some((remaining[1..remaining.len() - 1].to_string(), hash_count))
    } else {
        None
    }
}
```

**Per-type invocation lines (the 8 that fold):**

```rust
impl_parse_lit!(Str, |s: &str| {
    (s.starts_with('"') && s.ends_with('"')
        && !s.starts_with('r') && !s.starts_with('b') && !s.starts_with('c'))
        .then(|| Str { value: s[1..s.len() - 1].to_string() })
});

impl_parse_lit!(ByteStr, |s: &str| {
    (s.starts_with("b\"") && s.ends_with('"') && !s.starts_with("br"))
        .then(|| ByteStr { value: s[2..s.len() - 1].bytes().collect() })
});

impl_parse_lit!(CStr, |s: &str| {
    (s.starts_with("c\"") && s.ends_with('"') && !s.starts_with("cr"))
        .then(|| CStr { value: s[2..s.len() - 1].to_string() })
});

impl_parse_lit!(StrRaw, |s: &str| {
    let (value, hash_count) = parse_raw(s.strip_prefix('r')?)?;
    Some(StrRaw { value, hash_count })
});

impl_parse_lit!(ByteStrRaw, |s: &str| {
    let (value, hash_count) = parse_raw(s.strip_prefix("br")?)?;
    Some(ByteStrRaw { value: value.bytes().collect(), hash_count })
});

impl_parse_lit!(CStrRaw, |s: &str| {
    let (value, hash_count) = parse_raw(s.strip_prefix("cr")?)?;
    Some(CStrRaw { value, hash_count })
});

impl_parse_lit!(Integer, |s: &str| {
    const SUFFIXES: &[&str] = &[
        "u8", "u16", "u32", "u64", "u128", "usize",
        "i8", "i16", "i32", "i64", "i128", "isize",
    ];
    if s.contains('.') {
        return None;
    }
    for suffix in SUFFIXES {
        if let Some(value) = s.strip_suffix(suffix) {
            if value.chars().all(|c| c.is_ascii_digit() || c == '_'
                || (c == '-' && value.starts_with('-'))) {
                return Some(Integer {
                    value: value.to_string(),
                    suffix: Some((*suffix).to_string()),
                });
            }
            // no early return on a failed suffix match — the original loop keeps trying
        }
    }
    (s.chars().all(|c| c.is_ascii_digit() || c == '_' || (c == '-' && s.starts_with('-'))))
        .then(|| Integer { value: s.to_string(), suffix: None })
});

impl_parse_lit!(Float, |s: &str| {
    const SUFFIXES: &[&str] = &["f32", "f64"];
    if !s.contains('.') {
        return None;
    }
    for suffix in SUFFIXES {
        if let Some(value) = s.strip_suffix(suffix) {
            if value.parse::<f64>().is_ok() {
                return Some(Float {
                    value: value.to_string(),
                    suffix: Some((*suffix).to_string()),
                });
            }
        }
    }
    s.parse::<f64>().ok().map(|_| Float { value: s.to_string(), suffix: None })
});
```

**Impls that stay hand-written, and why:**
- **`Bool`** — matches `TokenTree::Ident`, not `Literal`; the scaffold's `Some(Literal(lit))` arm is the
  wrong shape. A one-off "parse_ident" helper for a single caller would cost more lines than the ~15-line
  hand-written body it replaces. Leave unchanged.
- **`ByteChar` and `Char`** — same outer scaffold, but their *inner* escape-match has two failure branches
  (bad escape char, ByteChar:56/Char:101; multi-char non-escape, ByteChar:58–60/Char:103–105) that do
  **not** push the literal back, unlike every other failure path in the file. Folding them through
  `parse_lit`'s `Option<T>` closure would push back on *every* `None` uniformly — a real (if
  arguably-fixing) behavior change on public parsing code. **Decision: preserve the inconsistency** — keep
  both hand-written but route their escape tables through the shared `unescape()` (behavior-identical,
  ~8 lines saved per impl). If the owner later wants the uniform push-back-on-all-failures behavior, both
  drop into `impl_parse_lit!` as a separate, explicitly flagged commit.

**Revised savings estimate (honest numbers):** before = 441 lines of impls; after ≈ 218 lines
(`parse_lit` 20 + `parse_raw` 11 + `unescape` 9 + `impl_parse_lit!` 12 + `Bool` unchanged 28 +
`ByteChar`/`Char` ~35 each + 8 invocation bodies ~51 + separators ~16). **≈ 225 lines saved** — slightly
*above* the plan's ~200 even though only 8/11 impls take the full fold, because the folded closure bodies
also tighten (`strip_suffix`/`bool::then` vs the original nested if/else) and `parse_raw`/`unescape`
remove duplication the original estimate didn't itemize. Also drops the stray blank line before each
closing brace (11×), already counted.

- Evidence (condensed): identical scaffold/tail in all Literal-matching impls; Bool matches Ident (as the
  plan's verifier already flagged); escape tables and raw-hash logic duplicated verbatim.
- Risk: **medium** — restructures 8 public `Parse` impls' internals (signatures unchanged); the
  no-push-back escape quirk and the raw-string latent bug are both deliberately preserved. Drops toward
  low because behavior is preserved bit-for-bit.
- Verify: `cargo build -p syan && cargo test --workspace` (must include `core/tests/proc_macro2_literal.rs`
  and the inline `mod tests` in `literal.rs`; also `rust/tests/rustsub_roundtrip.rs` exercises literal
  parsing end-to-end).

### 10. Dedup `literal/display_impl.rs` — ~18 lines [dup-code]

(a) Collapse the token-identical `Integer` (41–48) and `Float` (50–57) Display bodies (fields verified
`value: String`, `suffix: Option<String>` at literal.rs:22–25) to:

```rust
write!(f, "{}{}", self.value, self.suffix.as_deref().unwrap_or(""))
```

(b) Share the raw-string family (StrRaw 69–74, ByteStrRaw 94–100, CStrRaw 112–117 — differ only in prefix
and value rendering) via:

```rust
fn raw(
    f: &mut std::fmt::Formatter<'_>,
    prefix: &str,
    value: impl std::fmt::Display,
    hash_count: usize,
) -> std::fmt::Result {
    let hashes = "#".repeat(hash_count);
    write!(f, "{}{}\"{}\"{}", prefix, hashes, value, hashes)
}
```

Call sites: `raw(f, "r", &self.value, self.hash_count)` (StrRaw);
`raw(f, "br", String::from_utf8_lossy(&self.value), self.hash_count)` (ByteStrRaw — its field is
`Vec<u8>`, hence the lossy conversion, matching current behavior); `raw(f, "cr", &self.value,
self.hash_count)` (CStrRaw).

(c) Delete three narration comments (verbatim): line 3 `// Display implementations`; line 12
`// Handle common escape sequences` (ByteChar); line 28 `// Handle common escape sequences` (Char).

- Evidence (condensed): Integer/Float bodies identical modulo type name; raw trio shares the
  hashes/format shape; comments restate the match arms below them.
- Risk: **low** (output-identical refactor + comment-only). **Do this step before step 11** — step 11's
  `self.to_string()` calls inherit these Display impls, so land + verify Display parity first.
- Verify: `cargo build -p syan && cargo test --workspace` (Display coverage:
  `core/tests/proc_macro2_literal.rs` display tests + inline `test_display_implementations`).

### 11. Dedup `literal/unparse_impl.rs` via `emit_parsed` — ~40 lines [dup-code]

Six impls in 27–115 share the byte-identical 4-step scaffold (build `lit_str`;
`.parse::<proc_macro2::Literal>()` with fallback; `set_span(call_site)`; `write_one(TokenTree::Literal)`):
Integer 27–39, Float 41–53, StrRaw 62–72, ByteStrRaw 81–92, CStr 94–103, CStrRaw 105–115. **Correction to
the finding's cited range:** `Str` (55–60) and `ByteStr` (74–79) also sit inside 27–115 but use direct
`Literal::string`/`byte_string` constructors — they are not part of the fold and stay untouched. Extract:

```rust
fn emit_parsed<S: Emitter<proc_macro2::TokenTree>>(
    sink: &mut S,
    s: String,
    fallback: impl FnOnce() -> proc_macro2::Literal,
) -> Result<(), S::Error> {
    let mut literal = s.parse::<proc_macro2::Literal>().unwrap_or_else(|_| fallback());
    literal.set_span(proc_macro2::Span::call_site());
    sink.write_one(proc_macro2::TokenTree::Literal(literal))
}
```

Display parity was cross-checked per type: Integer/Float/StrRaw/ByteStrRaw/CStrRaw's `lit_str`
construction is literally the same `format!` as their Display body, so the five call sites become:

```rust
// Integer
emit_parsed(sink, self.to_string(), || proc_macro2::Literal::i64_unsuffixed(0))
// Float
emit_parsed(sink, self.to_string(), || proc_macro2::Literal::f64_unsuffixed(0.0))
// StrRaw
emit_parsed(sink, self.to_string(), || proc_macro2::Literal::string(&self.value))
// ByteStrRaw
emit_parsed(sink, self.to_string(), || proc_macro2::Literal::byte_string(&self.value))
// CStrRaw
emit_parsed(sink, self.to_string(), || proc_macro2::Literal::string(&self.value))
```

**`CStr` stays hand-written verbatim** — its divergence is real: Unparse builds
`format!("c\"{}\"", self.value)` (no escaping) while its Display escapes backslash/quote
(`replace('\\', "\\\\").replace('"', "\\\"")`). Folding it through `self.to_string()` would silently
change emitted tokens. Flag the escape inconsistency as a possible bug separately; do not fix here.

- Evidence (condensed): 6 impls share the identical parse/fallback/set_span/write_one block; Display ≡
  lit_str for 5 of them; CStr's Display/Unparse genuinely diverge.
- Risk: **low** (mechanical, behavior-preserving for the 5; CStr excluded). Depends on step 10 landing
  first (Display parity).
- Verify: `cargo build -p syan && cargo test --workspace` (round-trip coverage:
  `core/tests/proc_macro2_literal.rs` unparse/roundtrip tests, `rust/tests`).

### 12. Reformat the `impl_parse_for_char!` invocation in `source/string.rs` — ~86 lines [dup-code/format]

Lines 139–236 (98 lines) are exactly `impl_parse_for_char!(` + **96** comma-separated bare idents
(finding said ~100 — minor correction), one per line, + `);`. Change the invocation to brace-delimited
(`impl_parse_for_char! { _a, _b, ... }`) packed ~8–10 idents per line: rustfmt treats a brace-delimited
macro invocation as an opaque block and leaves it as typed, whereas the paren form parses as a
comma-separated expression list and gets exploded vertically. Zero semantic change (delimiter choice is
irrelevant to `macro_rules!` matching). Nuance vs the plan's cited precedent: `impl_char!` in symbol.rs
(70–113) is *also* paren-delimited but survives densely only because its body (`_a(a)@'a' ...`) doesn't
parse as an expression list — the brace form is still the standard, documented workaround and the right
fix here. Expected result: 98 lines → ~12 (≈86 saved).

- Evidence (condensed): pure rustfmt-behavior artifact; body is a plain ident list.
- Risk: **low** (formatting only). Apply **after** step 8 (which deletes string.rs:103, shifting this
  range by −1). Confirm with `cargo fmt --check` that rustfmt leaves the brace form alone.
- Verify: `cargo build -p syan && cargo test --workspace && cargo fmt --check`

### 13. Trim `core/src/symbol.rs` doc bloat — ~65 lines [comment]

All four doctests (252–260, 310–322, 329–339, 349–353) and the character-mapping table (343–347) are kept
— they are executed coverage / real contract. Line numbers below are pre-step-5 (after step 5 they shift
−10; anchor on the quoted headings).

(a) `imp::_Symbol` variant docs: DELETE 150–153 ("The symbol instance variant. … This is the only variant
that can be constructed…"); KEEP 156–161 ("Unreachable phantom variant… cannot be constructed due to the
[`Infallible`] field…" — real invariant); DELETE 165–168 ("Convenience re-export of the `Symbol`
variant…").

(b) The 52-line doc on `pub use imp::_Symbol as Symbol` (223–274): KEEP 223–227 (two-sentence summary)
**and line 229, the link definition `/// [\`Joint\`]: struct@crate::nested::Joint`** — line 226 references
`[Joint]`, so deleting the link def would leave a dangling rustdoc link. DELETE 231–234 (`# Variants`),
236–241 (`# Type Parameter` — its own `[\`chars\`]` link def at 241 is referenced nowhere else, safe to
delete together); KEEP 243–248 (`# Usage`, the "use the `Symbol!` macro" pointer) and 250–260 (doctest);
DELETE 262–266 (`# Traits`), 268–273 (`# Implementation Details`).

(c) `Symbol!` macro doc (294–374): KEEP 294–298 (summary); DELETE 300–304 (`# Syntax`, a non-executed
```` ```text ```` fence); KEEP 306–353 in full (the `# Examples` section: 3 doctests + the a-z/A-Z/0-9/_
mapping table); DELETE 355–359 (`# Type Structure`), 361–366 (`# Traits`), 368–373
(`# Implementation Details`).

- Evidence (condensed): deleted sections restate the item/derives/`_Phantom` doc or the doctests; all
  executed coverage and both real invariants (Infallible-uninhabited; chunking) survive.
- Risk: **low** (prose-only; the one trap is the `[Joint]` link def — keep line 229).
- Verify: `cargo build -p syan && cargo test --workspace` (doc-tests must stay green; optionally
  `cargo doc -p syan 2>&1 | grep -i warn` for the link check).

### 14. Trim narration comments in `core/src/source/proc_macro2.rs` — ~9 lines [comment]

Delete (verbatim first words quoted): line 70 `// Update is_joint based on the token type…` (restates the
`if let` at 71–75); lines 133–135 `// emit_sep should modify the spacing of the last token…` /
`// We need to convert the TokenStream to a vector, modify…` / `// and rebuild the stream`; line 140
`// Rebuild the stream with all tokens except the last` (**addendum** — same restatement species, the
plan's range list omitted it, hence ~9 not ~8 lines); line 143 `// Modify the last token's spacing if
it's a punctuation token`; line 146 `// Create a new punct with Alone spacing to indicate separation`;
line 153 `// For non-punct tokens, just add them back as-is`. Keep ONE folded contract line above
`fn write_sep`:

```rust
// write_sep re-spaces the trailing punct as Alone to signal separation.
```

- Evidence (condensed): each comment maps 1:1 to the adjacent statement; only the write_sep contract is
  worth one kept line.
- Risk: **low** (comment-only). Apply after step 8 (line 109 deletion shifts these by −1).
- Verify: `cargo build -p syan && cargo test --workspace`

### 15. Fix the stale wrapped-element doc in `core/src/visit.rs` — ~7 lines [comment]

Confirmed stale on close reading: the `//` block comment at **lines 60–63** (plan said 60–67; the stale
paragraph is exactly 60–63) justifies `SeqView<T>`'s type parameter by the coexistence of
`SeqView<Box<U>>` and `SeqView<U>` impls on `Vec<Box<U>>` — first words: `// The element type is a
**type parameter** (\`SeqView<T>\`, not an associated type) so the \`Box\`-wrapped // element forms
(\`Vec<Box<T>>\`, \`Option<Box<T>>\`) can implement the *unboxed* view…`. No such wrapped-element impl
exists: the actual impl block (217–324) is exactly `SeqView<T> for Vec<T>/VecDeque<T>/Punctuated<T,P>` and
`OptView<T> for Option<T>/Box<T>/Attempt<T>` — bare-element only, as the correct comment at 211–215
already states.

Replacement for 60–63 (the corrected rationale, one statement in the same style — consistent with
CLAUDE.md's "bare-element: the element *is* the viewed node… wrapped shapes descend by per-layer
recursion"):

```rust
// The element type is a **type parameter** (`SeqView<T>`, not an associated type); the traits are
// bare-element only — a wrapper like `Box<T>`/`Attempt<T>` implements `OptView<T>` directly (single-slot,
// always-full) and the visitor descends *through* wrapped shapes by recursing per layer, not via any
// wrapped-element `SeqView`/`OptView` impl.
```

Also fix the two stale doc phrases: in the `SeqView` trait doc at **65–66**, delete the clause `, and
their /// \`Box\`-wrapped element forms — box-transparent, so the element type is \`T\`, not \`Box<T>\`` —
corrected line:

```rust
/// A mutable, **sequence-like** view of an AST collection field (`Vec`/`VecDeque`/`Punctuated`),
/// bare-element — the element type is `T` itself, never a wrapped `Box<T>`. A generated
```

and in the `OptView` doc at **183–184**, delete `(and \`Option<Box<T>>\`, /// box-transparent)` —
corrected line:

```rust
/// A mutable, **Option-like** view (≤1 element) of an AST `Option` field, bare-element (nested
/// `Box`/`Attempt` layers descend separately). A generated
```

- Evidence (condensed): grep `SeqView<Box` finds no impl, only the stale comment; CLAUDE.md documents the
  wrapped-element model as removed.
- Risk: **low** (doc-only; the replacement wording matches the shipped bare-element design).
- Verify: `cargo build -p syan && cargo test --workspace`
