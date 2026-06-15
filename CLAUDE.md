# Implementation plan (visitor system)

## Context

The repo already has derive macros (`Parse`, `Unparse`, `Spanned`) and two function/attribute
macros (`symbol`, `recurse`) in the `syan-macro` crate (`macro/lib.rs`), with the core library
published as crate `syan` (`core/`). There is **no** visitor support yet (grep confirms no `Ast`
trait, `Repeater`, or `visitor!` anywhere). `rust_old/src/visit.rs` is a hand-written prototype of
the *runtime shape* we want to generate (the `Visit` trait, free `visit_*` fns, `IntoVisitor`
closure adapters, `Ast::visit`). The goal is to generate that shape automatically from AST type
definitions, **across crate/module boundaries**, which is why `type-leak` (already a dep of
`syan-macro`, but currently unused) is mandated: it makes copied type references portable.

AST types in this project look like `pub enum Expr<S, Tokens> { Binary(ExprBinary<S, Tokens>), ... }`
with fields wrapped in `Box`/`Vec`/`Option`, leaf token types (`Token![S => +]`, `Integer`),
generic params `<S>` / `<S, Tokens = Infallible>`, and they may be wrapped by `#[recurse]`
(`macro/recurse.rs`) and derived via `#[macro_derive(...)]` (type-macro-derive-tricks) when they
contain type-position macros. The visitor generator must tolerate all of these.

## Mechanism overview

Two cooperating macros plus two small library items:

1. **`#[derive(Ast)]`** (new `#[proc_macro_derive(Ast)]` in `macro/lib.rs` → `macro/ast.rs`).
   For each AST type it emits, *in the type's own context*:
   - `impl Ast for T<..> {}` (empty marker trait, defined in `syan`).
   - type-leak `Repeater<N>` impls carrying each field type out of the definition context.
   - an exported callback `macro_rules!` (the "metadata macro") encoding the type's structure
     (generics, variants, field accessors, portable field types). Re-exported into the macro
     namespace under the type's own name (`$vis use __t_ast_<nonce> as T;`) so it is reachable as
     `path::to::T! { .. }` from any crate — type vs. macro live in different namespaces, so the
     struct/enum `T` and the macro `T!` coexist.

2. **`#[visitor([base =>] T, U, ...)]`** (new `#[proc_macro_attribute] visitor`) applied to an empty
   `mod`. It cannot read another type's structure directly (a proc-macro can't evaluate a
   `macro_rules!` mid-expansion), so it uses a **macro ping-pong**: it expands to an invocation of
   the first metadata macro with a continuation pointing at a hidden helper proc-macro
   (`#[proc_macro] __visitor_build` in `macro/visitor.rs`). Each metadata macro substitutes its
   structure and re-invokes `__visitor_build`, which either fetches the next needed type's metadata
   macro or, once the structure set is closed, emits the final module.

3. **Library items** in `syan` (new `core/src/visit.rs`, re-exported from `core/src/lib.rs`):
   - `pub trait Ast {}`
   - `pub trait Repeater<const INDEX: usize> { type Type: ?Sized; }` (type-leak 0.2.0 does **not**
     define this; the user crate must — see its README "Repeater"). Must be at an absolute path
     (`::syan::Repeater`) reachable from both leaker and referrer.
   The per-module `Visit` / `IntoVisitor` traits are generated inside each visitor module, not in
   core (they reference the specific AST types), modeled on `rust_old/src/visit.rs`.

## `#[derive(Ast)]` details (`macro/ast.rs`)

- Reuse `attribute::FindAttribute::get_syan()` (honor `#[syan(path)]`) and the `random()` nonce from
  `macro/lib.rs`, exactly like the existing derives.
- Build a type-leak `Leaker` via `Leaker::from_struct(&ItemStruct)` / `Leaker::from_enum(&ItemEnum)`
  (reconstruct an `ItemStruct`/`ItemEnum` from `DeriveInput`), then `reduce_roots()` and
  `finish() -> Referrer` (type-leak `lib.rs:201/218/638/555`). `Referrer::iter()` yields the ordered
  leak types → emit one `impl<..> ::syan::Repeater<N> for __TLeaker<..> { type Type = <leak_ty_N>; }`
  per index, plus the leaker marker `struct __TLeaker<..>(PhantomData<..>)`.
