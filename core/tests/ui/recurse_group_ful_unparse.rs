// KNOWN LIMITATION (#1, deferred): `Unparse`/`Spanned` on the NATURAL type of a **group-ful**
// `#[recurse]` cycle. The natural `Expr<S>` is `Parse` (delegated through the depth-limited engine),
// but a cycle with a `#[group(self.brace)]` field keeps `Unparse`/`Spanned` on the `pub(crate)` engine
// only — they are NOT emitted on the natural type. (A group-FREE cycle does get them: directly for a
// single self-recursive cycle, or via the `__FromNat` engine delegation for a multi-type one — see
// `recurse_unparse_spanned.rs`.) So calling `.unparse()` on a group-ful natural `Expr` fails to resolve.
//
// Why deferred: the engine's group `Unparse` carries a `for<'a> <GroupBrace<…> as EmptyGroup>::Fill<
// Substruct<…>>: Unparse` HRTB bound whose transitive obligations can't be discharged from — or named
// in — a delegated impl (the `Substruct` is a derive-internal, nonce-named type). Lifting it needs a
// derive-level rework. See `docs/recurse-deferred-fixes-plan.md` §1 and CLAUDE.md "Known gaps".
//
// This test pins the limitation: if a future fix makes group-ful natural `Unparse` work, this stops
// failing and should be promoted to a passing round-trip test.
use syan::parse::{recurse, Parse, Unparse};
use template_quote::quote;

#[recurse]
mod grp {
    use syan::nested::group::GroupBrace;
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;

    #[derive(Parse, Unparse)]
    pub enum Expr<S> {
        Lit(Integer),
        Block {
            brace: GroupBrace<(), S>,
            #[group(self.brace)]
            inner: Vec<Expr<S>>,
        },
    }
}

fn main() {
    // Parse works (delegated through the engine)...
    let e: grp::Expr<_> = Parse::parse(quote! { { 1 } }).unwrap();
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    // ...but `Unparse` is engine-only for a group-ful cycle, so this does not resolve on the natural type.
    e.unparse(&mut (&mut out)).unwrap();
}
