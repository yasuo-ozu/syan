// `Unparse` on a GROUP-FUL `#[recurse]` cycle: the natural type DOES get a delegated `Unparse` impl
// (natural → engine `__FromNat` → engine's group `Unparse`, exactly like a multi-type group-free cycle).
// So this is NOT a `#[recurse]` limitation. What still fails is a **library-level** leaf gap shared with
// any non-`#[recurse]` group type: a brace/delimiter *symbol* only `Unparse`s to an atom that is
// `From<String> + AtomParsedToAllChars`, and the usual proc-macro atom `proc_macro2::TokenTree` is not
// one. Hence `OpenBrace: Unparse<TokenTree>` is unsatisfied.
//
// This file pins exactly that: the SAME `OpenBrace: Unparse<TokenTree>` error arises for a plain
// (non-recurse) group struct `Plain` AND for the recurse cycle `grp::Expr` — demonstrating that
// `#[recurse]`'s delegation adds no extra limitation. If the library later lets symbols unparse to
// `TokenTree` (or ships a `From<String>` atom), both stop failing together.
use syan::nested::group::GroupBrace;
use syan::parse::{recurse, Parse, Unparse};
use syan::source::proc_macro2::literal::Integer;

// (1) plain, NON-recurse group type — already cannot unparse to `TokenTree`.
#[derive(Parse, Unparse)]
pub struct Plain<S> {
    brace: GroupBrace<(), S>,
    #[group(self.brace)]
    inner: Vec<Integer>,
}

// (2) group-ful recurse cycle — the natural `Expr` HAS a delegated `Unparse` impl; it fails to resolve
// for `TokenTree` for the *same* leaf reason, not for lack of an impl.
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

fn assert_unparse<T: Unparse<proc_macro2::TokenTree>>() {}

fn main() {
    assert_unparse::<Plain<proc_macro2::Span>>(); // library-level fail (non-recurse)
    assert_unparse::<grp::Expr<proc_macro2::Span>>(); // same library-level fail (recurse, delegated)
}