- Use `Referrer::expand(ty, |_, idx| parse_quote!(<__TLeaker<..> as ::syan::Repeater<#idx>>::Type))`
  to rewrite every field type into a **portable** form before encoding it into the metadata macro.
- Emit the metadata `macro_rules!` as a callback muncher. Suggested grammar (one rule):
  ```text
  (@ast { $cb:path ! { $($pre:tt)* } }) => {
      $cb ! { $($pre)*
          @type [T] @generics [S, Tokens] @kind [enum]
          @variants [
              { @name [Binary] @style [tuple]
                @fields [ { @accessor [0] @ty [< portable ty for ExprBinary<S,Tokens> >] } ] }
              ...
          ]
      }
  };
  ```
  (struct → a single synthetic variant with `@style [struct|tuple]`). Field `@ty` is the
  type-leak-portable form so `__visitor_build` can splice it into the visitor module unchanged.

## `#[visitor(...)]` details (`macro/visitor.rs` + `__visitor_build`)

Parse args: optional `base_path =>` then `Punctuated<Path, Comma>` of visited AST type paths. The
attribute target is an empty `mod name {}`; keep `name`, `vis`, attrs.

Expansion algorithm (the ping-pong, all in `__visitor_build`):
- State threaded through each bounce as token blocks: `@config { name, vis, base, visited:[..] }`,
  `@done { ...resolved structures... }`, `@queue [ paths still to fetch ]`.
- Start: `#[visitor]` emits `FirstType! { @ast { ::syan::__visitor_build! { @config{..}
  @done{} @queue[ rest... ] } } }`.
- Each metadata macro appends its structure to the `__visitor_build` arg; `__visitor_build`:
  1. Records the structure in `@done`.
  2. Scans that structure's field types for **referenced Ast types not yet resolved** (e.g. `Cast`
     reached from `Expr::Cast`). Non-visited intermediates must also be drilled into, so enqueue
     them. Guard against cycles with a "seen" set.
  3. If `@queue` non-empty, emit `NextType! { @ast { ::syan::__visitor_build! { ... } } }`.
  4. If empty, emit the final module.

Final module shape (generated; mirrors `rust_old/src/visit.rs` and the spec above):
- `pub trait Visit [: base::Visit] { fn visit_t<S,Tokens>(&mut self, i: &T<S,Tokens>) { visit_t(self, i) } ... }`
  — one method **per visited type only**; inherited types come from the `base` supertrait.
- free `pub fn visit_t<V: Visit + ?Sized, S, Tokens>(this: &mut V, i: &T<..>) { <match arms> }` per
  visited type.
- `impl<V: Visit> Visit for &mut V { ... }` (forwarding, from prototype).
- `IntoVisitor<T>` trait + blanket `impl IntoVisitor<()> for T: Visit` + one
  `impl IntoVisitor<TheType> for F: FnMut(&TheType)` per visited type (closure adapters, from
  prototype lines 30–90).
- `impl<..> T<..> { pub fn visit<__V, __T>(&self, v: impl IntoVisitor<__T>) -> &Self { ... } }` per
  visited type.
- `use super::*;` at top so same-scope AST names resolve; cross-crate field types resolve because
  they were rewritten to the `<__TLeaker as Repeater<N>>::Type` portable form.

### Field-type → visit-call lowering (the core of `visit_*` bodies)

Given a field accessor `f` and its (portable) type, peel wrappers and emit:
- `Box<X>`        → recurse on `&*f` (prototype uses `&*s`).
- `Vec<X>` / `[X]`→ `for __x in f { <recurse __x> }`.
- `Option<X>`     → `if let Some(__x) = f { <recurse __x> }`.
- `&X` / `&mut X` → recurse on `f`.
- head ident `H` with H ∈ **visited set** → `this.visit_h(<ref to f>)`.
- head ident `H` that is **another Ast type** (has metadata macro) but not visited → **drill in**:
  expand H's structure and, for each of *its* fields, emit `this.visit_*(&f.<accessor>)`
  (this is exactly `Expr::Cast(cast) => this.visit_type(&cast.0)` from the spec).
- anything else (token types, primitives, `PhantomData`) → no-op leaf.

