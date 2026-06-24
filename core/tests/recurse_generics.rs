//! `#[recurse]` cycle types may carry lifetime / type / const generic parameters alongside the
//! depth `__Rec`, as long as every cycle type shares the root's signature. The params thread through
//! the regenerated aliases and (for `#[recurse(visit)]`) the depth-generic visitor.
//!
//! Was previously rejected: a lifetime param produced a confusing E0106; const params were refused.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::recurse;

#[derive(Default)]
struct Counter(usize);

// ── lifetime parameter ────────────────────────────────────────────────────────
#[recurse(visit)]
mod lt {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<'a, S> {
        Nest(Box<Expr<'a, S>>),
        Lit(PhantomData<(&'a (), S)>),
    }
}

impl<'a, S> lt::Visit<'a, S> for Counter {
    fn visit_expr<R: lt::VisitRec<'a, S, Self>>(&mut self, i: &lt::ExprNode<'a, S, R>) {
        self.0 += 1;
        lt::visit_expr(self, i);
    }
}

#[test]
fn lifetime_param_visitor() {
    let e: lt::Expr<'static, ()> = lt::Expr::Nest(Box::new(lt::ExprNode::Lit(PhantomData)));
    let mut c = Counter::default();
    lt::Visit::visit_expr(&mut c, &e);
    assert_eq!(c.0, 2, "outer Nest + inner Lit (reached via the back-edge)");
}

// ── const generic parameter ─────────────────────────────────────────────────────
#[recurse(visit)]
mod ct {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S, const N: usize> {
        Nest(Box<Expr<S, N>>),
        Lit(PhantomData<(S, [(); N])>),
    }
}

impl<S, const N: usize> ct::Visit<S, N> for Counter {
    fn visit_expr<R: ct::VisitRec<S, N, Self>>(&mut self, i: &ct::ExprNode<S, N, R>) {
        self.0 += 1;
        ct::visit_expr(self, i);
    }
}

#[test]
fn const_param_visitor() {
    let e: ct::Expr<(), 2> = ct::Expr::Nest(Box::new(ct::ExprNode::Lit(PhantomData)));
    let mut c = Counter::default();
    ct::Visit::visit_expr(&mut c, &e);
    assert_eq!(c.0, 2, "const param N threads through the depth-generic visitor");
}

// ── non-`usize` const generic parameter ─────────────────────────────────────────
// The terminator used to encode each const param as `PhantomData<[(); N]>`, which only works for
// `const N: usize`. Const params are now simply omitted from the terminator's `PhantomData` (unused
// const params don't trigger E0392), so any const type — e.g. `const C: char` — is supported.
#[recurse(visit)]
mod ct_char {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S, const C: char> {
        Nest(Box<Expr<S, C>>),
        Lit(PhantomData<S>),
    }
}

impl<S, const C: char> ct_char::Visit<S, C> for Counter {
    fn visit_expr<R: ct_char::VisitRec<S, C, Self>>(&mut self, i: &ct_char::ExprNode<S, C, R>) {
        self.0 += 1;
        ct_char::visit_expr(self, i);
    }
}

#[test]
fn non_usize_const_param_visitor() {
    let e: ct_char::Expr<(), 'x'> =
        ct_char::Expr::Nest(Box::new(ct_char::ExprNode::Lit(PhantomData)));
    let mut c = Counter::default();
    ct_char::Visit::visit_expr(&mut c, &e);
    assert_eq!(c.0, 2, "const C: char threads through; terminator no longer needs `[(); N]`");
}

// ── two type params + a cross-edge cycle ─────────────────────────────────────────
#[recurse(visit)]
mod multi {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S, T> {
        Stmt(Box<Stmt<S, T>>),
        Lit(PhantomData<(S, T)>),
    }

    #[derive(Ast)]
    #[subast()]
    pub enum Stmt<S, T> {
        Expr(Box<Expr<S, T>>),
        Nop(PhantomData<(S, T)>),
    }
}

impl<S, T> multi::Visit<S, T> for Counter {
    fn visit_expr<R: multi::VisitRec<S, T, Self>>(&mut self, i: &multi::ExprNode<S, T, R>) {
        self.0 += 1;
        multi::visit_expr(self, i);
    }
    fn visit_stmt<R: multi::VisitRec<S, T, Self>>(&mut self, i: &multi::StmtNode<S, T, R>) {
        self.0 += 1;
        multi::visit_stmt(self, i);
    }
}

#[test]
fn two_type_params_cross_edge() {
    // Expr -> Stmt (cross) -> Expr (back-edge).
    let e: multi::Expr<(), u8> =
        multi::Expr::Stmt(Box::new(multi::Stmt::Expr(Box::new(multi::ExprNode::Lit(PhantomData)))));
    let mut c = Counter::default();
    multi::Visit::visit_expr(&mut c, &e);
    assert_eq!(c.0, 3, "outer Expr + Stmt + inner Expr");
}

// ── heterogeneous generics: cycle types with DIFFERENT params ───────────────────
// `Expr<S>` is the root; `Stmt<S, T>` carries an extra `T` (filled concretely by the cross-edge).
// Each type keeps its own params; the trait is keyed on the root's `S`, and `Stmt`'s extra `T`
// becomes a generic on `visit_stmt`.
#[recurse(visit)]
mod het {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        Stmt(Box<Stmt<S, u8>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast()]
    pub enum Stmt<S, T> {
        Back(Box<Expr<S>>),
        Tag(PhantomData<(S, T)>),
    }
}

impl<S> het::Visit<S> for Counter {
    fn visit_expr<R: het::VisitRec<S, Self>>(&mut self, i: &het::ExprNode<S, R>) {
        self.0 += 1;
        het::visit_expr(self, i);
    }
    fn visit_stmt<T, R: het::VisitRec<S, Self>>(&mut self, i: &het::StmtNode<S, T, R>) {
        self.0 += 1;
        het::visit_stmt(self, i);
    }
}

#[test]
fn heterogeneous_generics_visitor() {
    // Expr -> Stmt<_, u8> (cross) -> Expr (back-edge).
    let e: het::Expr<()> =
        het::Expr::Stmt(Box::new(het::Stmt::Back(Box::new(het::ExprNode::Lit(PhantomData)))));
    let mut c = Counter::default();
    het::Visit::visit_expr(&mut c, &e);
    assert_eq!(c.0, 3, "Expr + Stmt (extra param T=u8) + inner Expr");
}

// ── base recurse (no visit) with a lifetime: confirm the alias compiles (no E0106) ──
#[recurse]
mod base {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<'a, S> {
        Nest(Box<Expr<'a, S>>),
        Lit(PhantomData<(&'a (), S)>),
    }
}

#[test]
fn base_recurse_lifetime_compiles() {
    let _e: base::Expr<'static, ()> = base::Expr::Lit(PhantomData);
}
