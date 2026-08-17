//! Visiting a `#[recurse]` cycle: struct/closure/mut visitors over single and disjoint-param
//! cycles. `#[recurse]` exposes the cycle as natural recursive types, so `visitor!()` builds an
//! ordinary acyclic visitor (one `visit_*` per type, no depth parameter).
#![allow(dead_code)]

// visitor!() over a two-type cycle: struct Visit, Visit::visit_*, and the visit_mut mirror.
mod via_visitor {
    use core::marker::PhantomData;
    use syan::parse::recurse;

    #[recurse]
    mod ast {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast(crate::via_visitor::ast::Stmt)]
        pub enum Expr<S> {
            Stmt(Box<Stmt<S>>),
            Lit(PhantomData<S>),
        }

        #[derive(Ast)]
        #[subast(crate::via_visitor::ast::Expr)]
        pub enum Stmt<S> {
            Expr(Box<Expr<S>>),
            Nop(PhantomData<S>),
        }
    }

    mod v {
        syan::visit::visitor!(crate::via_visitor::ast::Expr, crate::via_visitor::ast::Stmt);
    }

    #[derive(Default)]
    struct Counter {
        e: usize,
        s: usize,
    }

    impl<S> v::Visit<S> for Counter {
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
    fn walks_the_cycle() {
        let e: ast::Expr<()> = ast::Expr::Stmt(Box::new(ast::Stmt::Expr(Box::new(
            ast::Expr::Lit(PhantomData),
        ))));
        let mut c = Counter::default();
        v::Visit::visit_expr(&mut c, &e);
        assert_eq!(
            (c.e, c.s),
            (2, 1),
            "outer Expr + inner Expr (via back-edge) = 2; one Stmt"
        );
    }

    #[test]
    fn leaf_only() {
        let e: ast::Expr<()> = ast::Expr::Lit(PhantomData);
        let mut c = Counter::default();
        v::Visit::visit_expr(&mut c, &e);
        assert_eq!((c.e, c.s), (1, 0));
    }

    impl<S> v::VisitMut<S> for Counter {
        fn visit_expr_mut(&mut self, i: &mut ast::Expr<S>) {
            self.e += 1;
            v::visit_expr_mut(self, i);
        }
        fn visit_stmt_mut(&mut self, i: &mut ast::Stmt<S>) {
            self.s += 1;
            v::visit_stmt_mut(self, i);
        }
    }

    #[test]
    fn walks_the_cycle_mut() {
        let mut e: ast::Expr<()> = ast::Expr::Stmt(Box::new(ast::Stmt::Expr(Box::new(
            ast::Expr::Lit(PhantomData),
        ))));
        let mut c = Counter::default();
        e.visit_mut(&mut c);
        assert_eq!(
            (c.e, c.s),
            (2, 1),
            "same shape as the shared walk, via &mut"
        );
    }

    #[test]
    fn closure_over_recurse_cycle() {
        let e: ast::Expr<()> = ast::Expr::Stmt(Box::new(ast::Stmt::Expr(Box::new(
            ast::Expr::Lit(PhantomData),
        ))));
        let mut exprs = 0usize;
        e.visit(|_e: &ast::Expr<()>| exprs += 1);
        assert_eq!(exprs, 2, "both Expr nodes seen by the closure");
    }

    // A single self-recursive root (no other cycle type): both `Add` operands are root back-edges.
    #[recurse]
    mod tree {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast()]
        pub enum Expr<S> {
            Add(Box<Expr<S>>, Box<Expr<S>>),
            Lit(PhantomData<S>),
        }
    }

    mod v_tree {
        syan::visit::visitor!(crate::via_visitor::tree::Expr);
    }

    #[test]
    fn closure_over_self_recursive_root() {
        let e: tree::Expr<()> = tree::Expr::Add(
            Box::new(tree::Expr::Lit(PhantomData)),
            Box::new(tree::Expr::Lit(PhantomData)),
        );
        let mut n = 0usize;
        e.visit(|_e: &tree::Expr<()>| n += 1);
        assert_eq!(n, 3);
    }
}

// One visitor!() over two disjoint-param cycles (Expr<S> + Foo<T>) — a union-param acyclic visitor.
mod disjoint_params {
    use core::marker::PhantomData;
    use syan::parse::recurse;

    #[recurse]
    mod m {
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
        pub enum Foo<T> {
            Nest(Box<Foo<T>>),
            Lit(PhantomData<T>),
        }
    }

    mod v {
        syan::visit::visitor!(
            crate::disjoint_params::m::Expr,
            crate::disjoint_params::m::Foo
        );
    }

    #[test]
    fn disjoint_param_cycles_one_visitor() {
        let e: m::Expr<()> = m::Expr::Nest(Box::new(m::Expr::Lit(PhantomData)));
        let f: m::Foo<u8> = m::Foo::Nest(Box::new(m::Foo::Lit(PhantomData)));
        // The tuple of closures fixes the union `<S = (), T = u8>` from the two argument types.
        let mut ec = 0usize;
        let mut fc = 0usize;
        e.visit((|_: &m::Expr<()>| ec += 1, |_: &m::Foo<u8>| fc += 1));
        f.visit((|_: &m::Expr<()>| ec += 1, |_: &m::Foo<u8>| fc += 1));
        assert_eq!((ec, fc), (2, 2), "two Expr nodes + two Foo nodes");
    }
}