Enum arms bind variant fields with generated idents (reuse the binding strategy in
`attribute.rs::map_fields_to_idents`, `macro/attribute.rs:696`).

## Reused utilities

- `macro/attribute.rs`: `FindAttribute::get_syan` (`:15`), `map_fields_to_idents` (`:696`),
  `Adt::all_fields` (`:719/769`) and the enum/struct field-walking patterns.
- `macro/lib.rs`: `random()` nonce (`:11`) and the `#[proc_macro_error]` wrapper convention.
- `type-leak 0.2.0`: `Leaker::from_struct/from_enum/with_generics/intern`, `reduce_roots`,
  `finish`, `Referrer::{iter, expand, into_visitor, is_empty}` (it is `Parse` + `ToTokens`, so a
  `Referrer` round-trips through a `macro_rules!` as a parenthesized type list).
- `template-quote::quote!` (already used throughout `macro/`).
- Runtime shape: copy/adapt `rust_old/src/visit.rs`.

## Files to change

- `core/src/visit.rs` (new): `Ast`, `Repeater` traits. `core/src/lib.rs`: `pub mod visit;` and
  re-export `Ast` from `syan_macro` (like `span::Spanned`); add the derive to `_imp` if needed.
- `macro/lib.rs`: add `#[proc_macro_derive(Ast, attributes(syan, group, ...))]`,
  `#[proc_macro_attribute] visitor`, and hidden `#[proc_macro] __visitor_build`; `mod ast; mod visitor;`.
- `macro/ast.rs` (new), `macro/visitor.rs` (new).
- `macro/Cargo.toml`: `type-leak` dep already present (unused today) — start using it.
- Tests under `core/tests/` (mirroring `core/tests/recurse_test.rs` style) and/or `rust/`.

## Staged implementation (build + test each stage)

1. **Library scaffold**: add `Ast` + `Repeater` to `syan`; `#[derive(Ast)]` emitting only
   `impl Ast` + the metadata macro (no type-leak yet). Unit-test the metadata macro expands.
2. **Same-module visitor, visited types only**: `#[visitor(T, U)]` with `use super::*`, no drill-in,
   no inheritance. Reproduce the `Type`/`Expr` example from this file. Use the prototype runtime.
3. **Drill-in** through non-visited Ast intermediates (the `Cast` case) via the ping-pong +
   metadata-macro fetch closure.
4. **Containers**: `Box`/`Vec`/`Option`/`&` lowering.
5. **Inheritance**: `base =>` supertrait wiring; only emit new methods.
6. **type-leak portability**: switch field types to `<__TLeaker as Repeater<N>>::Type` so visitor
   modules work cross-crate / cross-module; add a 2-crate test.
7. **Robustness**: `#[recurse]` modules, `#[macro_derive]` type-macro fields (leaves), generic
   defaults, const generics.

## Verification

- `cargo build -p syan-macro && cargo test -p syan` (workspace at repo root).
- New `core/tests/visitor_test.rs`: define `Type<S>`, `Cast<S>`, `Expr<S>`, `Stmt<S>` as in this
  file's example, generate `#[visitor(Type, Expr)]` and an inheriting `#[visitor(super::v => Stmt)]`,
  then assert traversal order with a counting visitor and with a closure
  (`ast.visit(|e: &Expr<()>| { ... })`) — exactly the three call styles in `rust_old/src/visit.rs`.
- `trybuild` UI tests (pattern already in `core/tests/ui/`) for: applying `#[visitor]` to a
  non-empty module, unknown type path, and a closure type that isn't a visited type.
- Cross-crate test: a second test crate that derives `Ast` and a visitor over `syan-rust` AST types
  to prove the type-leak path works beyond one crate.

## Open decisions (resolve before/while implementing)

- **Surface syntax**: this file shows both `#[visitor(Type, Expr)]` (attribute on `mod`) and
  `visitor!(super::base => )` (function-like inside a `mod`). Plan picks the **attribute**
  `#[visitor([base =>] T, ...)]` as primary (matches "applied to empty module"); a thin
  `visitor!{ [base =>] T, ... }` wrapper can be added if the function-like form is also wanted.
- **`visit` vs `visit_mut`**: spec is `&self` only. Add `VisitMut` later by mirroring with `&mut`.
- **Method naming**: `visit_<snake_case(ident)>`; confirm desired casing for multi-word idents.

