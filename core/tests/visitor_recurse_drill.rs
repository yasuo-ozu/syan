//! Drill-in and `#[recurse]` active in the *same module*.
//!
//! `#[recurse]` rewrites only the types that form a *cycle* (here `Expr`/`Stmt`): it renames them
//! internally, threads a depth parameter, and exposes depth-limited public aliases. Acyclic types in
//! the same module (here the `Decl`/`Cast`/`Type` drill-in chain) are passed through untouched, so
//! `#[derive(Ast)]` + `#[subast]` apply to them under their own names and a drill-in visitor works
//! normally. A field of a recurse'd type (`Decl::body: Expr<S>`) that is *not* listed in `#[subast]`
//! is correctly treated as a leaf, so the visitor never tries to resolve a metadata macro for the
//! recurse'd alias.
//!
//! Visiting *through* the recurse'd cycle now works too (natural types make a former-cycle an ordinary
//! acyclic visitor); the `unlisted` module below drills through an *unlisted* cross-edge cycle type.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::recurse;
use syan::visit::Ast;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    // ── Recurse'd cycle: renamed + depth-threaded by `#[recurse]` ───────────────────────────────
    #[derive(Ast)]
    pub enum Expr<S> {
        Stmt(Box<Stmt<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    pub enum Stmt<S> {
        Expr(Box<Expr<S>>),
        Nop(PhantomData<S>),
    }

    // ── Acyclic drill-in chain: untouched by `#[recurse]` ───────────────────────────────────────
    #[derive(Ast)]
    pub enum Type<S> {
        Unit(PhantomData<S>),
    }

    // Unlisted intermediate (drilled through), followed to its `Type`.
    #[derive(Ast)]
    #[subast(crate::ast::Type)]
    pub struct Cast<S>(pub Type<S>);

    #[derive(Ast)]
    #[subast(crate::ast::Cast)]
    pub struct Decl<S> {
        pub cast: Cast<S>,
        // A recurse'd alias as a field. Not in `#[subast]`, so it is a leaf for the visitor.
        pub body: Expr<S>,
    }
}

pub mod visit {
    // Drill-in visitor over the acyclic types; `Cast` is unlisted and drilled through to `Type`.
    syan::visit::visitor!(crate::ast::Decl, crate::ast::Type);
}

use ast::{Cast, Decl, Expr, Type};

fn assert_is_ast<T: Ast>() {}

#[test]
fn ast_markers_hold_for_both_recurse_aliases_and_acyclic_types() {
    // The cycle's public aliases keep the `Ast` marker (carried from the renamed internal types)...
    assert_is_ast::<Expr<()>>();
    // ...and the acyclic drill-in types are plain `#[derive(Ast)]` types.
    assert_is_ast::<Type<()>>();
    assert_is_ast::<Cast<()>>();
    assert_is_ast::<Decl<()>>();
}

#[test]
fn drills_through_cast_while_ignoring_the_recurse_alias_field() {
    let decl: Decl<()> = Decl {
        cast: Cast(Type::Unit(PhantomData)),
        body: Expr::Lit(PhantomData),
    };
    let mut types = 0usize;
    decl.visit(|_t: &Type<()>| types += 1);
    assert_eq!(
        types, 1,
        "drilled Decl -> Cast -> Type once; the recurse'd `body: Expr` field is a leaf"
    );
}

#[test]
fn struct_visitor_reaches_type_only_through_drilling() {
    #[derive(Default)]
    struct Counter {
        decls: usize,
        types: usize,
    }
    impl<S> visit::Visit<S> for Counter {
        fn visit_decl(&mut self, i: &Decl<S>) {
            self.decls += 1;
            visit::visit_decl(self, i);
        }
        fn visit_type(&mut self, i: &Type<S>) {
            self.types += 1;
            visit::visit_type(self, i);
        }
    }
    let decl: Decl<()> = Decl {
        cast: Cast(Type::Unit(PhantomData)),
        body: Expr::Stmt(Box::new(ast::Stmt::Nop(PhantomData))),
    };
    let mut c = Counter::default();
    decl.visit(&mut c);
    assert_eq!(c.decls, 1);
    assert_eq!(c.types, 1, "reached via drilling through Cast, not via the Expr field");
}

/// Drilling through an *unlisted* cross-edge cycle type (it gets no `visit_*`), reaching the listed
/// types nested inside it. Here `Expr` is the root (self-referential via `Bin`) and the only listed type;
/// `Cast` is an unlisted cross-edge that `visit_expr` drills through to reach the inner `Expr`.
mod unlisted {
    use core::marker::PhantomData;
    use syan::parse::recurse;

    #[recurse]
    mod ast {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast(crate::unlisted::ast::Cast)]
        pub enum Expr<S> {
            Bin(Box<Expr<S>>),  // self-reference → Expr is the root
            Cast(Box<Cast<S>>), // cross-edge to the UNLISTED Cast
            Lit(PhantomData<S>),
        }

        #[derive(Ast)]
        #[subast(crate::unlisted::ast::Expr)]
        pub enum Cast<S> {
            Inner(Box<Expr<S>>), // ref to the root Expr — reached by drilling through the unlisted Cast
            Nope(PhantomData<S>),
        }
    }

    mod v {
        // Cast is NOT listed → it must be drilled through.
        syan::visit::visitor!(crate::unlisted::ast::Expr);
    }

    #[derive(Default)]
    struct Counter(usize);

    impl<S> v::Visit<S> for Counter {
        fn visit_expr(&mut self, i: &ast::Expr<S>) {
            self.0 += 1;
            v::visit_expr(self, i);
        }
    }

    #[test]
    fn drills_through_unlisted_cast() {
        // Expr::Cast( Cast::Inner( Expr::Lit ) ) → outer Expr + (drill Cast, no visit_cast) + inner Expr.
        let e: ast::Expr<()> =
            ast::Expr::Cast(Box::new(ast::Cast::Inner(Box::new(ast::Expr::Lit(PhantomData)))));
        let mut c = Counter::default();
        v::Visit::visit_expr(&mut c, &e);
        assert_eq!(c.0, 2, "outer Expr + inner Expr reached by drilling through the unlisted Cast");
    }
}
