//! Visiting a `#[recurse]` cycle: struct/closure/mut visitors over single, multi-root, multi-cycle,
//! and disjoint-param cycles. `#[recurse]` exposes the cycle as natural recursive types, so
//! `visitor!()` builds an ordinary acyclic visitor (one `visit_*` per type, no depth parameter).
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
        assert_eq!((c.e, c.s), (2, 1), "same shape as the shared walk, via &mut");
    }
}

// Closure visitors over a recurse cycle (the closure-over-recurse gap, now closed).
mod cycle {
    use core::marker::PhantomData;
    use syan::parse::recurse;

    #[recurse]
    mod ast {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast(crate::cycle::ast::Stmt)]
        pub enum Expr<S> {
            Stmt(Box<Stmt<S>>),
            Lit(PhantomData<S>),
        }

        #[derive(Ast)]
        #[subast(crate::cycle::ast::Expr)]
        pub enum Stmt<S> {
            Expr(Box<Expr<S>>),
            Nop(PhantomData<S>),
        }
    }

    mod v_ast {
        syan::visit::visitor!(crate::cycle::ast::Expr, crate::cycle::ast::Stmt);
    }

    #[derive(Default)]
    struct Counter {
        exprs: usize,
        stmts: usize,
    }

    impl<S> v_ast::Visit<S> for Counter {
        fn visit_expr(&mut self, i: &ast::Expr<S>) {
            self.exprs += 1;
            v_ast::visit_expr(self, i);
        }
        fn visit_stmt(&mut self, i: &ast::Stmt<S>) {
            self.stmts += 1;
            v_ast::visit_stmt(self, i);
        }
    }

    #[test]
    fn closure_over_recurse_cycle() {
        let e: ast::Expr<()> =
            ast::Expr::Stmt(Box::new(ast::Stmt::Expr(Box::new(ast::Expr::Lit(PhantomData)))));
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
        syan::visit::visitor!(crate::cycle::tree::Expr);
    }

    #[derive(Default)]
    struct Nodes(usize);

    impl<S> v_tree::Visit<S> for Nodes {
        fn visit_expr(&mut self, i: &tree::Expr<S>) {
            self.0 += 1;
            v_tree::visit_expr(self, i);
        }
    }

    #[test]
    fn visits_self_recursive_root() {
        let e: tree::Expr<()> = tree::Expr::Add(
            Box::new(tree::Expr::Lit(PhantomData)),
            Box::new(tree::Expr::Lit(PhantomData)),
        );
        let mut n = Nodes::default();
        v_tree::Visit::visit_expr(&mut n, &e);
        assert_eq!(n.0, 3, "the Add node + its two operands");
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

// One visitor!() over two mutually- and self-referential cycle types (A and B).
mod multiroot {
    use core::marker::PhantomData;
    use syan::parse::recurse;

    #[recurse]
    mod ast {
        use core::marker::PhantomData;
        use syan::visit::Ast;
        #[derive(Ast)]
        #[subast(crate::multiroot::ast::B)]
        #[allow(clippy::enum_variant_names)]
        pub enum A<S> {
            SelfA(Box<A<S>>),
            ToB(Box<B<S>>),
            Lit(PhantomData<S>),
        }
        #[derive(Ast)]
        #[subast(crate::multiroot::ast::A)]
        #[allow(clippy::enum_variant_names)]
        pub enum B<S> {
            ToA(Box<A<S>>),
            SelfB(Box<B<S>>),
            Lit(PhantomData<S>),
        }
    }

    mod v {
        syan::visit::visitor!(crate::multiroot::ast::A, crate::multiroot::ast::B);
    }

    #[derive(Default)]
    struct C {
        a: usize,
        b: usize,
    }
    impl<S> v::Visit<S> for C {
        fn visit_a(&mut self, i: &ast::A<S>) {
            self.a += 1;
            v::visit_a(self, i);
        }
        fn visit_b(&mut self, i: &ast::B<S>) {
            self.b += 1;
            v::visit_b(self, i);
        }
    }

    #[test]
    fn multiroot_via_visitor() {
        let x: ast::A<()> = ast::A::ToB(Box::new(ast::B::ToA(Box::new(
            ast::A::SelfA(Box::new(ast::A::Lit(PhantomData))),
        ))));
        let mut c = C::default();
        v::Visit::visit_a(&mut c, &x);
        assert_eq!((c.a, c.b), (3, 1));
    }
}

// One visitor!() spanning two independent (disjoint) cycles sharing a param (Expr and Type).
mod multicycle {
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

        #[derive(Ast)]
        #[subast()]
        pub enum Type<S> {
            Arrow(Box<Type<S>>),
            Unit(PhantomData<S>),
        }
    }

    mod v {
        syan::visit::visitor!(crate::multicycle::ast::Expr, crate::multicycle::ast::Type);
    }

    #[derive(Default)]
    struct C {
        e: usize,
        t: usize,
    }

    impl<S> v::Visit<S> for C {
        fn visit_expr(&mut self, i: &ast::Expr<S>) {
            self.e += 1;
            v::visit_expr(self, i);
        }
        fn visit_type(&mut self, i: &ast::Type<S>) {
            self.t += 1;
            v::visit_type(self, i);
        }
    }

    #[test]
    fn two_independent_cycles_one_visitor() {
        let e: ast::Expr<()> = ast::Expr::Nest(Box::new(ast::Expr::Lit(PhantomData)));
        let t: ast::Type<()> = ast::Type::Arrow(Box::new(ast::Type::Unit(PhantomData)));
        let mut c = C::default();
        e.visit(&mut c);
        t.visit(&mut c);
        assert_eq!((c.e, c.t), (2, 2), "each cycle traversed (2 nodes each), independently");
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
        syan::visit::visitor!(crate::disjoint_params::m::Expr, crate::disjoint_params::m::Foo);
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