---

# Implementation plan addendum: IntoVisitor, multi-closure, visit_mut, seq/opt reduce-append

These extend the runtime shape in `rust_old/src/visit.rs`. They mostly affect what the visitor
module *generates*; the `#[derive(Ast)]` metadata macro is unchanged except it must also report,
per field, the **container kind** (`Direct` / `Box` / `Vec` / `Option`) and the inner AST head so
the generator can pick seq/opt hooks.

## IntoVisitor: composition needs a shallow-hook split

The prototype bakes recursion into each `Visit` method (`fn visit_expr(..) { f(i); visit_expr(self,i) }`).
That is correct for a **single** visitor but cannot be *composed*: calling two such methods at one
node recurses twice. To support tuples of closures with a **single** traversal, split the closure
path into a shallow hook + a driver (struct visitors keep using the prototype trait directly via
`IntoVisitor<S,()>`):

```rust
// generated per module
pub trait Visit<S> { fn visit_expr(&mut self,i:&Expr<S>){visit_expr(self,i)} /* ...per type... */ }

// shallow, no recursion; default no-ops; one method per visited type
pub trait Hook<S> { fn hook_expr(&mut self,_:&Expr<S>){} fn hook_stmt(&mut self,_:&Stmt<S>){} }

// turns any Hook into a real single-pass Visit (fires hooks at every level, recurses once)
pub struct Driver<H>(H);
impl<S,H:Hook<S>> Visit<S> for Driver<H> {
    fn visit_expr(&mut self,i:&Expr<S>){ self.0.hook_expr(i); visit_expr(self,i) }
    fn visit_stmt(&mut self,i:&Stmt<S>){ self.0.hook_stmt(i); visit_stmt(self,i) }
}

pub trait IntoVisitor<S,T>{ fn into_visitor(self)->impl Visit<S>; }
pub trait IntoHook<S,T>{ fn into_hook(self)->impl Hook<S>; }

impl<S,V:Visit<S>> IntoVisitor<S,()> for V { fn into_visitor(self)->impl Visit<S>{ self } } // struct visitors
```

The disambiguating second type param `T` keeps all impls non-overlapping (`()`, `Expr<S>`,
`(T0,T1,..)`), so no specialization is needed.

## Single + multiple closures (tuples)

```rust
// single-type hook, generated per visited type
struct ExprHook<F>(F);
impl<S,F:FnMut(&Expr<S>)> Hook<S> for ExprHook<F>{ fn hook_expr(&mut self,i:&Expr<S>){(self.0)(i)} }
impl<S,F:FnMut(&Expr<S>)> IntoHook<S,Expr<S>> for F{ fn into_hook(self)->impl Hook<S>{ExprHook(self)} }
impl<S,F:FnMut(&Expr<S>)> IntoVisitor<S,Expr<S>> for F{ fn into_visitor(self)->impl Visit<S>{Driver(ExprHook(self))} }

// chain combinator + tuple impls for arity 2..=K (e.g. 8), generated once per module
struct Chain<A,B>(A,B);
impl<S,A:Hook<S>,B:Hook<S>> Hook<S> for Chain<A,B>{
    fn hook_expr(&mut self,i:&Expr<S>){self.0.hook_expr(i);self.1.hook_expr(i)} /* ...per type... */
}
impl<S,F0,T0,F1,T1> IntoVisitor<S,(T0,T1)> for (F0,F1)
where F0:IntoHook<S,T0>, F1:IntoHook<S,T1>
{ fn into_visitor(self)->impl Visit<S>{ Driver(Chain(self.0.into_hook(), self.1.into_hook())) } }
```

So `ast.visit((|e:&Expr<()>|..., |s:&Stmt<()>|...))` runs one traversal firing both closures at the
right node types. Closures may target any subset of visited types, in any order.

## visit_mut (full mirror, `&mut`)

Generate a parallel set: `VisitMut<S>` (`fn visit_expr_mut(&mut self,&mut Expr<S>)`), free
`visit_expr_mut(this,&mut Expr<S>)`, `HookMut`/`DriverMut`, closures `FnMut(&mut Expr<S>)`,
`IntoVisitorMut`/`IntoHookMut`, tuple impls, and `AstMut::visit_mut(&mut self, v) -> &mut Self`.
Match-arm lowering identical but binds `&mut` and recurses through `*_mut` fns.

