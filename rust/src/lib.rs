//! `syan-rust`: Rust AST definitions for the syan parser.
//!
//! Rebuilt incrementally; previous contents are preserved in `rust_old/` at the repo root.

/// A tiny sample AST used to exercise the visitor system across crate boundaries
/// (see `tests/cross_crate.rs`).
pub mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Debug, Ast)]
    #[subast(crate::ast::Stmt)]
    pub enum Expr<S> {
        Stmt(Box<Stmt<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Debug, Ast)]
    #[subast(crate::ast::Expr)]
    pub enum Stmt<S> {
        Expr(Box<Expr<S>>),
        Nop(PhantomData<S>),
    }
}

/// A visitor over [`ast`], generated in this crate so its inherent `visit` methods are available to
/// downstream crates (see `tests/cross_crate.rs`).
pub mod visit {
    syan::visit::visitor!(crate::ast::Expr, crate::ast::Stmt);
}

/// A `#[recurse]` cycle defined upstream with **no in-crate visitor** — a downstream crate builds the
/// (depth-generic) visitor with `visitor!(syan_rust::recursed::Expr, …)`. Exercises the cross-crate
/// `@recurse` metadata: the `$crate`-rooted `@node`/`@terms` paths resolve back to this crate.
/// See `tests/cross_crate_recurse.rs`.
#[syan::parse::recurse]
pub mod recursed {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast(crate::recursed::Stmt)]
    pub enum Expr<S> {
        Stmt(Box<Stmt<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast(crate::recursed::Expr)]
    pub enum Stmt<S> {
        Expr(Box<Expr<S>>),
        Nop(PhantomData<S>),
    }
}

/// Base AST + a base **visitor**, generated in this crate, for cross-crate *inheritance*: a
/// downstream crate writes `visitor!(syan_rust::inherit::base => NewType)` to extend this visitor
/// (supertrait) and add a method for its own type. The base exports its `Visit` trait, free
/// `visit_*` fns, and the `__syan_visited` inheritance macro — all reachable downstream. Exercised
/// by `tests/cross_crate_inherit.rs`.
pub mod inherit {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Debug, Ast)]
    pub enum Type<S> {
        Unit(PhantomData<S>),
    }

    #[derive(Debug, Ast)]
    #[subast(crate::inherit::Type)]
    pub enum Expr<S> {
        Typed(Box<Type<S>>),
        Lit(PhantomData<S>),
    }

    /// The base visitor over [`Type`] and [`Expr`]. A downstream `visitor!(<path>::base => T)`
    /// inherits it (see `tests/cross_crate_inherit.rs`).
    pub mod base {
        syan::visit::visitor!(crate::inherit::Type, crate::inherit::Expr);
    }

    #[derive(Debug, Ast)]
    #[subast(crate::inherit::Expr)]
    pub enum Item<S> {
        Ex(Box<Expr<S>>),
        NoExpr(PhantomData<S>),
    }

    /// A *mid* visitor — itself UPSTREAM — that inherits [`base`] and adds [`Item`]. A downstream
    /// `visitor!(<path>::mid => T)` therefore inherits `mid` AND, transitively, `base` — *both
    /// upstream*. `mid` records `base` as the `crate::inherit::base` it was given (its own crate),
    /// so the downstream extender must requalify that ancestor against `mid`'s host crate to satisfy
    /// the transitive `base::Visit` supertrait. Exercised by `tests/cross_crate_inherit_multilevel.rs`.
    pub mod mid {
        syan::visit::visitor!(crate::inherit::base => crate::inherit::Item);
    }

    #[derive(Debug, Ast)]
    #[subast(crate::inherit::Item)]
    pub enum Block<S> {
        I(Box<Item<S>>),
        Nil(PhantomData<S>),
    }

    /// An *upper* visitor — also UPSTREAM — that inherits [`mid`] and adds [`Block`], so its `@an`
    /// chain carries TWO `crate::`-relative ancestors (`mid` and `base`). A downstream
    /// `visitor!(<path>::upper => T)` must requalify *both* against `upper`'s host crate — the
    /// 4-level case (`base => mid => upper => new`) that drives the requalify loop more than once.
    /// Exercised by `tests/cross_crate_inherit_4level.rs`.
    pub mod upper {
        syan::visit::visitor!(crate::inherit::mid => crate::inherit::Block);
    }

    /// A *mid* variant that records its `base` ancestor via a **`super::`-relative** path (instead of
    /// the `crate::inherit::base` that [`mid`] uses). It builds fine upstream, but a downstream
    /// `visitor!(<path>::mid_ss => T)` receives the ancestor as `super::base` — relative to the
    /// *downstream* module, where it's unresolvable (a proc-macro can't requalify `super`/`self` like
    /// it does a leading `crate::`). This is the residual cross-crate hole; the fix is to use a
    /// `crate::`-rooted entry path (as `mid` does). Realized by `tests/ui/cross_crate_super_self.rs`.
    /// (Its own `ItemSs` — not `Item` — to avoid colliding with `mid` on the inherent `visit`.)
    #[derive(Debug, Ast)]
    #[subast(crate::inherit::Expr)]
    pub enum ItemSs<S> {
        Ex(Box<Expr<S>>),
        NoExpr(PhantomData<S>),
    }
    pub mod mid_ss {
        syan::visit::visitor!(super::base => crate::inherit::ItemSs);
    }
}

/// Acyclic types whose `#[subast]` paths are `crate::`-rooted, so a *downstream* crate can build a
/// visitor that drills through `Wrap` and resolves its `#[subast]` child in *this* crate (the
/// metadata macro `$crate`-roots them). Exercised by `tests/cross_crate_drill.rs`.
pub mod drillable {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Debug, Ast)]
    pub struct Inner<S>(pub PhantomData<S>);

    #[derive(Debug, Ast)]
    #[subast(crate::drillable::Inner)]
    pub struct Wrap<S>(pub Inner<S>);
}
