//! PROBE: an unlisted intermediate `Cast` reached from two visited types via DIFFERENT path
//! spellings (`crate::Cast` from Expr, `super::Cast` from Stmt). Tests dedup/lookup by norm_path.

use core::marker::PhantomData;
use syan::visit::Ast;

#[derive(Debug, Ast)]
pub enum Type<S> {
    Unit(PhantomData<S>),
}

#[derive(Debug, Ast)]
#[subast(crate::Type)]
pub struct Cast<S>(pub Type<S>);

// Expr references Cast as `crate::Cast`
#[derive(Debug, Ast)]
#[subast(crate::Cast)]
pub enum Expr<S> {
    C(Cast<S>),
    Lit(PhantomData<S>),
}

// Stmt references the SAME Cast but spelled `super::Cast` (a module makes super resolve here)
pub mod inner {
    use super::Cast;
    use core::marker::PhantomData;
    use syan::visit::Ast;
    #[derive(Debug, Ast)]
    #[subast(super::Cast)]
    pub enum Stmt<S> {
        C(Cast<S>),
        Lit(PhantomData<S>),
    }
}
use inner::Stmt;

pub mod visit {
    // Expr and Stmt visited; Type visited; Cast is the unlisted intermediate reached two ways.
    syan::visit::visitor!(super::Expr, super::inner::Stmt, super::Type);
}

#[test]
fn cast_reached_via_two_spellings() {
    let e: Expr<()> = Expr::C(Cast(Type::Unit(PhantomData)));
    let s: Stmt<()> = Stmt::C(Cast(Type::Unit(PhantomData)));
    let mut types = 0usize;
    e.visit(|_t: &Type<()>| types += 1);
    s.visit(|_t: &Type<()>| types += 1);
    assert_eq!(types, 2);
}
