# Plans: the two deferred `#[recurse]` natural-type limitations

Both are **capability gaps, not bugs** — the suite is green; the engine retains the traits. Each plan is
concrete enough to execute. Code references are to `macro/recurse.rs` unless noted.

Status quo recap (what's already shipped):
- `Parse` always delegates engine→natural (`__ToNat_*` + `gen_natural_extras`).
- `Unparse`/`Spanned` reach the natural type for **group-free** cycles: directly via `#[ignore_bounds]`
  (single self-recursive) or via the `__FromNat_*` engine delegation (multi-type, depth-limited).
- `param_decls` already threads *param bounds* (`S: Span`) into the conversion/delegation impls.
- A cycle type's full `where`-clause is threaded onto the generated impls (`where_preds_of`) — **#2 DONE**.
- All generated type/trait names are nonce-stamped, so a user `ExprTerm`/`__XxxRec`/… can't collide
  (the old `audit_recurse_terminator_collision` limitation is gone).
- Finite-size guard; engine emitted only when needed (`scc_needs_engine`).

**All three originally-deferred recurse gaps are now resolved at the recurse level.** #1 below is
**DONE** (delegation wired); its residue is a *library-level* gap unrelated to recurse.

---

## #1 — Group-ful cycle `Unparse`/`Spanned` on the natural type — ✅ DONE (delegation wired)

> **CORRECTION + RESOLUTION.** The earlier probe write-up here was **wrong**: it claimed
> `__ExprRec<S,__ExprDefault<S>>: Unparse` was "not provable" because of the engine's
> `for<'a> Fill<Substruct>: Unparse` HRTB. Re-probing with a *concrete* atom (rather than reading the
> method-resolution summary, which only showed `Unparse<_>` with an unresolved atom) revealed the HRTB
> **does** resolve — the compiler walks it down to concrete leaves. The real failure was a leaf:
> `OpenBrace: Unparse<TokenTree>` — i.e. a **library-level** constraint, *not* a recurse one:
>  - brace/delimiter *symbols* only `Unparse` to an atom that is `From<String> + AtomParsedToAllChars`,
>    and `proc_macro2::TokenTree` is not (and no `From<String>` atom ships); and
>  - a `Group<(),…>` slot is `()`, which impls `Span` but **not** `Spanned`, so group `Spanned` needs
>    `(): Spanned`.
> Both reproduce **identically for a plain non-`#[recurse]` group type** — confirmed in
> `ui/recurse_group_ful_unparse.rs` (the cycle `grp::Expr` and a plain `Plain` struct fail on the same
> `OpenBrace: Unparse<TokenTree>` note).
>
> **Implemented:** dropped the `!scc_has_group` exclusion from `delegated_trait` in `recurse()`, so a
> group-ful cycle now gets the full `__FromNat` delegation + delegated `impl Unparse`/`impl Spanned`
> exactly like a multi-type cycle. The `from_conv_*`/`from_leaf_clones`/terminator machinery already
> handles a group field (the `brace` is a cloned leaf; `inner` is the recursive child). So the natural
> group-ful type now *has* the `Unparse`/`Spanned` impl, conditionally provable for any atom whose
> leaves satisfy it — no worse than a non-recurse group type.
>
> **Residual (library-level, out of scope for recurse):** to actually unparse a group to a real
> proc-macro atom, the library would need symbol→`TokenTree` `Unparse` (or a shipped `From<String>`
> atom); to `span()` an empty-slot group it would need `(): Spanned`. Those are general
> `#[derive(Unparse/Spanned)]`/atom features, tracked separately if wanted.

**Today.** A cycle with a `#[group(self.brace)]` field keeps `Unparse`/`Spanned` on the `pub(crate)`
engine only (`scc_us_natural` and the delegation sets exclude group-ful via `!scc_has_group`). So a
natural `Expr<S>` with a group field is `Parse` but not directly `Unparse`/`Spanned`.

**Why it was excluded.** The `from_nat` delegation (natural→engine, then engine's `Unparse`) is what
multi-type group-free cycles use. For a group-ful cycle two extra things bite:
1. **`from_nat` must rebuild the engine's group substruct.** The engine's `Block` variant is
   `Block { brace, #[group(self.brace)] inner: Vec<__Rec> }`; reconstructing it from the natural value is
   purely structural (clone `brace`, convert `inner`) — `from_conv_expr`/`from_conv_body` already handle
   the field shapes. So `from_nat` itself is fine; the group attr doesn't change the *value* layout.
2. **The real blocker is proving `engine: Unparse`.** The engine's derived group `Unparse` carries a
   `for<'a> <GroupBrace<(),S> as EmptyGroup>::Fill<Substruct<'a,S,__Rec>>: Unparse<Atom>` HRTB bound.
   To prove `__ExprRec<S, __ExprDefault<S>>: Unparse` the solver walks that `Fill` bound at **every
   depth level** (each a distinct nonce-named substruct), bottoming at `ExprTerm`. This chain has
   **never been proven** by any test (no test unparses a group-ful cycle, even pre-rework via the old
   alias), so it may be latently unsatisfiable or hit the trait-solver recursion limit.

**Plan (do in this order; stop when green):**

1. **First, settle whether engine group `Unparse` is provable at all** (independent of delegation).
   Add a throwaway probe: in a group-ful `#[recurse]` module, force `fn _assert() where
   __ExprRec<S, __ExprDefault<S>>: Unparse<TokenTree> {}` (needs the engine to be reachable — do it via
   a generated `#[cfg(test)]` assert, or temporarily make the engine `pub`). Two outcomes:
   - **Provable** → the only missing piece is the delegation wiring (step 2).
   - **Not provable / overflow** → the engine's group `Unparse`/`Spanned` derive is itself the problem;
     fixing delegation is pointless until the engine's group bound resolves. Likely needs the
     `Fill<Substruct>: Unparse` bound to be expressed depth-recursively (or the substruct's `Unparse`
     to drop the `for<'a>` and take the children by value). Treat as a *derive/group* fix, separate
     from recurse.
2. **If provable:** drop the `!scc_has_group` exclusion from the delegation sets so group-ful cycles
   join the `__FromNat` path:
   - in `recurse()`, `delegated_trait(..)`: remove `&& !scc_has_group[idx]` (keep `!scc_us_natural`).
   - `from_conv_*` already lowers the group field structurally (it's just `Vec<child>` after peeling),
     so no codegen change there.
   - `from_leaf_clones` already unions leaf types across the cycle; `brace: GroupBrace<(),S>` is a leaf
     → it gets `GroupBrace<(),S>: Clone` (→ `S: Clone`), which is correct (the engine's group `Unparse`
     also requires `Clone` on the brace).
   - Re-run; the group-ful `grp` test from `recurse_unparse_spanned.rs` history is the fixture (parse
     `{ 1 }`, unparse round-trip).
3. **If NOT provable (the derive-level fix):** the cleanest route is to make the engine's group
   `Unparse`/`Spanned` not rely on the HRTB `Fill` bound for the recursive child. Option: have the
   group substruct hold the child **by value already converted**, or emit the engine's group `Unparse`
   body to unparse `brace` + iterate `inner` directly (the children are `__Rec`, already `Unparse`)
   rather than through `Fill`. This is an `attribute.rs` change to `extract_unparse`'s group path,
   scoped to when the grouped field is a recurse child — risky (touches the shared derive), so gate it
   or prototype on a standalone non-recurse group-recursive type first.

**Caveats to document either way:** delegated group `Unparse`/`Spanned` is **depth-limited** (panics
past `limit`, like the multi-type case) and requires the leaves (incl. `brace`) `: Clone`.

**Estimate:** step 1 is ~30 min and decides everything. If (2): ~1h. If (3): ~half-day + derive-risk.

---

## #2 — `where`-clause / non-trivial param bounds on a Parse-deriving cycle type — ✅ DONE

> **Implemented** (commit on branch `recurse-natural-types`): `gen_natural_extras` captures each cycle
> type's `where`-clause (`where_preds_of`) and threads its predicates onto every generated trait
> declaration + impl that names the natural type (`__ToNat`/`__FromNat`/delegated `Parse`/`Unparse`/
> `Spanned`, incl. the terminator impls). Both a param bound (`where S: Clone`) and the self-referential
> `where Expr<S>: Marker` (old "problem 6") shape work. Test: `recurse_where_clause.rs`; the
> `ui/audit_recurse_where_clause.rs` + `ui/problem6_where_clause.rs` compile-fail probes were removed.
> Below is the original plan, kept for reference.

**Today.** `param_decls` threads simple *param bounds* (`S: Span`) into the conversion/delegation impls
(that's what made multi-type `Spanned` work). NOT threaded: an explicit **`where`-clause** on the cycle
type (`pub enum Expr<S> where S: Clone { … }` or `where Expr<S>: Marker`). The engine type keeps the
clause (via `transform_item` cloning generics), the derives keep it (`append_user_where_predicates`,
attribute.rs ~156), but the generated `__ToNat`/`__FromNat`/delegated-`Parse` impls in
`gen_natural_extras` do **not** carry it — so the conversion's `-> Expr<S>` (or the delegated `impl Parse
for Expr<S>`) requires the clause undischarged → E0277. Pinned by `ui/audit_recurse_where_clause.rs`.

Note `where Expr<S>: Marker` (the `problem6` shape): in the natural-type world `Expr<S>` is the *natural*
type (depth-independent), so the old "resolves to the fixed-depth alias" miscompile is gone — it's now
the same plain "clause not threaded onto the generated impls" issue.

**Plan:**

1. **Capture the cycle type's `where`-clause** alongside its generics where `gen_natural_extras` already
   reads `generics` per item (the loop binds `(id, generics)`). Add
   `let where_preds = generics.where_clause.as_ref().map(|w| &w.predicates);`.
2. **Thread it onto every generated impl** that names the natural or engine type in
   `gen_natural_extras`:
   - the `__ToNat_X` impl (`-> Expr<S>`),
   - the delegated `impl Parse for Expr<S>`,
   - the `__FromNat_X` impl and the delegated `impl Unparse`/`impl Spanned` (when present).
   Append `where_preds` to each impl's `where` block (they already have `where` clauses — extend them).
   Use the same predicates verbatim; they reference the cycle's own params, which are in scope on these
   impls.
3. **Root `Expr<S>`-self-referential predicates correctly.** A clause like `where Expr<S>: Marker`
   names the *natural* `Expr<S>` — fine on the natural-typed impls (`__ToNat`'s return,
   delegated `Parse`). But on the `__ToNat`/`__FromNat` impls whose `Self` is the *engine*, a predicate
   mentioning the natural `Expr<S>` is still valid (it's a real type) — just verify it doesn't
   accidentally need the engine form. Most clauses bound a param (`S: Clone`), which is unambiguous.
4. **Also handle the trait *declarations*** (`trait __ToNat_X<…>`, `trait __FromNat_X<…>`): they mention
   `Expr<S>` in the method signature, so if the clause is needed to name `Expr<S>` (e.g. `S: Clone`
   makes `Expr<S>` well-formed only when… — actually a where-clause doesn't gate WF of naming a type;
   naming `Expr<S>` is fine regardless). So the trait decls likely need nothing; the *impls* are where
   the obligation must be dischargeable. Confirm by test.
5. **Tests:** flip `ui/audit_recurse_where_clause.rs` from compile-fail to a passing test
   (`Expr<S> where S: Clone` deriving `Parse, Unparse`, then parse + unparse). Add a `where Expr<S>:
   Marker` positive case. Keep `ui/problem6_where_clause.rs` only if it still fails for a *different*
   reason; otherwise repurpose it.

**Scope note.** This generalizes the existing `param_decls` bound-threading from *param bounds* to
*full where-clauses*. It's localized to `gen_natural_extras` (+ maybe `transform_item` already covers the
engine type). Low risk — additive `where` predicates on the generated impls.

**Estimate:** ~1–2h including tests. Independent of #1.

---

## Sequencing & recommendation

Do **#2 first** (smaller, self-contained, clears a pinned compile-fail and a real ergonomic wall:
bounded/where-claused recurse cycles). Then **#1 step 1** (the 30-min probe) to learn whether group-ful
delegation is a quick win (#1 step 2) or a derive-level project (#1 step 3) — and decide accordingly.
