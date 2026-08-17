//! `#[recurse]` generics + multi-cycle/multi-root shapes, visited via `visitor!`.
//! Each source test is a mod so their crate-root items (`Counter`, `Expr`, …) don't collide.
#![allow(dead_code)]

// `#[recurse]` cycle types carrying lifetime / type / const generic params, visited via `visitor!`.
mod generics {
    use core::marker::PhantomData;
    use syan::parse::recurse;

    #[derive(Default)]
    struct Counter(usize);

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
        syan::visit::visitor!(crate::generics::lt::Expr);
    }

    impl<'a, S> v_lt::Visit<'a, S> for Counter {
        fn visit_expr(&mut self, i: &lt::Expr<'a, S>) {
            self.0 += 1;
            v_lt::visit_expr(self, i);
        }
    }

    #[test]
    fn lifetime_param_visitor() {
        let e: lt::Expr<'static, ()> = lt::Expr::Nest(Box::new(lt::Expr::Lit(PhantomData)));
        let mut c = Counter::default();
        v_lt::Visit::visit_expr(&mut c, &e);
        assert_eq!(c.0, 2, "outer Nest + inner Lit (reached via the back-edge)");
    }

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
        syan::visit::visitor!(crate::generics::ct::Expr);
    }

    impl<S, const N: usize> v_ct::Visit<S, N> for Counter {
        fn visit_expr(&mut self, i: &ct::Expr<S, N>) {
            self.0 += 1;
            v_ct::visit_expr(self, i);
        }
    }

    #[test]
    fn const_param_visitor() {
        let e: ct::Expr<(), 2> = ct::Expr::Nest(Box::new(ct::Expr::Lit(PhantomData)));
        let mut c = Counter::default();
        v_ct::Visit::visit_expr(&mut c, &e);
        assert_eq!(c.0, 2, "const param N threads through the depth-generic visitor");
    }

    // Const params are omitted from the terminator's `PhantomData` (unused const params don't trigger
    // E0392), so a non-`usize` const type like `const C: char` is supported.
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
        syan::visit::visitor!(crate::generics::ct_char::Expr);
    }

    impl<S, const C: char> v_ct_char::Visit<S, C> for Counter {
        fn visit_expr(&mut self, i: &ct_char::Expr<S, C>) {
            self.0 += 1;
            v_ct_char::visit_expr(self, i);
        }
    }

    #[test]
    fn non_usize_const_param_visitor() {
        let e: ct_char::Expr<(), 'x'> =
            ct_char::Expr::Nest(Box::new(ct_char::Expr::Lit(PhantomData)));
        let mut c = Counter::default();
        v_ct_char::Visit::visit_expr(&mut c, &e);
        assert_eq!(c.0, 2, "const C: char threads through; terminator no longer needs `[(); N]`");
    }

    #[recurse]
    mod multi {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast(crate::generics::multi::Stmt)]
        pub enum Expr<S, T> {
            Stmt(Box<Stmt<S, T>>),
            Lit(PhantomData<(S, T)>),
        }

        #[derive(Ast)]
        #[subast(crate::generics::multi::Expr)]
        pub enum Stmt<S, T> {
            Expr(Box<Expr<S, T>>),
            Nop(PhantomData<(S, T)>),
        }
    }

    mod v_multi {
        syan::visit::visitor!(crate::generics::multi::Expr, crate::generics::multi::Stmt);
    }

    impl<S, T> v_multi::Visit<S, T> for Counter {
        fn visit_expr(&mut self, i: &multi::Expr<S, T>) {
            self.0 += 1;
            v_multi::visit_expr(self, i);
        }
        fn visit_stmt(&mut self, i: &multi::Stmt<S, T>) {
            self.0 += 1;
            v_multi::visit_stmt(self, i);
        }
    }

    #[test]
    fn two_type_params_cross_edge() {
        let e: multi::Expr<(), u8> = multi::Expr::Stmt(Box::new(multi::Stmt::Expr(Box::new(
            multi::Expr::Lit(PhantomData),
        ))));
        let mut c = Counter::default();
        v_multi::Visit::visit_expr(&mut c, &e);
        assert_eq!(c.0, 3, "outer Expr + Stmt + inner Expr");
    }

    // Heterogeneous generics: `Expr<S>` is the root, `Stmt<S, T>` carries an extra `T` filled
    // concretely by the cross-edge, so `T` becomes a generic on `visit_stmt` (trait keyed on `S`).
    #[recurse]
    mod het {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast(crate::generics::het::Stmt)]
        pub enum Expr<S> {
            Stmt(Box<Stmt<S, u8>>),
            Lit(PhantomData<S>),
        }

        #[derive(Ast)]
        #[subast(crate::generics::het::Expr)]
        pub enum Stmt<S, T> {
            Back(Box<Expr<S>>),
            Tag(PhantomData<(S, T)>),
        }
    }

    mod v_het {
        syan::visit::visitor!(crate::generics::het::Expr, crate::generics::het::Stmt);
    }

    impl<S> v_het::Visit<S> for Counter {
        fn visit_expr(&mut self, i: &het::Expr<S>) {
            self.0 += 1;
            v_het::visit_expr(self, i);
        }
        fn visit_stmt<T>(&mut self, i: &het::Stmt<S, T>) {
            self.0 += 1;
            v_het::visit_stmt(self, i);
        }
    }

    #[test]
    fn heterogeneous_generics_visitor() {
        let e: het::Expr<()> =
            het::Expr::Stmt(Box::new(het::Stmt::Back(Box::new(het::Expr::Lit(PhantomData)))));
        let mut c = Counter::default();
        v_het::Visit::visit_expr(&mut c, &e);
        assert_eq!(c.0, 3, "Expr + Stmt (extra param T=u8) + inner Expr");
    }

}

