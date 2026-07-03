## Slice `macro-visitor` — detailed design

All 14 findings verified against `/home/yasuo/ghq/github.com/yasuo-ozu/syan2-reduce` (commit `e5d0576`
+ uncommitted split). Line citations below are the worktree's *current* numbers — most match the plan
exactly; a few comment findings drift by ±1 line from the plan's citation (noted inline) with no effect
on the finding's validity. Execution order below follows the plan's own listing order, which already
sequences correctly: dead-code (1–2) → dup-code (3–8) → emit-reduction (9) → comment (10–14). Estimated
~74 lines saved, matching the plan header. **0 rejected** — every finding holds on close reading; two
(#4, #6) have real emission-visible side effects, both shown safe below. Two findings (#6/#7/#8/#13) share
`macro/visitor/build_input.rs` at disjoint line ranges — apply in listed order, no conflicts.

### 1. `macro/visitor/build_input.rs:47` — drop `#[derive(Clone)]` on `AncIn` [dead-code]

- Evidence (condensed): `AncIn` is only constructed (`build_input.rs:77`, `visitor.rs:252,271`),
  iterated by reference (`emit_ancestors`, `build_input.rs:87`), or field-cloned (`a.path.clone()`,
  `a.names.clone()`); grep for a whole-value `.clone()` on `AncIn`/`Vec<AncIn>` is empty.
- **Code shape**: one-line deletion.
  ```diff
  -#[derive(Clone)]
   pub(crate) struct AncIn {
       pub(crate) path: Path,
       pub(crate) names: Vec<Ident>,
   }
  ```
- **Emission**: unaffected — `AncIn` is a proc-macro-internal struct, never in generated tokens.
- **Ordering**: independent of every other finding in this slice (no shared lines).
- **Risk**: low. **Verify**: `cargo build -p syan-macro` (a stray `.clone()` on a whole `AncIn` would be
  the only possible break, and grep confirms none exists).

### 2. `macro/visitor/util.rs:143,153` — drop `PartialEq` from `Container`/`LayerKind` [dead-code]

- Evidence (condensed): both enums are consumed only by pattern match (`lower.rs:84-89,342`,
  `util.rs:250`) or move (`view: Option<Container>` in `lower_field`); grep for `==`/`!=`/`.contains`
  on either type across `macro/` is empty; both are `pub(crate)` so no downstream impact.
- **Code shape**:
  ```diff
  -#[derive(Clone, Copy, PartialEq)]
  +#[derive(Clone, Copy)]
   pub(crate) enum Container { Seq, Opt }
   ...
  -#[derive(Clone, Copy, PartialEq)]
  +#[derive(Clone, Copy)]
   pub(crate) enum LayerKind { View, Raw }
  ```
  `Copy`/`Clone` stay (both are moved-out-of-`Option` in `lower_field`, `lower.rs:289,330`).
- **Emission**: unaffected — same as #1.
- **Ordering**: independent; touches `util.rs`, which no other finding in this slice touches.
- **Risk**: low. **Verify**: `cargo build -p syan-macro`.

### 3. `macro/visitor/side.rs` — fold the seq/opt emission pair into `Vec<ViewSpec>` [dup-code]

- Evidence (condensed): the `has_seq`/`has_opt` blocks in `trait_def` (`side.rs:259-280`) and
  `blanket_ref_impl` (`side.rs:292-301`) are token-for-token parallel (`SeqView`/`OptView`,
  `view_iter_mut`/`get_mut`, `#p_vw`/`#p_ow`, `seq_method`/`opt_method`); `seq_doc`/`opt_doc`
  (`side.rs:135-143`) differ only in wording; none of `has_seq/has_opt/seq_method/opt_method/
  seq_doc/opt_doc` is referenced outside `side.rs` (confirmed by grep — safe to restructure).
- **Concrete code shape.** Add one struct, remove six `S` fields for one:
  ```rust
  /// One container-edit view (`#[seq]` or `#[opt]`) generated for a visited type. Replaces the
  /// paired `has_seq`/`has_opt` bools + `seq_method`/`opt_method` + `seq_doc`/`opt_doc` fields on `S`
  /// with a single `Vec` so `trait_def`/`blanket_ref_impl` iterate once instead of two parallel
  /// `#(if ..)` blocks.
  struct ViewSpec {
      /// `visit_<name>_seq` / `visit_<name>_opt`.
      method: Ident,
      /// Doc string for the trait method (was `S::seq_doc`/`S::opt_doc`).
      doc: String,
      /// `::syan::visit::SeqView` / `::syan::visit::OptView` — bound root, sans `<Ty>`.
      view_trait: TokenStream,
      /// The per-method generic naming the view type: `p_vw` for seq, `p_ow` for opt (the *same*
      /// idents `gen_side` already mints once via `fresh_ident` — not reminted per type/kind).
      view_param: Ident,
      /// Trait-method default body. NOT further folded: the seq body is a `for .. in
      /// view_iter_mut(v)` loop, the opt body an `if let Some(..) = get_mut(v)` — genuinely
      /// different shapes, only the *emission site* (trait_def/blanket_ref_impl) is shared.
      default_body: TokenStream,
  }
  ```
  `struct S` loses `seq_doc: String, opt_doc: String, seq_method: Ident, opt_method: Ident,
  has_seq: bool, has_opt: bool` and gains `views: Vec<ViewSpec>` (6 fields → 1).
- **Builder** (inside the existing `.map(|t| {...})` closure that builds `sides`): hoist the
  already-duplicated `method_ident_m(&ident, mutable)` call into one `let method = ...;` (currently
  called twice — once for the `mname` doc string, once for the `S.method` field — reusing it is a
  free-standing minor simplification, not required by the finding but adjacent and harmless), then:
  ```rust
  let mut views = Vec::new();
  if mutable && seq_used.contains(&name) {
      views.push(ViewSpec {
          method: Ident::new(&format!("visit_{}_seq", to_snake(&ident)), Span::call_site()),
          doc: seq_doc,           // unchanged string, built exactly as today
          view_trait: quote!(::syan::visit::SeqView),
          view_param: p_vw.clone(),
          default_body: quote! {
              for __syan_e in ::syan::visit::SeqView::view_iter_mut(v) {
                  self.#method(__syan_e);
              }
          },
      });
  }
  if mutable && opt_used.contains(&name) {
      views.push(ViewSpec {
          method: Ident::new(&format!("visit_{}_opt", to_snake(&ident)), Span::call_site()),
          doc: opt_doc,
          view_trait: quote!(::syan::visit::OptView),
          view_param: p_ow.clone(),
          default_body: quote! {
              if let ::core::option::Option::Some(__syan_e) = ::syan::visit::OptView::get_mut(v) {
                  self.#method(__syan_e);
              }
          },
      });
  }
  ```
- **`trait_def` rewrite** (`side.rs:245-283`), replacing the two `#(if s.has_seq)`/`#(if s.has_opt)`
  blocks with one loop (loop var named `spec`, not `v`, to avoid reading confusion with the emitted
  parameter literal `v:` inside the quote body — `template_quote` only substitutes `#{...}`
  interpolations, so a bare `v` token in the quote is unaffected either way):
  ```rust
  #(for spec in &s.views) {
      #[doc = #{&spec.doc}]
      fn #{&spec.method}< #(for mp in &s.method_params) { #mp, } #{&spec.view_param}: #{&spec.view_trait}< #{&s.ty} > >(
          &mut self,
          v: &mut #{&spec.view_param},
      ) #{&s.trait_where} {
          #{&spec.default_body}
      }
  }
  ```
- **`blanket_ref_impl` rewrite** (`side.rs:285-305`), same fold, forwarder body (no `default_body`
  needed here — a distinct UFCS forward, reusing only `method`/`view_param`/`view_trait`):
  ```rust
  #(for spec in &s.views) {
      fn #{&spec.method}< #{&spec.view_param}: #{&spec.view_trait}< #{&s.ty} > >(&mut self, v: &mut #{&spec.view_param}) {
          <#p_v as #visit_tr #g_use>::#{&spec.method}(self, v)
      }
  }
  ```
- **Byte-identical emission**: the resulting token stream for a type with both `#[seq]` and `#[opt]`
  usage is the same two `fn` items in the same order (seq pushed before opt, matching the original
  `has_seq` block preceding `has_opt`); for a type with only one or neither, the loop emits exactly
  that many items — identical to the original `#(if ..)` gating. `view_trait`/`view_param` splice to
  the same tokens (`::syan::visit::SeqView< Ty >` etc.) as the inline original. Not literally
  byte-for-byte on whitespace (template-quote token trees don't preserve source spacing either way),
  but the *token stream* — what rustc and every existing test observes — is unchanged.
- **Ordering/conflicts**: touches `side.rs` lines ~55-171 (struct + builder) and ~245-305
  (`trait_def`/`blanket_ref_impl`). Finding #11 (comment cleanup) also touches the "Opt-in
  container-edit hooks…" comment that sits immediately above the old `has_seq`/`has_opt` blocks
  (`side.rs:257-258`) — apply #3 first (it restructures that region into the `views` loop), then #11
  removes the now-adjacent redundant comment. Finding #9 (use hoist) touches a disjoint region
  (`free_fns`, `side.rs:307-326`) — no conflict, order between #3 and #9 doesn't matter, but #9 is
  listed after per the plan's dup-code-before-emit-reduction convention.
- **Risk**: low (no `.stderr`-visible surface — `#[seq]`/`#[opt]` methods are exercised only by
  compiling/running real visitors, e.g. `visitor_edit.rs`, `visitor_reduce.rs`,
  `visitor_edit_containers.rs`, `visitor_edit_group.rs`, `rust/tests/cross_crate_edit.rs`).
  **Verify**: `cargo build -p syan-macro && cargo test -p syan --test visitor_edit --test
  visitor_edit_containers --test visitor_edit_group && cargo test --workspace`.

### 4. `macro/visitor/lower.rs:84-89,~286-299,~329-341` — merge the `#[seq]`/`#[opt]` marker aborts [dup-code]

- Evidence (condensed): the two abort bodies differ by exactly the substring `"single "`; message A
  (`peel` returns `None` — no followed head at all) is pinned verbatim by
  `core/tests/ui/visitor_edit_marker_unvisited.stderr`; message B ( `peel` succeeds but the resolved
  head isn't in `method_set` — an unlisted-intermediate or tuple head) is grepped **zero** times
  across every `core/tests/ui/*.stderr` (`grep -rn "single container of a type listed"
  core/tests/ui/` → empty) — genuinely unpinned by any UI fixture (confirmed: none of the 5
  `visitor_edit_marker_*.rs` fixtures reach message B; `_unvisited.rs` hits message A via a plain
  `Vec<String>` field with no followed head).
- **Exact diverging helper.** `marker_word` (`lower.rs:84-89`) is folded in (its only two callers are
  the two abort sites being merged):
  ```rust
  /// Abort: a `#[seq]`/`#[opt]`-marked field's element isn't a visited type. `single` distinguishes
  /// the two call sites: `false` when `peel` found no followed head at all (a leaf field, e.g.
  /// `Vec<String>`); `true` when a head *was* found but it resolves to an unlisted intermediate or a
  /// tuple, not a `visitor!(..)`-listed type. Only the `false` (first) wording is UI-pinned
  /// (`core/tests/ui/visitor_edit_marker_unvisited.stderr`) — the `true` wording is unpinned
  /// anywhere in the trybuild suite, so this merge is free to standardize on the pinned text plus one
  /// inserted word.
  fn abort_marker_not_visited(ty: &Type, kind: &Container, single: bool) -> ! {
      let marker = marker_word(kind);
      let extra = if single { "single " } else { "" };
      abort!(
          ty,
          "a `#[{}]` field's element type is not a visited type — mark only a field whose element is \
           a {}container of a type listed in `visitor!(..)` (or reached via `#[subast]`)",
          marker,
          extra
      );
  }
  ```
  (`marker_word` itself — `fn marker_word(kind: &Container) -> &'static str` — stays as a private
  helper called once inside `abort_marker_not_visited`, replacing its two direct call sites.)
- **Call-site rewrite**:
  ```diff
  // site A (peel == None, lower.rs ~286-299)
  -if let Some(kind) = view {
  -    let marker = marker_word(&kind);
  -    abort!(ty, "a `#[{}]` field's element type is not a visited type — mark only a field whose \
  -                element is a container of a type listed in `visitor!(..)` (or reached via \
  -                `#[subast]`)", marker);
  -}
  -return None;
  +if let Some(kind) = view {
  +    abort_marker_not_visited(ty, &kind, false);
  +}
  +return None;

  // site B (mutable marker branch, lower.rs ~329-341)
  -_ => abort!(ty, "a `#[{}]` field's element type is not a visited type — mark only a field whose \
  -              element is a single container of a type listed in `visitor!(..)` (or reached via \
  -              `#[subast]`)", marker),
  +_ => abort_marker_not_visited(ty, &kind, true),
  ```
- **Emission-visible change**: message B's wording changes (loses nothing pinned — it's unpinned) but
  its *text* stays character-identical to today's (the `abort_marker_not_visited(_, _, true)` call
  reproduces the exact current site-B string); this is a pure refactor, not a wording change, despite
  the task's request to double-check for "abort-message merge" risk — the two messages are **not**
  merged into one text, they're merged into one *generator* that reproduces both texts unchanged.
  So this finding, correctly implemented as specified above (site B keeps `single`), changes **zero**
  characters of emitted diagnostic text. (A more aggressive merge that also dropped `single` from site
  B, as one misreading of the plan's phrasing might suggest, would be safe too per the empty-grep
  check above — but is not necessary and not what "keep the wording of the first variant" implies:
  that phrase means "when in doubt about which of the two to standardize on, keep A's" — here both are
  kept via the `single` flag, so no doubt exists.)
- **Ordering/conflicts**: touches `lower.rs` lines ~84-89 (delete `marker_word`'s two direct call
  sites, keep the fn), ~286-299 and ~329-341. Finding #12 (comment cleanup) touches `lower.rs:145`
  (unrelated fn `visit_value`) and `~307-309`/`428` (between and after the two abort sites, but not
  overlapping them) — apply in either order relative to #4, no line conflicts.
- **Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test -p syan --test
  visitor_diagnostics -- --include-ignored` then `git diff core/tests/ui/*.stderr` — must be **empty**
  (no `TRYBUILD=overwrite` needed for this slice; message B has no existing pin to break).

### 5. `macro/visitor.rs:308-321,400-407` — dedup `lower`/`lower_mut` and the two `gen_side` calls [dup-code]

- Evidence (condensed): the two `Lower { .. }` struct literals (`visitor.rs:308-321`) are
  field-for-field identical except `mutable: false`/`true`; the two `gen_side(..)` calls
  (`visitor.rs:400-407`) are identical except the leading bool — both pure parameterization.
- **Code shape** — closure form for `Lower` (keeps two named bindings `lower`/`lower_mut` since both
  are used by name later at `visitor.rs:347,349-350`, so a `[..].map()` array form would need
  destructuring back into two names anyway; a closure is the smaller diff):
  ```rust
  let mk_lower = |mutable: bool| Lower {
      method_set: &method_set,
      done_by_path: &done_by_path,
      mutable,
      seq_used: &seq_used,
      opt_used: &opt_used,
  };
  let lower = mk_lower(false);
  let lower_mut = mk_lower(true);
  ```
  and for the `gen_side` pair — array-map form works cleanly here since both results are consumed only
  by name (`shared`, `mutable`) in the final `quote!`:
  ```rust
  let [shared, mutable] = [false, true].map(|m| {
      gen_side(
          m, &vtypes, &g_params, &g_args, &g_def, &g_use, &base_g_use, &ancestors, &st.base,
          &union_where, struct_only, &seq_used, &opt_used,
      )
  });
  ```
- **Emission**: unaffected — `mk_lower(false)`/`mk_lower(true)` and the array-map produce the exact
  same two `Lower` values / `gen_side` outputs as the two inlined call sites; no token-level change.
- **Ordering/conflicts**: touches `visitor.rs:308-321` (disjoint from #7's target `~152-159`) and
  `~400-407` (disjoint from #9's target `~415-427` and #10's targets `83,160,342-343,382`). Apply
  after #7 and #10 if doing a single combined `visitor.rs` pass (see slice-level ordering note below),
  but no line ranges actually overlap so order among {5,7,9,10} within `visitor.rs` is flexible.
- **Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace`.

### 6. `macro/visitor/entry.rs:87-98` + `build_input.rs` rest-bounce block — shared `state_tokens` serializer [dup-code, risk medium]

- Evidence (condensed): `entry.rs`'s `make_state` closure (87-98) and `build_input.rs`'s rest-bounce
  `quote!` (in `pub fn build`, the `if !st.rest.is_empty() { .. }` block) emit the same `@`-section
  skeleton; entry's is a strict subset (omits `@baseg`/`@anc`, which `BuildInput::parse` defaults to
  empty when the section is absent).
- **Exact diverging helper**:
  ```rust
  /// Serialize one `__visitor_build` ping-pong bounce's full state payload. Shared by `entry` (the
  /// first bounce — `inherited`/`base_generics`/`anc`/`done` are always empty, nothing fetched yet)
  /// and `build` (every later bounce, carrying the accumulated state). Content pieces that need
  /// their own rendering (`@base`, `@anc`, `@done`) are passed pre-rendered so this fn stays a pure
  /// section-list assembler.
  fn state_tokens(
      base: &TokenStream,             // base_tokens(&base_path) or quote!()
      build: &Path,
      nonce: &TokenStream,
      visited: &[Path],
      inherited: &[Ident],
      base_generics: &[GenericParam],
      anc: &TokenStream,              // emit_ancestors(&base_ancestors) or quote!()
      fetching: &TokenStream,
      done: &TokenStream,             // emit_done(&done) or quote!()
      rest: &[Path],
  ) -> TokenStream {
      quote! {
          @base { #base }
          @build { #build }
          @nonce { #nonce }
          @visited { #(#visited),* }
          @inherited { #(#inherited)* }
          @baseg { #(#base_generics),* }
          @anc { #anc }
          @fetching { #fetching }
          @done { #done }
          @rest { #(#rest),* }
      }
  }
  ```
- **`entry.rs` call site** (replaces `make_state`):
  ```rust
  let base_ts = base_tokens(&args.base);
  let make_state = |fetching: TokenStream, rest: &[Path]| {
      state_tokens(&base_ts, &build, &nonce, all_types, &[], &[], &quote!(), &fetching, &quote!(), rest)
  };
  ```
- **`build_input.rs` call site** (replaces the inline rest-bounce `quote!`):
  ```rust
  let base_ts = base_tokens(base);
  let done_ts = emit_done(done);
  let anc_ts = emit_ancestors(base_ancestors);
  let state = state_tokens(
      &base_ts, build, nonce, visited, inherited, base_generics, &anc_ts, &quote!(#next), &done_ts, rest,
  );
  return quote! { #next ! { @ast #build { #state } } };
  ```
- **Emission-visible change (the reason for "medium risk"):** `entry.rs`'s first-bounce payload today
  **omits** the `@baseg`/`@anc` sections entirely; after this merge it emits them as `@baseg { }`
  `@anc { }` (present, empty). This is a real token-stream change to an **intermediate**
  macro-to-macro payload — but it is not user-visible generated code: it is the private
  `__visitor_build` ping-pong argument, never part of any expanded item, doc comment, or public API,
  and no test snapshots it (confirmed: whole-worktree grep finds no consumer/test of this token shape
  other than `BuildInput::parse` itself). Safety: `BuildInput::parse`'s `"baseg" | "bg"` arm is
  guarded (`if !content.is_empty() { base_generics = .. }`) so an empty `@baseg { }` leaves
  `base_generics` at its already-empty default — a no-op; the `"anc" | "an"` arm is **unguarded**
  (`base_ancestors = parse_ancestors(content)?`) but `parse_ancestors` on empty input hits its
  `while !input.is_empty()` loop's false branch immediately and returns `Ok(vec![])` — also a no-op,
  and this exact empty-content path is already exercised today by `entry.rs`'s existing `@done { }`
  (also always present-but-empty on the first bounce) and by `emit_ancestors(&[])` on every no-base
  bounce elsewhere in the pipeline. So the change is behaviorally inert, just newly-present-but-empty
  sections in a private wire format.
- **Ordering/conflicts**: `build_input.rs` is also touched by #7 (`~377-382`), #8's caller
  (`~383,391`), and #13 (`~204`, `~365-366`) — all at line ranges strictly before this finding's
  target (the rest-bounce block is near the end of `pub fn build`, after the `just_def`-handling block
  #7/#8/#13 touch). No overlap; apply in the plan's listed order (6 before 7/8/13, per the raw
  finding list) or reorder freely — line ranges don't intersect either way.
- **Risk**: medium (per plan — the only non-mechanical risk in this slice, from the `@baseg`/`@anc`
  presence change above; verified safe). **Verify**: `cargo build -p syan-macro && cargo test
  --workspace`, with extra attention to `core/tests/visitor_inherit.rs` (mods `basic`/`arity`/
  `multilevel` — exercises multi-level `base => mid => New` inheritance, the only path that actually
  populates `@baseg`/`@anc` with non-empty content and round-trips through both `entry` and `build`)
  and `rust/tests/cross_crate_inherit*.rs`.

### 7. `macro/visitor/build_input.rs:377-382` — extract `BuildInput::method_set()` [dup-code]

- Evidence (condensed): `build_input.rs:377-382` and `visitor.rs:155-159` (confirmed exact line
  numbers via grep) both compute `HashSet<String>` = `st.visited` mapped through
  `last_ident(..).to_string()` chained with `st.inherited` idents-as-strings.
- **Code shape**:
  ```rust
  impl BuildInput {
      /// Visited types' last-idents ∪ inherited idents — the set of heads that dispatch via a
      /// `visit_*` method call rather than being drilled/leaf.
      fn method_set(&self) -> HashSet<String> {
          self.visited
              .iter()
              .map(|p| last_ident(p).to_string())
              .chain(self.inherited.iter().map(|i| i.to_string()))
              .collect()
      }
  }
  ```
- **`build_input.rs:377-382` call site**: `let method_set = st.method_set();` (was the 5-line
  expression, now reading `self.visited`/`self.inherited` instead of `st.visited`/`st.inherited` — same
  fields, same set).
- **`visitor.rs:155-159` call site**: currently builds from a *different* intermediate
  (`visited: HashSet<String> = path_of.keys().cloned().collect()`, itself derived from `st.visited`)
  chained with `st.inherited`. Since `path_of`'s keys are exactly `last_ident(p).to_string()` for each
  `p` in `st.visited` (built two lines earlier as `path_of: HashMap<String, &Path> = st.visited.iter()
  .map(|p| (last_ident(p).to_string(), p)).collect()`), `visited.iter().cloned().chain(..)` and
  `st.method_set()` are the same set — replace with `let method_set = st.method_set();`.
- **Emission**: unaffected (both sites already produce the identical `HashSet<String>`; the extraction
  changes no downstream consumer, e.g. `Lower { method_set: &method_set, .. }` at `visitor.rs:309,316`
  is untouched).
- **Ordering/conflicts**: `build_input.rs` region (~377-382) is immediately followed by #8's caller
  edit (~383,391 — `self_ident`) in the same `if let Some(def) = ..` block; apply #7 first so #8's
  diff lands cleanly against the already-shrunk block. `visitor.rs`'s half touches ~155-159, adjacent
  to but not overlapping #10's comment deletion at line 160 (which follows immediately after and can
  be deleted in the same pass without conflict, since it's the *next* line, not a shared line).
- **Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace`.

### 8. `macro/visitor/discover.rs:12-15` — reuse `self_and_subast_keys` [dup-code]

- Evidence (condensed): `discover.rs:12-15`'s inline `HashSet` build is the same construction as
  `self_and_subast_keys` (`params.rs:34-40`, confirmed exact line match) modulo `&str` vs `&Ident`;
  `self_and_subast_keys` has exactly one existing caller (`lower.rs:283`, confirmed via grep).
- **Code shape**: change `followed_intermediates`'s and `discover_followed`'s `self_ident` parameter
  from `Option<&str>` to `Option<&Ident>`:
  ```diff
   pub(crate) fn followed_intermediates(
       def: &Item,
       subast: &[SubEntry],
       method_set: &HashSet<String>,
  -    self_ident: Option<&str>,
  +    self_ident: Option<&Ident>,
   ) -> Vec<Path> {
  -    let mut user_types: HashSet<String> = subast.iter().map(|e| e.key.to_string()).collect();
  -    if let Some(s) = self_ident {
  -        user_types.insert(s.to_string());
  -    }
  +    let user_types = self_and_subast_keys(self_ident, subast);
       ...
   }
  ```
  and in `discover_followed` (signature `self_ident: Option<&Ident>` too), the compare at
  `discover.rs:44` becomes an `Ident` compare:
  ```diff
  -let hs = head.to_string();
  -if Some(hs.as_str()) == self_ident {
  +if Some(head) == self_ident {
       return; // self -> already in `done`
   }
  ```
  (`hs` is dropped entirely — its only other use, `subast.iter().find(|e| &e.key == head)`, already
  uses `head` directly, not `hs`.)
- **Caller update** (`build_input.rs:383,391`):
  ```diff
  -let self_ident = item_ident(&def).map(|i| i.to_string());
  +let self_ident = item_ident(&def);
   ...
  -followed_intermediates(&def, &subast, &method_set, self_ident.as_deref())
  +followed_intermediates(&def, &subast, &method_set, self_ident)
  ```
  (`item_ident(&def)` already returns `Option<&Ident>` — the `.map(|i| i.to_string())`/`.as_deref()`
  pair was only there to satisfy the old `Option<&str>` signature.)
- **Emission**: unaffected — pure internal set-construction refactor, no generated tokens involved.
- **Ordering/conflicts**: `discover.rs` is touched by no other finding in this slice. The
  `build_input.rs` caller edit sits at `~383,391`, immediately after #7's `~377-382` — apply #7 then
  #8 as one contiguous pass over that block.
- **Risk**: low. **Verify**: `cargo build -p syan-macro && cargo test --workspace` (drill-in behavior
  specifically covered by `core/tests/visitor_drill*.rs`).

### 9. `macro/visitor/side.rs:317-320` (+ `visitor.rs` final quote) — hoist the `use ::syan::visit::{OptView as _, SeqView as _}` [emit-reduction]

- Evidence (condensed): only the free-fn bodies (`side.rs:307-326`, embedding `s.body`/`s.body_mut`
  from `Lower::destructure`, which calls `fold_containers` → bare `.view_iter()`/`.view_iter_mut()`
  method calls) need the traits nameable in scope; `trait_def`'s seq/opt defaults and
  `blanket_ref_impl`'s forwarders use fully-qualified UFCS (`::syan::visit::SeqView::view_iter_mut(v)`
  at the former `side.rs:265`, now inside `ViewSpec::default_body` per finding #3; and
  `<#p_v as #visit_tr #g_use>::method(..)` in `blanket_ref_impl`) — confirmed by reading every call
  site in `side.rs`. `visitor!(..)` is documented as invoked inside an (initially empty) `mod`, and
  `generate_module`'s final `quote!` (`visitor.rs:415-427`) already emits ancestor-trait `use`s
  directly into that module.
- **Code shape** — delete from `free_fns` (`side.rs:317-320`):
  ```diff
   pub fn #{&s.method}< .. >(this: &mut #p_v, i: #amp #{&s.ty}) #{&s.free_where} {
  -    #[allow(unused_imports)]
  -    use ::syan::visit::{OptView as _, SeqView as _};
       #{&s.body}
   }
  ```
  add once in `generate_module`'s final `quote!` (`visitor.rs`, next to the ancestor `use` loop):
  ```diff
   quote! {
       #visited_macro

       #(for a in &ancestors) {
           #[allow(unused_imports)]
           use #{&a.path}::{Visit as _, VisitMut as _};
       }

  +    #[allow(unused_imports)]
  +    use ::syan::visit::{OptView as _, SeqView as _};
  +
       #shared
       #mutable
   }
  ```
- **Why this is safe despite changing emission**: `#shared`/`#mutable` (the `gen_side` outputs) are
  spliced as *direct items of the same enclosing module* — not nested in a sub-`mod` or a `fn` body —
  so every free fn generated inside them sits in the same lexical scope as the hoisted `use`. Rust
  resolves an unqualified trait method call (`.view_iter_mut()`) by scanning traits in scope at *any*
  lexically enclosing level, function-local or module-level; a module-level `use Trait as _` is visible
  to every function textually inside that module exactly as if each function repeated it locally. Since
  `as _` binds no name, one copy vs. `2 × (#visited types)` copies cannot conflict or shadow anything.
  The **only** call sites that need the trait nameable (`view_iter`/`view_iter_mut`, unqualified) are
  the free-fn bodies — confirmed no other generated item calls these methods unqualified (`trait_def`'s
  seq/opt defaults and `blanket_ref_impl`'s forwarders are fully UFCS/associated-fn calls, immune to
  scope).
- **Net emission delta**: expansion shrinks by `2 × (#visited types) − 1` `use` lines net (was `2 ×
  N`, now `1`) plus one `#[allow(unused_imports)]`; this is an intentional, stated emission change
  (unlike #1–#8, which are token-identical).
- **Ordering/conflicts**: `side.rs` half (~307-326) is disjoint from #3's target (~55-171,245-305) and
  #11's targets (178,244,257-258) — apply after #3 (dup-code-before-emit-reduction convention; no
  actual line dependency). `visitor.rs` half (~415-427) is disjoint from #5 (~400-407) and #10
  (83,160,342-343,382) — safe in any order.
- **Risk**: low (no `.stderr` surface; `use ... as _` never appears in a diagnostic). **Verify**:
  `cargo build -p syan-macro && cargo test --workspace`, plus a spot check that
  `-D unused-imports`/clippy stays clean: `cargo clippy -p syan-macro -p syan -- -D warnings` (per the
  plan's overall post-slice gate).

### 10. `macro/visitor.rs:83,160,342-343,382` — delete narration comments [comment]

- Evidence (condensed): all four restate the adjacent line or a field doc elsewhere, confirmed by
  direct read: L83 "Recurse into every type argument…" sits directly above the `for arg in &ab.args`
  loop it restates; L160 "Fetched types keyed by full path…" duplicates `Lower::done_by_path`'s doc
  (`lower.rs:98-99`); L342-343 restates the `let scrut_path = path_of.get(..).unwrap_or(&d.path);`
  line immediately below (confirmed at current line 344, one line off the plan's citation — same
  content); L382 "The mut walk has finished…" narrates `seq_used.into_inner()`/`opt_used.into_inner()`.
- **Code shape**: four straight deletions, no logic change.
- **Emission**: unaffected — doc/line comments in macro source never appear in generated tokens.
- **Ordering/conflicts**: L160's deletion is adjacent to #7's `method_set` extraction (which ends at
  what becomes the new line ~156) — apply #7 first so #10 deletes the correct (shifted) line. L342-343
  is untouched by any other finding. L83/L382 are in `has_concrete_fill`/`generate_module`, regions no
  other finding here touches.
- **Risk**: low. **Verify**: `cargo build -p syan-macro` (comment-only; `cargo test --workspace` as
  the slice-level gate, not specifically required per-finding).

### 11. `macro/visitor/side.rs:178,~244,257-258` — delete stale/duplicate comments [comment]

- Evidence (condensed): "// Generated API docs." (current line **178**, plan cites 179 — 1-line
  drift, same content) is a bare section header adding nothing; "the final `quote!` splices them
  verbatim…" (**244**, part of a 2-line comment starting at 243) narrates a past refactor (the
  named-token-block-assembly style itself), not a live constraint; the in-quote comment "Opt-in
  container-edit hooks… Default: descend each held node via `visit_*_mut`" (**257-258**) duplicates
  the `seq_doc`/`opt_doc` text generated two lines below it (and, after finding #3, becomes `ViewSpec`
  doc text one level further down).
- **Code shape**: three deletions. Rationale comments explicitly kept per the plan (struct_only doc at
  16-19, `?Sized` note at 310-311, ancestor `Driver` impls note at 356-358) are untouched.
- **Emission**: unaffected.
- **Ordering/conflicts**: L257-258 sits directly above the `has_seq`/`has_opt` blocks that #3 replaces
  with the `views` loop — **apply #3 before #11** so #11's deletion targets the post-restructure
  location (the comment would otherwise sit above `#(for spec in &s.views)` — still applicable to
  delete, just at a shifted position). L178 and L243-244 are outside #3's/#9's touched regions — safe
  in any order relative to those.
- **Risk**: low. **Verify**: `cargo build -p syan-macro`.

### 12. `macro/visitor/lower.rs:145,~307-309,428` — delete/trim narration comments [comment]

- Evidence (condensed): L145 "Unlisted intermediate -> inline drill." (confirmed exact) restates what
  `Lower`'s struct doc already says (`lower.rs:91-94`); L307-309 (plan cites 308-310 — 1-line drift)
  "Dispatch at the innermost… and now `Vec<(A, B)>`" mixes a live one-sentence fact ("An empty body …
  ⇒ the whole field is a leaf") with historical narration ("and now") — trim to keep only the live
  sentence; L428 trailing `// tuple of only leaves -> leaf (empty body)` restates the early return and
  `lower_tuple`'s doc (`391-392`).
- **Code shape**:
  ```diff
  -        // Unlisted intermediate -> inline drill.
           let key = norm_path(drill_path);
  ```
  ```diff
  -        // Dispatch at the innermost (container-peeled) accessor, then wrap the container chain
  -        // (handles nested containers like `Vec<Option<T>>`, and now `Vec<(A, B)>`). An empty body
  -        // (a leaf head, or a finite drill reaching nothing) ⇒ the whole field is a leaf.
  +        // An empty body (a leaf head, or a finite drill reaching nothing) ⇒ the whole field is a leaf.
           let acc = innermost_acc(&p.conts, binding);
  ```
  ```diff
  -            return quote!(); // tuple of only leaves -> leaf (empty body)
  +            return quote!();
  ```
- **Emission**: unaffected.
- **Ordering/conflicts**: L145 sits in `visit_value` (untouched by #4). L307-309 sits between #4's two
  abort sites (~286-299 and ~329-341) but does not overlap either. L428 is in `lower_tuple`, after
  every other finding's target. No conflicts with #4; apply in either order.
- **Risk**: low. **Verify**: `cargo build -p syan-macro`.

### 13. `macro/visitor/build_input.rs:204,365-366` — delete duplicate-rationale comments [comment]

- Evidence (condensed): L204 (plan cites 203 — 1-line drift) `// crate::REST -> <host>::REST (replace
  only the leading segment)` duplicates the rule already spelled out in `requalify_ancestor`'s doc
  comment (`170-182`, specifically the bullet at 175); L365-366 `// Record the just-fetched type (def +
  subast) under the path it was fetched by, then discover the followed-but-unvisited intermediates it
  references and enqueue them for drilling.` narrates the visibly-named code immediately below
  (`st.done.push(..)`, the `followed_intermediates(..)` loop pushing to `st.rest`) without adding a
  constraint.
- **Code shape**: two deletions (L204: one line inside the `"crate" => { .. }` arm of
  `requalify_ancestor`; L365-366: the two-line comment immediately preceding `if let Some(def) =
  st.just_def.take() { .. }` in `pub fn build`).
- **Emission**: unaffected.
- **Ordering/conflicts**: L204 is in `requalify_ancestor`, untouched by #6/#7/#8. L365-366 is the very
  first thing inside the block that #7 (`~377-382`) and #8's caller (`~383,391`) also touch — apply
  #13's comment deletion first (or last; it's a pure comment removal at the top of the block, doesn't
  shift the meaning of the code #7/#8 edit below it) — no functional conflict either way, only a minor
  textual-diff-ordering convenience in doing it first.
- **Risk**: low. **Verify**: `cargo build -p syan-macro`.

### 14. `macro/visitor/params.rs:6` — trim stale half-sentence from `param_union`'s doc [comment]

- Evidence (condensed): confirmed exact line match. `grep -rn "param_union"` across the whole worktree
  finds exactly two hits: the definition (`params.rs:7`) and its one caller (`visitor.rs:179`) — no
  recurse-path code (`macro/recurse*.rs`) calls it, so "the recurse path additionally filters this to
  the cycle roots' params" is stale (recurse-over-visitor is now an ordinary acyclic visitor per
  CLAUDE.md — no depth/param-filtering machinery survives).
- **Code shape**:
  ```diff
   /// The deduped union of every target's generic params (first declaration wins), followed by the
   /// base's params (for inheritance — the new trait must declare them to name `base::Visit<base params>`
  -/// as a supertrait, so the new union must ⊇ the base's). The caller normalizes order with
  -/// `sort_lifetimes_first`; the recurse path additionally filters this to the cycle roots' params.
  +/// as a supertrait, so the new union must ⊇ the base's). The caller normalizes order with
  +/// `sort_lifetimes_first`.
   pub(crate) fn param_union(targets: &[&DoneType], base_generics: &[GenericParam]) -> Vec<GenericParam> {
  ```
- **Emission**: unaffected.
- **Ordering/conflicts**: `params.rs` is touched by no other finding in this slice.
- **Risk**: low. **Verify**: `cargo build -p syan-macro`.

---

**Slice-level verification.** After applying all 14 in the order above:
`cargo build -p syan-macro && cargo test --workspace && cargo clippy --workspace -- -D warnings`.
Only finding **#4** touches trybuild-adjacent code and only finding **#6** touches an
emission-affecting-but-unsnapshotted wire format; run `git diff core/tests/ui/*.stderr` after the
`visitor_diagnostics` test — it must come back **empty**. No `.stderr` file in this slice legitimately
needs `TRYBUILD=overwrite`.
