//! `#[recurse]` cycle types may carry lifetime / type / const generic parameters alongside the
//! depth `__Rec`. The params thread through the regenerated aliases and through a depth-generic
//! `visitor!()` over the cycle (keyed on the roots' params; a non-root cycle type's *extra* params
//! become `visit_*` method generics — see the heterogeneous `het` case below).
//!
//! Was previously rejected: a lifetime param produced a confusing E0106; const params were refused.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::recurse;

#[derive(Default)]
struct Counter(usize);

// ── lifetime parameter ────────────────────────────────────────────────────────
#[recurse]
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

mod v_lt {
    syan::visit::visitor!(crate::lt::Expr);
}

impl<'a, S> v_lt::Visit<'a, S> for Counter {
    fn visit_expr<R: v_lt::VisitRec<'a, S, Self>>(&mut self, i: &v_lt::ExprNode<'a, S, R>) {
        self.0 += 1;
        v_lt::visit_expr(self, i);
    }
}

#[test]
fn lifetime_param_visitor() {
    let e: lt::Expr<'static, ()> = lt::Expr::Nest(Box::new(v_lt::ExprNode::Lit(PhantomData)));
    let mut c = Counter::default();
    v_lt::Visit::visit_expr(&mut c, &e);
    assert_eq!(c.0, 2, "outer Nest + inner Lit (reached via the back-edge)");
}

// ── const generic parameter ─────────────────────────────────────────────────────
#[recurse]
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

mod v_ct {
    syan::visit::visitor!(crate::ct::Expr);
}

impl<S, const N: usize> v_ct::Visit<S, N> for Counter {
    fn visit_expr<R: v_ct::VisitRec<S, N, Self>>(&mut self, i: &v_ct::ExprNode<S, N, R>) {
        self.0 += 1;
        v_ct::visit_expr(self, i);
    }
}

#[test]
fn const_param_visitor() {
    let e: ct::Expr<(), 2> = ct::Expr::Nest(Box::new(v_ct::ExprNode::Lit(PhantomData)));
    let mut c = Counter::default();
    v_ct::Visit::visit_expr(&mut c, &e);
    assert_eq!(c.0, 2, "const param N threads through the depth-generic visitor");
}

// ── non-`usize` const generic parameter ─────────────────────────────────────────
// The terminator used to encode each const param as `PhantomData<[(); N]>`, which only works for
// `const N: usize`. Const params are now simply omitted from the terminator's `PhantomData` (unused
// const params don't trigger E0392), so any const type — e.g. `const C: char` — is supported.
#[recurse]
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

mod v_ct_char {
    syan::visit::visitor!(crate::ct_char::Expr);
}

impl<S, const C: char> v_ct_char::Visit<S, C> for Counter {
    fn visit_expr<R: v_ct_char::VisitRec<S, C, Self>>(&mut self, i: &v_ct_char::ExprNode<S, C, R>) {
        self.0 += 1;
        v_ct_char::visit_expr(self, i);
    }
}

#[test]
fn non_usize_const_param_visitor() {
    let e: ct_char::Expr<(), 'x'> =
        ct_char::Expr::Nest(Box::new(v_ct_char::ExprNode::Lit(PhantomData)));
    let mut c = Counter::default();
    v_ct_char::Visit::visit_expr(&mut c, &e);
    assert_eq!(c.0, 2, "const C: char threads through; terminator no longer needs `[(); N]`");
}

// ── two type params + a cross-edge cycle ─────────────────────────────────────────
#[recurse]
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

mod v_multi {
    syan::visit::visitor!(crate::multi::Expr, crate::multi::Stmt);
}

impl<S, T> v_multi::Visit<S, T> for Counter {
    fn visit_expr<R: v_multi::VisitRec<S, T, Self>>(&mut self, i: &v_multi::ExprNode<S, T, R>) {
        self.0 += 1;
        v_multi::visit_expr(self, i);
    }
    fn visit_stmt<R: v_multi::VisitRec<S, T, Self>>(&mut self, i: &v_multi::StmtNode<S, T, R>) {
        self.0 += 1;
        v_multi::visit_stmt(self, i);
    }
}

#[test]
fn two_type_params_cross_edge() {
    // Expr -> Stmt (cross) -> Expr (back-edge).
    let e: multi::Expr<(), u8> = multi::Expr::Stmt(Box::new(multi::Stmt::Expr(Box::new(
        v_multi::ExprNode::Lit(PhantomData),
    ))));
    let mut c = Counter::default();
    v_multi::Visit::visit_expr(&mut c, &e);
    assert_eq!(c.0, 3, "outer Expr + Stmt + inner Expr");
}

// ── heterogeneous generics: cycle types with DIFFERENT params ───────────────────
// `Expr<S>` is the root; `Stmt<S, T>` carries an extra `T` (filled concretely by the cross-edge).
// Each type keeps its own params; the `visitor!()` trait is keyed on the root's `S`, and `Stmt`'s
// extra `T` becomes a generic on `visit_stmt`. (A standalone version is `visitor_recurse_heterogeneous.rs`.)
#[recurse]
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

mod v_het {
    syan::visit::visitor!(crate::het::Expr, crate::het::Stmt);
}

impl<S> v_het::Visit<S> for Counter {
    fn visit_expr<R: v_het::VisitRec<S, Self>>(&mut self, i: &v_het::ExprNode<S, R>) {
        self.0 += 1;
        v_het::visit_expr(self, i);
    }
    fn visit_stmt<T, R: v_het::VisitRec<S, Self>>(&mut self, i: &v_het::StmtNode<S, T, R>) {
        self.0 += 1;
        v_het::visit_stmt(self, i);
    }
}

#[test]
fn heterogeneous_generics_visitor() {
    // Expr -> Stmt<_, u8> (cross) -> Expr (back-edge).
    let e: het::Expr<()> =
        het::Expr::Stmt(Box::new(het::Stmt::Back(Box::new(v_het::ExprNode::Lit(PhantomData)))));
    let mut c = Counter::default();
    v_het::Visit::visit_expr(&mut c, &e);
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