## List / Option reduce-append (the new capability)

For every visited type `X` add **container hook methods** to the trait so users can resize:

```rust
// in VisitMut<S> (override to append/remove/reorder — you get &mut Vec / &mut Option):
fn visit_expr_seq_mut(&mut self, seq:&mut Vec<Expr<S>>){ for x in seq.iter_mut(){ self.visit_expr_mut(x) } }
fn visit_expr_opt_mut(&mut self, opt:&mut Option<Expr<S>>){ if let Some(x)=opt{ self.visit_expr_mut(x) } }
// in Visit<S> (shared-ref, observation only): &[Expr<S>] / &Option<Expr<S>>
```

Container-field lowering then routes through these instead of inlining the loop:
- field `Vec<Expr<S>>`    → `this.visit_expr_seq[_mut](&[mut] f)`
- field `Option<Expr<S>>` → `this.visit_expr_opt[_mut](&[mut] f)`
- field `Box<Expr<S>>`    → `this.visit_expr[_mut](&[mut] *f)`
- field `Expr<S>`         → `this.visit_expr[_mut](&[mut] f)`

Recognized containers: `Vec`, `Option`, `Box` (also `VecDeque`; `Punctuated` later). Anything whose
inner head is not an Ast type is a leaf (no hook). Reduce = user drains/retains the `&mut Vec`;
append = user pushes. Because resizing happens in the user's `*_seq_mut` override and the driver
re-reads `seq.iter_mut()` only in the default body, user overrides have full control over count.

## Implementation/commit order (supersedes the 7-stage list above for these features)

1. lib scaffold (`Ast`,`Repeater`) → 2. `#[derive(Ast)]`+metadata macro → 3. `#[visitor]` ping-pong
+ `Visit`/free-fns/`Ast::visit` + struct visitors (`IntoVisitor<()>`) → 4. single-closure
`Driver`/`Hook` → 5. tuple `Chain` → 6. `Vec`/`Option`/`Box` lowering + `*_seq`/`*_opt` → 7. drill-in
→ 8. `visit_mut` mirror → 9. `*_seq_mut`/`*_opt_mut` reduce-append → 10. inheritance → 11. cross-crate.
Commit per numbered step once it builds + its test passes.

---

# Implementation status (what actually shipped)

Code: `core/src/visit.rs` (`Ast`, `Repeater`), `macro/ast.rs` (`#[derive(Ast)]`),
`macro/visitor.rs` (`#[visitor]` + `__visitor_build`). Tests: `rust/tests/visitor_*.rs`,
`rust/tests/cross_crate.rs`, `rust/tests/ast_derive.rs`. AST sample for cross-crate: `rust/src/lib.rs`.

**Done & tested** (one commit each, `feat(visitor): …`):

- `#[derive(Ast)]`: marker impl + `#[macro_export]` callback metadata macro carrying a cleaned copy
  of the definition (re-parsed downstream as a `syn::Item`), re-exported under the type's own name.
- `visitor!([base =>] T, …)` invoked **inside** an (otherwise empty) `mod` (function-like; a
  `macro_rules!` shim in `syan` captures `$crate` and forwards the syan path to the `__visitor_entry`
  proc-macro, so no `#[syan(..)]` is needed). Metadata ping-pong via `__visitor_build` → generates
  per visited type a `Visit`/`VisitMut` method, a free `visit_*`/`visit_*_mut` traversal fn, and
  **inherent** `visit` / `visit_mut` methods on each visited type (no trait import at the call site;
  the visitor must be generated in the types' crate). Type args are written as paths resolvable from
  inside the visitor module (e.g. `super::Expr`, `crate::ast::Expr`). Direct and `Box`-wrapped AST
  fields are traversed; other heads (incl. `Vec`/`Option`) are leaves (container traversal removed).
- Visitor inputs (the `IntoVisitor`/`IntoVisitorMut` selector design): struct visitors (via `&mut`),
  single closures, and **tuples of closures** (arity 2..=8) that run in **one** traversal via a
  shallow `Hook` + single-pass `Driver` + `Chain`.
- `visit_mut` full mirror (in-place mutation).
- **Reduce/append**: override the *parent* node's `visit_*_mut` (it owns the `&mut Vec`/`&mut
  Option` field) to add/remove/replace, then descend — see `rust/tests/visitor_reduce.rs`.
- Inheritance `#[visitor(base => New)]` for new→base reference DAGs (base exports a `__syan_visited`
  list macro; new trait extends it via supertrait).
