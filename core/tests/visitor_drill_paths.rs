//! Two distinct unlisted intermediates that share a last segment (`a::Cast` and `b::Cast`) must
//! both be fetched (fetch-dedup is on the *full* resolved path, not the last segment) and drilled to
//! their own `Type`. The `#[subast]` collision is disambiguated by aliasing one entry and writing
//! the field with that alias.

use core::marker::PhantomData;
use syan::visit::Ast;

#[derive(Debug, Ast)]
pub enum Type<S> {
    Unit(PhantomData<S>),
}

pub mod a {
    use syan::visit::Ast;
    #[derive(Debug, Ast)]
    #[subast(crate::Type)]
    pub struct Cast<S>(pub crate::Type<S>);
}

pub mod b {
    use syan::visit::Ast;
    #[derive(Debug, Ast)]
    #[subast(crate::Type)]
    pub struct Cast<S>(pub crate::Type<S>);
}

use a::Cast;
use b::Cast as BCast;

#[derive(Debug, Ast)]
#[subast(crate::a::Cast, crate::b::Cast as BCast)]
pub enum Expr<S> {
    A(Cast<S>),
    B(BCast<S>),
}

pub mod visit {
    // `a::Cast` and `b::Cast` are both unlisted intermediates; only `Type` (shared) is visited.
    syan::visit::visitor!(super::Expr, super::Type);
}

#[test]
fn distinct_same_named_intermediates_both_drilled() {
    let ast: Expr<()> = Expr::A(Cast(Type::Unit(PhantomData)));
    let mut n = 0usize;
    ast.visit(|_t: &Type<()>| n += 1);
    assert_eq!(n, 1, "drilled through a::Cast");

    let ast: Expr<()> = Expr::B(BCast(Type::Unit(PhantomData)));
    let mut n = 0usize;
    ast.visit(|_t: &Type<()>| n += 1);
    assert_eq!(n, 1, "drilled through b::Cast (fetched distinctly from a::Cast)");
}
