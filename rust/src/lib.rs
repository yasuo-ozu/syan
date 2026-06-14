//! `syan-rust`: Rust AST definitions for the syan parser.
//!
//! Rebuilt incrementally; previous contents are preserved in `rust_old/` at the repo root.

/// A tiny sample AST used to exercise the visitor system across crate boundaries
/// (see `tests/cross_crate.rs`).
pub mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Debug, Ast)]
    pub enum Expr<S> {
        Stmt(Box<Stmt<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Debug, Ast)]
    pub enum Stmt<S> {
        Expr(Box<Expr<S>>),
        Nop(PhantomData<S>),
    }
}

/// A visitor over [`ast`], generated in this crate so its inherent `visit` methods are available to
/// downstream crates (see `tests/cross_crate.rs`).
#[syan::visit::visitor(crate::ast::Expr, crate::ast::Stmt)]
pub mod visit {}
