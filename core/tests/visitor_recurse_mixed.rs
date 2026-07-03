//! A `visitor!()` spanning acyclic-outer types and a `#[recurse]` cycle: extra params, closures,
//! drill-in, heterogeneous concrete-fill.
#![allow(dead_code)]

// One `visitor!()` spanning an acyclic outer type and a `#[recurse]` cycle, crossed by a single `.visit()`.
mod one_visitor {
    use core::marker::PhantomData;
    use syan::parse::recurse;
    use syan::visit::Ast;

    #[recurse]
    mod ast {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast(crate::one_visitor::ast::Stmt)]
        pub enum Expr<S> {
            Stmt(Box<Stmt<S>>),
            Lit(PhantomData<S>),
        }

        #[derive(Ast)]
        #[subast(crate::one_visitor::ast::Expr)]
        pub enum Stmt<S> {
            Expr(Box<Expr<S>>),
            Nop(PhantomData<S>),
        }
    }

    #[derive(Ast)]
    #[subast(crate::one_visitor::ast::Expr)]
    pub struct Program<S> {
        pub body: Vec<ast::Expr<S>>,
        pub tail: ast::Expr<S>,
    }

    mod v {
        syan::visit::visitor!(
            crate::one_visitor::Program,
            crate::one_visitor::ast::Expr,
            crate::one_visitor::ast::Stmt
        );
    }

    #[derive(Default)]
    struct Counter {
        p: usize,
        e: usize,
        s: usize,
    }

    impl<S> v::Visit<S> for Counter {
        fn visit_program(&mut self, i: &Program<S>) {
            self.p += 1;
            v::visit_program(self, i);
        }
        fn visit_expr(&mut self, i: &ast::Expr<S>) {
            self.e += 1;
            v::visit_expr(self, i);
        }
        fn visit_stmt(&mut self, i: &ast::Stmt<S>) {
            self.s += 1;
            v::visit_stmt(self, i);
        }
    }

    #[test]
    fn one_visit_spans_outer_and_inner() {
        let prog: Program<()> = Program {
            body: vec![ast::Expr::Stmt(Box::new(ast::Stmt::Expr(Box::new(
                ast::Expr::Lit(PhantomData),
            ))))],
            tail: ast::Expr::Lit(PhantomData),
        };
        let mut c = Counter::default();
        prog.visit(&mut c);
        assert_eq!(c.p, 1, "the one Program");
        assert_eq!(c.e, 3, "body Expr + its inner Expr (back-edge) + tail Expr");
        assert_eq!(c.s, 1, "the one Stmt");
    }
}

// An acyclic outer type carrying an extra param beyond the cycle's: an ordinary union-param visitor.
mod extra_param {
    use core::marker::PhantomData;
    use syan::parse::recurse;

    #[recurse]
    mod ast {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast()]
        pub enum Expr<S> {
            Nest(Box<Expr<S>>),
            Lit(PhantomData<S>),
        }
    }

    #[derive(syan::visit::Ast)]
    #[subast(crate::extra_param::ast::Expr)]
    pub struct Program<S, T> {
        pub body: ast::Expr<S>,
        pub tag: PhantomData<T>,
    }

    mod v {
        syan::visit::visitor!(crate::extra_param::Program, crate::extra_param::ast::Expr);
    }

    #[test]
    fn mixed_recurse_with_extra_acyclic_param() {
        let prog: Program<(), u32> = Program {
            body: ast::Expr::Nest(Box::new(ast::Expr::Lit(PhantomData))),
            tag: PhantomData,
        };
        // The tuple infers `T = u32` from `prog`.
        let mut p = 0usize;
        let mut e = 0usize;
        prog.visit((
            |_: &Program<(), u32>| p += 1,
            |_: &ast::Expr<()>| e += 1,
        ));
        assert_eq!((p, e), (1, 2), "Program + 2 Exprs (Nest + inner Lit)");
    }
}

// Closures (incl. inherent `.visit()` and a tuple) over acyclic types living in a `#[recurse]` module.
mod closure {
    use core::marker::PhantomData;

    #[syan::parse::recurse]
    mod ast {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast()]
        pub enum Expr<S> {
            Nest(Box<Expr<S>>),
            Lit(PhantomData<S>),
        }

        #[derive(Ast)]
        #[subast()]
        pub enum Type<S> {
            Int(PhantomData<S>),
            Bool(PhantomData<S>),
        }

        #[derive(Ast)]
        #[subast(crate::closure::ast::Param, crate::closure::ast::Type)]
        pub struct Decl<S> {
            pub params: Vec<Param<S>>,
            pub ret: Type<S>,
            // A recurse-cycle node as a field: not in `#[subast]`, so a leaf for the acyclic visitor.
            pub body: Expr<S>,
        }