- Cross-crate use validated.

**Generics:** the visitor trait is parameterized by the **union** of all visited types' generic
params (by name, first-decl wins), e.g. `Visit<S, Tokens>`; each type is referenced with its own
subset (`Expr<S, Tokens>`, `BinOp<S>`). One visitor can span mixed arities and closures still work
(see `rust/tests/visitor_generics.rs`). Caveat: calling `.visit()` on a root type that doesn't use
every union param may need a turbofish to pin the unused params; visiting from a full-param root
infers them.

**Two known limitations (deferred):**

1. **Auto drill-in through *unlisted* wrapper types** (the `Cast` case from the spec) is not done:
   deciding "is this field-head an `Ast` type to drill into?" can't be tested at macro-expansion
   time. Workaround that already works: **list the wrapper** in `#[visitor(…, Cast, …)]` — it then
   gets its own `visit_cast` that descends into its fields. (Only difference from the spec: `Cast`
   becomes visitable.)
2. **Cross-crate portability of the *visited* types is done without type-leak:** the generated module
   names them by the full path given to `#[visitor(...)]` (field types are never spliced — only head
   idents drive macro-time decisions — so type-leak's `Repeater` isn't needed for this). See
   `rust/tests/cross_crate.rs` (no module-scope import). type-leak is still wanted for `$crate`-based
   portability of *field* paths in the metadata macro (relevant once auto drill-in lands).


# TODOs

- [~] use type-leak — `#[derive(Ast)]` now builds a `type_leak::Leaker` from the definition and
  emits a `__<type>_leaker_<nonce>` marker + one `::syan::visit::Repeater<N>` impl per
  context-dependent field type (`macro/ast.rs`), so those types are accessible portably as
  `<leaker as Repeater<N>>::Type`. NOTE: the *visitor itself* never splices field types (it detects
  visited fields by head ident and emits accessors/method-names only), so it doesn't consume the
  Repeater impls yet — they're the foundation for drill-in / external metadata consumers. To make
  them discoverable, the metadata macro would also carry the leaker path (deferred until a consumer
  needs it).
- [x] add tests that use #[derive(Ast)] and #[recurse] at the same structs (`core/tests/ast_recurse.rs`; marker coexists. Building a *visitor* over recurse aliases still needs the metadata macro reachable via the alias name — future.)
- [x] move visitor tests to /core/tests, that are not related to syan-rust crate
- [ ] implement auto drill-in feature — UNBLOCKED by the `#[no_ast]` convention (see the
  "Drill-in implementation plan" section below): non-`#[no_ast]` fields are Ast *by contract*, so
  the generator follows them and invokes their metadata macros without ever testing existence.
- [x] change #[visitor] macro to `visitor!()` function-like macro used inside of visitor module, and deligate $crate to proc-macro to solve syan crate. (`$crate` in the `#[derive(Ast)]` metadata macro is only needed once field paths are spliced — folded into the type-leak TODO.)
- [x] remove visit_*_{seq,opt} (and `Cont::Vec`/`Cont::Option` container traversal)
- [x] remove Visitable trait. instead implement `visit()` directly for the AST types. (you can limit that the AST types specified to `visitor!()` macro is located in the same crate.)

---

# Drill-in implementation plan

## Context

Drill-in lets a visitor traverse *through* an Ast type it doesn't have a `visit_*` method for (the
spec's `Expr::Cast(cast) => this.visit_type(&cast.0)` — `Cast` is unlisted but must be descended
into to reach the visited `Type`). The blocker was that "is this field-head an `Ast` type?" can't be
tested at macro-expansion time. The **`#[no_ast]` convention** removes it: in a `#[derive(Ast)]`
struct/enum, **every field's (head) type is itself `#[derive(Ast)]`** — so it has a metadata macro
reachable by name — **unless the field is `#[no_ast]`** (leaf: `Token![..]`, `Integer`, primitives,
`PhantomData`, spans, …). The generator never tests Ast-ness: non-`#[no_ast]` ⇒ Ast by contract; a
forgotten `#[no_ast]` surfaces as a clear "cannot find macro `Foo`" error. This also gives the
`Repeater` impls (already emitted by `#[derive(Ast)]`) their consumer: portable sub-AST references.

