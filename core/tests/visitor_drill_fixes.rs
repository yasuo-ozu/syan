//! Regression tests for drill-in bugs found by review:
//!  * an aliased `#[subast]` entry that denotes a *visited* type dispatches to its real `visit_*`
//!    method (and is not double-fetched),
//!  * a user AST type named like a container keyword (`Option`) is treated as a node, not a std
//!    container,
//!  * a `Box<Option<T>>` field's `if let Some(..)` derefs through the `Box`.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::visit::Ast;

// ── Aliased #[subast] entry on a VISITED type ────────────────────────────────────────────────────

mod other {
    use core::marker::PhantomData;
    use syan::visit::Ast;
    #[derive(Debug, Ast)]
    pub enum Real<S> {
        U(PhantomData<S>),
    }
}

use other::Real;
use other::Real as Aliased;

#[derive(Debug, Ast)]
#[subast(crate::other::Real as Aliased)]
pub enum Expr<S> {
    R(Aliased<S>),
    Lit(PhantomData<S>),
}

pub mod va {
    // `Real` is visited but the field references it through the alias `Aliased`.
    syan::visit::visitor!(crate::Expr, crate::other::Real);
}

#[test]
fn aliased_subast_entry_dispatches_to_real_method() {
    let ast: Expr<()> = Expr::R(Aliased::U(PhantomData));
    let mut reals = 0usize;
    ast.visit(|_r: &Real<()>| reals += 1);
    assert_eq!(reals, 1, "field `Aliased` lowered to visit_real, no duplicate fetch");
}

// ── A user AST type named `Option` ───────────────────────────────────────────────────────────────

#[derive(Debug, Ast)]
pub enum Leaf<S> {
    U(PhantomData<S>),
}

mod weird {
    use super::Leaf;
    use syan::visit::Ast;
    // A user type whose name collides with the `Option` container keyword.
    #[derive(Debug, Ast)]
    #[subast(crate::Leaf)]
    pub struct Option<S>(pub Leaf<S>);
}

#[derive(Debug, Ast)]
#[subast(crate::weird::Option)]
pub enum Outer<S> {
    O(weird::Option<S>),
    Lit(PhantomData<S>),
}

pub mod vw {
    // `weird::Option` is an unlisted intermediate drilled through to the visited `Leaf`.
    syan::visit::visitor!(crate::Outer, crate::Leaf);
}

#[test]
fn user_type_named_like_a_container_is_a_node() {
    let ast: Outer<()> = Outer::O(weird::Option(Leaf::U(PhantomData)));
    let mut leaves = 0usize;
    ast.visit(|_l: &Leaf<()>| leaves += 1);
    assert_eq!(leaves, 1, "drilled through the user `Option` type to its Leaf");
}

// ── Box<Option<T>>: `if let` must deref through the Box ──────────────────────────────────────────
// (A distinct leaf type `LeafB`, since two visitors over the same type in one crate would emit
// duplicate inherent `visit`/`visit_mut`.)

#[derive(Debug, Ast)]
pub enum LeafB<S> {
    U(PhantomData<S>),
}

#[derive(Debug, Ast)]
#[subast(crate::LeafB)]
pub struct Holder<S> {
    pub boxed_opt: Box<Option<LeafB<S>>>,
}

pub mod vh {
    syan::visit::visitor!(crate::Holder, crate::LeafB);
}

#[test]
fn box_around_option_derefs_in_if_let() {
    let some: Holder<()> = Holder {
        boxed_opt: Box::new(Some(LeafB::U(PhantomData))),
    };
    let mut n = 0usize;
    some.visit(|_l: &LeafB<()>| n += 1);
    assert_eq!(n, 1, "visited the LeafB inside Box<Option<_>>");

    let none: Holder<()> = Holder {
        boxed_opt: Box::new(None),
    };
    let mut n = 0usize;
    none.visit(|_l: &LeafB<()>| n += 1);
    assert_eq!(n, 0);
}

#[test]
fn box_around_option_mut_side() {
    struct C(usize);
    impl<S> vh::VisitMut<S> for C {
        fn visit_leaf_b_mut(&mut self, i: &mut LeafB<S>) {
            self.0 += 1;
            vh::visit_leaf_b_mut(self, i);
        }
    }
    let mut h: Holder<()> = Holder {
        boxed_opt: Box::new(Some(LeafB::U(PhantomData))),
    };
    let mut c = C(0);
    h.visit_mut(&mut c);
    assert_eq!(c.0, 1);
}
