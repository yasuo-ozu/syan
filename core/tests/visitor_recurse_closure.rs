//! `<AST>.visit(closure)` on an AST type **defined in a `#[recurse]` module**.
//!
//! A `#[recurse]` module may hold *acyclic* types alongside the recurse cycle; `#[recurse]` passes the
//! acyclic ones through untouched (normal `#[derive(Ast)]`). A `visitor!()` over those acyclic types is
//! therefore an ordinary visitor — so the **closure** inputs work, including the inherent
//! `.visit(closure)` and a **tuple of closures** in one pass. (The recurse *cycle* types themselves
//! can't be visited by a closure: their `visit_*<R>` methods are depth-generic, which a closure can't
//! be — use a struct `Visit` impl, see `visitor_recurse_via_visitor.rs`. A cycle-typed field of an
//! acyclic type is simply a leaf for this visitor, as `body` below shows.)
#![allow(dead_code)]

use core::marker::PhantomData;

#[syan::parse::recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    // The recurse cycle (renamed + depth-threaded by `#[recurse]`).
    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        Nest(Box<Expr<S>>),
        Lit(PhantomData<S>),
    }

    // Acyclic types in the SAME module — untouched by `#[recurse]`, normal `#[derive(Ast)]`.
    #[derive(Ast)]
    #[subast()]
    pub enum Type<S> {
        Int(PhantomData<S>),
        Bool(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast(crate::ast::Param, crate::ast::Type)]
    pub struct Decl<S> {
        pub params: Vec<Param<S>>,
        pub ret: Type<S>,
        // A recurse-cycle node as a field: NOT in `#[subast]`, so it's a leaf for the acyclic visitor.
        pub body: Expr<S>,
    }

    // Drilled-through intermediate (unlisted): Param -> Type.
    #[derive(Ast)]
    #[subast(crate::ast::Type)]
    pub struct Param<S> {
        pub ty: Type<S>,
    }
}

mod v {
    // A visitor over the ACYCLIC types in the recurse module — closures supported.
    syan::visit::visitor!(crate::ast::Decl, crate::ast::Type);
}

fn sample() -> ast::Decl<()> {
    ast::Decl {
        params: vec![
            ast::Param { ty: ast::Type::Int(PhantomData) },
            ast::Param { ty: ast::Type::Bool(PhantomData) },
        ],
        ret: ast::Type::Int(PhantomData),
        body: ast::Expr::Lit(PhantomData), // a recurse-cycle node; a leaf for this acyclic visitor
    }
}

#[test]
fn single_closure() {
    let d = sample();
    let mut types = 0usize;
    // inherent `.visit()` with a single closure; drills Decl -> Param -> Type and Decl.ret.
    d.visit(|_t: &ast::Type<()>| types += 1);
    assert_eq!(types, 3, "two param Types (drilled through Param) + the ret Type");
}

#[test]
fn tuple_of_closures() {
    let d = sample();
    let mut decls = 0usize;
    let mut types = 0usize;
    // two closures (over Decl and Type) running in one traversal
    d.visit((
        |_d: &ast::Decl<()>| decls += 1,
        |_t: &ast::Type<()>| types += 1,
    ));
    assert_eq!((decls, types), (1, 3));
}