## `#[derive(Ast)]` (`macro/ast.rs`)

- Add `no_ast` to the helper attributes: `#[proc_macro_derive(Ast, attributes(syan, no_ast))]`;
  read `#[no_ast]` per field (strip it from the embedded def, like other helper attrs).
- The metadata macro emits, per variant → per field, a record carrying: the **accessor** (tuple
  index or named ident), the **container** (`direct` / `box`), and for non-`#[no_ast]` fields the
  inner **head ident** plus a **portable macro path** to that field type's metadata macro (so it is
  callable from the visitor's context). `#[no_ast]` fields carry just `@no_ast`.
- Keep the `Leaker` + `Repeater<N>` impls; use the `Referrer` to make the per-field type/macro path
  portable (the consumer that justifies the type-leak work).

## `__visitor_build` (`macro/visitor.rs`) — orchestration + auto-discovery

- `visited` = types listed in `visitor!(...)` (these get `visit_*` methods / `IntoVisitor` / inherent
  `visit`). The user lists only the entry types they care about — **not** the whole graph.
- **Auto-discovery** (extends the ping-pong): when a fetched type has a non-`#[no_ast]` field whose
  head type isn't in `done`, enqueue its (portable) macro path and fetch it; repeat to the reachable
  closure. A `seen` set guards cycles.
- **Body lowering** per field (peeling `Box`):
  - `#[no_ast]` ⇒ bind `_`, skip.
  - head ∈ `visited` ⇒ `this.visit_<head>(<&accessor>)`.
  - head Ast but ∉ `visited` (**intermediate**) ⇒ **drill**: inline-descend into that type's
    fetched fields, extending the accessor (`&c.0`, `&c.0.1`, …) and recursing with the same rules.
    A recursive intermediate must be listed in `visited` to break the inline recursion (else error).

## Pathing (type-leak / `$crate`)

To invoke a sub-AST's metadata macro from the visitor's context its path must resolve there. Carry
the field's macro path portably: for same-module AST graphs, relative to the defining type's module;
for imported/cross-module, anchor via `$crate` and the `Repeater` indirection for the *type*. This
is the concrete payoff of the already-emitted `Repeater` impls + `$crate` delegation.

## Delegation note

"Delegate code via the sub-ASTs' `macro_rules!`" is realized as: each sub-AST's metadata macro
supplies its own structure (the ping-pong *is* the delegation); `__visitor_build` composes the drill
body from the fetched structures. (A purer variant where each `macro_rules!` emits its drill body
directly is possible but pushes visited-set membership tests + cycle guards into `macro_rules!` —
far harder than doing them in the proc-macro; recommend the compose-in-proc-macro approach.)

## Open decisions (resolve before implementing)

1. **Containers in scope?** Only `Direct` + `Box` are traversed today (`Vec`/`Option` were removed).
   Real ASTs need `Vec<Stmt>` / `Option<Expr>` children traversed — re-introduce them as "Ast
   containers" (iterate/deref, then drill the inner head) as part of drill-in, or keep out and
   require `#[no_ast]` on container fields?
2. **Delegation style:** compose the drill body in the `__visitor_build` proc-macro from
   fetched structures (recommended — visited-set membership + cycle guards are easy there) vs. have
   each `#[derive(Ast)]` `macro_rules!` emit its own drill body (purer "delegate via macro_rules!",
   but pushes membership/cycle logic into `macro_rules!`).

## Tests (`core/tests`)

- Spec example: `Type`, `Cast(Type)`, `Expr { Cast(Cast) }`; `visitor!(super::Type, super::Expr)`
  (Cast unlisted) ⇒ `visit_expr` drills `Expr::Cast(c) => this.visit_type(&c.0)`; assert no
  `visit_cast` exists (Cast not visitable).
- `#[no_ast]` leaf field is skipped; a non-Ast field without `#[no_ast]` is a clear error.
- Auto-discovery: `visitor!(super::Root)` only ⇒ all reachable non-leaf types traversed.
