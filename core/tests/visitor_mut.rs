//! Stage 8: `visit_mut` mirror — mutate AST nodes in place via closures and struct visitors.

use core::marker::PhantomData;
use syan::visit::Ast;

#[derive(Debug, Ast)]
pub enum Expr<S> {
    Add(Box<Expr<S>>, Box<Expr<S>>),
    Lit(i64, PhantomData<S>),
}

pub mod visit {
    syan::visit::visitor!(super::Expr);
}

fn sample() -> Expr<()> {
    Expr::Add(
        Box::new(Expr::Lit(1, PhantomData)),
        Box::new(Expr::Add(
            Box::new(Expr::Lit(2, PhantomData)),
            Box::new(Expr::Lit(3, PhantomData)),
        )),
    )
}

fn sum(e: &Expr<()>) -> i64 {
    let mut s = 0;
    e.visit(|x: &Expr<()>| {
        if let Expr::Lit(n, _) = x {
            s += *n;
        }
    });
    s
}

#[test]
fn mut_closure_increments_literals() {
    let mut ast = sample();
    ast.visit_mut(|x: &mut Expr<()>| {
        if let Expr::Lit(n, _) = x {
            *n += 1;
        }
    });
    assert_eq!(sum(&ast), (1 + 1) + (2 + 1) + (3 + 1));
}

#[test]
fn struct_mut_visitor_doubles() {
    struct Doubler;
    impl<S> visit::VisitMut<S> for Doubler {
        fn visit_expr_mut(&mut self, i: &mut Expr<S>) {
            if let Expr::Lit(n, _) = i {
                *n *= 2;
            }
            visit::visit_expr_mut(self, i);
        }
    }
    let mut ast = sample();
    ast.visit_mut(&mut Doubler);
    assert_eq!(sum(&ast), 2 + 4 + 6);
}