        #[derive(Ast)]
        #[subast(crate::closure::ast::Type)]
        pub struct Param<S> {
            pub ty: Type<S>,
        }
    }

    mod v {
        syan::visit::visitor!(crate::closure::ast::Decl, crate::closure::ast::Type);
    }

    fn sample() -> ast::Decl<()> {
        ast::Decl {
            params: vec![
                ast::Param { ty: ast::Type::Int(PhantomData) },
                ast::Param { ty: ast::Type::Bool(PhantomData) },
            ],
            ret: ast::Type::Int(PhantomData),
            body: ast::Expr::Lit(PhantomData),
        }
    }

    #[test]
    fn single_closure() {
        let d = sample();
        let mut types = 0usize;
        d.visit(|_t: &ast::Type<()>| types += 1);
        assert_eq!(types, 3, "two param Types (drilled through Param) + the ret Type");
    }

    #[test]
    fn tuple_of_closures() {
        let d = sample();
        let mut decls = 0usize;
        let mut types = 0usize;
        d.visit((
            |_d: &ast::Decl<()>| decls += 1,
            |_t: &ast::Type<()>| types += 1,
        ));
        assert_eq!((decls, types), (1, 3));
    }
}

// Drill-in and `#[recurse]` in the same module; a cycle-typed field not in `#[subast]` stays a leaf.
mod drill {
    use core::marker::PhantomData;
    use syan::parse::recurse;
    use syan::visit::Ast;

    #[recurse]
    mod ast {
        use core::marker::PhantomData;
        use syan::visit::Ast;

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

        #[derive(Ast)]
        pub enum Type<S> {
            Unit(PhantomData<S>),
        }

        #[derive(Ast)]
        #[subast(crate::drill::ast::Type)]
        pub struct Cast<S>(pub Type<S>);

        #[derive(Ast)]
        #[subast(crate::drill::ast::Cast)]
        pub struct Decl<S> {
            pub cast: Cast<S>,
            // A recurse'd alias field. Not in `#[subast]`, so a leaf for the visitor.
            pub body: Expr<S>,
        }
    }

    pub mod visit {
        // `Cast` is unlisted and drilled through to `Type`.
        syan::visit::visitor!(crate::drill::ast::Decl, crate::drill::ast::Type);
    }

    use ast::{Cast, Decl, Expr, Type};

    fn assert_is_ast<T: Ast>() {}

    #[test]
    fn ast_markers_hold_for_both_recurse_aliases_and_acyclic_types() {
        assert_is_ast::<Expr<()>>();
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

    // Drilling through an *unlisted* cross-edge cycle type (`Cast`) to reach the listed root `Expr`.
    mod unlisted {
        use core::marker::PhantomData;
        use syan::parse::recurse;

        #[recurse]
        mod ast {
            use core::marker::PhantomData;
            use syan::visit::Ast;

            #[derive(Ast)]
            #[subast(crate::drill::unlisted::ast::Cast)]
            pub enum Expr<S> {
                Bin(Box<Expr<S>>),
                Cast(Box<Cast<S>>),
                Lit(PhantomData<S>),
            }

            #[derive(Ast)]
            #[subast(crate::drill::unlisted::ast::Expr)]
            pub enum Cast<S> {
                Inner(Box<Expr<S>>),
                Nope(PhantomData<S>),
            }
        }

        mod v {
            syan::visit::visitor!(crate::drill::unlisted::ast::Expr);
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
            let e: ast::Expr<()> =
                ast::Expr::Cast(Box::new(ast::Cast::Inner(Box::new(ast::Expr::Lit(PhantomData)))));
            let mut c = Counter::default();
            v::Visit::visit_expr(&mut c, &e);
            assert_eq!(c.0, 2, "outer Expr + inner Expr reached by drilling through the unlisted Cast");
        }
    }
}

// Heterogeneous cycle: a non-shared param concrete-filled in a cross-edge becomes a method generic.
mod heterogeneous {
    use core::marker::PhantomData;
    use syan::parse::recurse;

    #[recurse]
    mod ast {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast(crate::heterogeneous::ast::Stmt)]
        pub enum Expr<S> {
            Stmt(Box<Stmt<S, u8>>), // cross-edge to Stmt, filling its extra param T = u8
            Lit(PhantomData<S>),
        }

        #[derive(Ast)]
        #[subast(crate::heterogeneous::ast::Expr)]
        pub enum Stmt<S, T> {
            Back(Box<Expr<S>>),
            Tag(PhantomData<(S, T)>),
        }
    }

    mod v {
        syan::visit::visitor!(crate::heterogeneous::ast::Expr, crate::heterogeneous::ast::Stmt);
    }

    #[derive(Default)]
    struct Counter(usize);

    // The trait is keyed on the shared `S`; `Stmt`'s extra `T` is a generic on `visit_stmt`.
    impl<S> v::Visit<S> for Counter {
        fn visit_expr(&mut self, i: &ast::Expr<S>) {
            self.0 += 1;
            v::visit_expr(self, i);
        }
        fn visit_stmt<T>(&mut self, i: &ast::Stmt<S, T>) {
            self.0 += 1;
            v::visit_stmt(self, i);
        }
    }

    #[test]
    fn heterogeneous_cycle_via_visitor() {
        let e: ast::Expr<()> =
            ast::Expr::Stmt(Box::new(ast::Stmt::Back(Box::new(ast::Expr::Lit(PhantomData)))));
        let mut c = Counter::default();
        v::Visit::visit_expr(&mut c, &e);
        assert_eq!(c.0, 3, "Expr + Stmt (extra param T=u8) + inner Expr");
    }
}
