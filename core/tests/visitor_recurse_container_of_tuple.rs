//! AUDIT finding (recurse path): a tuple nested inside a container holding a recursive ref
//! (`Vec<(Box<Expr<S>>, Box<Expr<S>>)>`) was silently skipped — the same missing `Type::Tuple` arm in
//! `peel`. With the natural-type design the cycle is acyclic and the visitor is ordinary; this pins
//! the tuple-in-container traversal.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::recurse;
use syan::visit::Ast;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        Pair((Box<Expr<S>>, Box<Expr<S>>)),          // control: top-level tuple back-edges
        VecPair(Vec<(Box<Expr<S>>, Box<Expr<S>>)>),  // bug: tuple back-edges inside a Vec
        OptPair(Option<(Box<Expr<S>>, Box<Expr<S>>)>), // bug: tuple back-edges inside an Option
        Lit(PhantomData<S>),
    }
}

mod v {
    syan::visit::visitor!(crate::ast::Expr);
}

#[derive(Default)]
struct Counter {
    e: usize,
}
impl<S> v::Visit<S> for Counter {
    fn visit_expr(&mut self, i: &ast::Expr<S>) {
        self.e += 1;
        v::visit_expr(self, i);
    }
}

#[test]
fn recurse_vec_of_tuple_back_edges() {
    // VecPair with two pairs ⇒ outer(1) + 4 inner Lits = 5. Inner nodes use the natural type path.
    let e: ast::Expr<()> = ast::Expr::VecPair(vec![
        (Box::new(ast::Expr::Lit(PhantomData)), Box::new(ast::Expr::Lit(PhantomData))),
        (Box::new(ast::Expr::Lit(PhantomData)), Box::new(ast::Expr::Lit(PhantomData))),
    ]);
    let mut c = Counter::default();
    v::Visit::visit_expr(&mut c, &e);
    assert_eq!(c.e, 5, "outer Expr + 4 tuple-nested back-edges");
}

#[test]
fn recurse_opt_of_tuple_back_edges() {
    // OptPair(Some) ⇒ outer(1) + 2 inner Lits = 3
    let e: ast::Expr<()> = ast::Expr::OptPair(Some((
        Box::new(ast::Expr::Lit(PhantomData)),
        Box::new(ast::Expr::Lit(PhantomData)),
    )));
    let mut c = Counter::default();
    v::Visit::visit_expr(&mut c, &e);
    assert_eq!(c.e, 3, "outer Expr + 2 tuple-nested back-edges");
}