// Several *independent* cycles (separate SCCs) in one `#[recurse]` module, visited via one `visitor!`.
mod multi_cycle {
    use core::marker::PhantomData;
    use syan::parse::recurse;

    #[derive(Default)]
    struct Counter(usize);

    // Expr and Type are disjoint self-referential cycles (independent SCCs).
    #[recurse]
    mod vis {
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
            Arrow(Box<Type<S>>),
            Unit(PhantomData<S>),
        }
    }

    mod v_vis {
        syan::visit::visitor!(crate::multi_cycle::vis::Expr, crate::multi_cycle::vis::Type);
    }

    impl<S> v_vis::Visit<S> for Counter {
        fn visit_expr(&mut self, i: &vis::Expr<S>) {
            self.0 += 10;
            v_vis::visit_expr(self, i);
        }
        fn visit_type(&mut self, i: &vis::Type<S>) {
            self.0 += 1;
            v_vis::visit_type(self, i);
        }
    }

    #[test]
    fn independent_visitors_are_separate() {
        // Each cycle's visitor descends only its own type — they don't bleed into each other.
        let e: vis::Expr<()> = vis::Expr::Nest(Box::new(vis::Expr::Lit(PhantomData)));
        let t: vis::Type<()> = vis::Type::Arrow(Box::new(vis::Type::Unit(PhantomData)));

        let mut c = Counter::default();
        v_vis::Visit::visit_expr(&mut c, &e);
        assert_eq!(c.0, 20, "two Expr nodes at +10 each");

        let mut c2 = Counter::default();
        v_vis::Visit::visit_type(&mut c2, &t);
        assert_eq!(c2.0, 2, "two Type nodes at +1 each");
    }
}

// A strongly-connected cycle where several types are each self-referential AND reference each other.
mod multiroot {
    use core::marker::PhantomData;
    use syan::parse::recurse;

    #[recurse]
    mod ast {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast(crate::multiroot::ast::B)]
        pub enum A<S> {
            Me(Box<A<S>>),
            ToB(Box<B<S>>),
            Lit(PhantomData<S>),
        }

        #[derive(Ast)]
        #[subast(crate::multiroot::ast::A)]
        pub enum B<S> {
            Me(Box<B<S>>),
            ToA(Box<A<S>>),
            Lit(PhantomData<S>),
        }
    }

    mod v_ast {
        syan::visit::visitor!(crate::multiroot::ast::A, crate::multiroot::ast::B);
    }

    #[derive(Default)]
    struct Counter {
        a: usize,
        b: usize,
    }

    impl<S> v_ast::Visit<S> for Counter {
        fn visit_a(&mut self, i: &ast::A<S>) {
            self.a += 1;
            v_ast::visit_a(self, i);
        }
        fn visit_b(&mut self, i: &ast::B<S>) {
            self.b += 1;
            v_ast::visit_b(self, i);
        }
    }

    #[test]
    fn each_root_keeps_its_own_depth() {
        // A(outer) -> ToB(B) -> ToA(A) -> Lit.
        let v: ast::A<()> =
            ast::A::ToB(Box::new(ast::B::ToA(Box::new(ast::A::Lit(PhantomData)))));
        let mut c = Counter::default();
        v_ast::Visit::visit_a(&mut c, &v);
        assert_eq!((c.a, c.b), (2, 1), "two A nodes (outer + inner) and one B node");
    }

    #[test]
    fn visit_from_either_root() {
        // Pure-B nesting B -> Me(B) -> Lit, entered through visit_b.
        let v: ast::B<()> = ast::B::Me(Box::new(ast::B::Lit(PhantomData)));
        let mut c = Counter::default();
        v_ast::Visit::visit_b(&mut c, &v);
        assert_eq!((c.a, c.b), (0, 2), "two B nodes, no A");
    }
}
