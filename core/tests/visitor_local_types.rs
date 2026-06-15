//! A `#[derive(Ast)]` type with a field whose type is local / only imported in the AST module's
//! context. This works in the current pipeline because such a field is a *leaf* (its head ident is
//! not a visited type): it is bound to `_` and never named in generated code, so its (non-portable)
//! type is never resolved. type-leak would be required only once a field *type* (not just its head
//! ident) is emitted into the generated visitor — e.g. faithful re-emission, or drill-in that must
//! resolve a wrapper's field type rather than match it by head ident.

#![allow(dead_code)]

mod helper {
    #[derive(Debug)]
    pub struct Local;
}

mod ast {
    use super::helper::Local; // imported only in this module's context
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Debug, Ast)]
    pub enum Expr<S> {
        Lit(Local, PhantomData<S>), // field type valid only here
        Rec(Box<Expr<S>>),
    }
}

mod vis {
    syan::visit::visitor!(super::ast::Expr);
}

#[test]
fn local_leaf_field_type() {
    use ast::Expr;
    use helper::Local;
    let tree: Expr<()> = Expr::Rec(Box::new(Expr::Lit(Local, core::marker::PhantomData)));
    let mut n = 0usize;
    tree.visit(|_e: &Expr<()>| n += 1);
    assert_eq!(n, 2);
}
