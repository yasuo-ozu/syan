//! Container/tuple field-shape descent through a `#[recurse]` cycle (peel audit pins).
#![allow(dead_code)]

// Box-around-Option (`cont_box`) and tuple-typed fields peeled/dispatched element-by-element.
mod containers {
    use core::marker::PhantomData;
    use syan::parse::recurse;

    #[recurse]
    mod ast {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast()]
        #[allow(clippy::vec_box)]
        pub enum Expr<S> {
            // A Box wrapping the Option (`cont_box`) — patterns don't auto-deref Box.
            Opt(Box<Option<Box<Expr<S>>>>),
            Pair((Box<Expr<S>>, Box<Expr<S>>)),
            // A tuple mixing a followed element with a leaf.
            Tagged((Box<Expr<S>>, PhantomData<S>)),
            Many(Vec<Box<Expr<S>>>),
            OptIn(Option<Box<Expr<S>>>),
            Lit(PhantomData<S>),
        }
    }

    mod v_ast {
        syan::visit::visitor!(crate::containers::ast::Expr);
    }

    #[derive(Default)]
    struct Nodes(usize);

    impl<S> v_ast::Visit<S> for Nodes {
        fn visit_expr(&mut self, i: &ast::Expr<S>) {
            self.0 += 1;
            v_ast::visit_expr(self, i);
        }
    }

    fn count(e: &ast::Expr<()>) -> usize {
        let mut n = Nodes::default();
        v_ast::Visit::visit_expr(&mut n, e);
        n.0
    }

    #[test]
    fn box_around_option_some() {
        let e: ast::Expr<()> =
            ast::Expr::Opt(Box::new(Some(Box::new(ast::Expr::Lit(PhantomData)))));
        assert_eq!(count(&e), 2, "Box<Option<Box<Expr>>> descends through the Some");
    }

    #[test]
    fn box_around_option_none() {
        let e: ast::Expr<()> = ast::Expr::Opt(Box::new(None));
        assert_eq!(count(&e), 1, "None stops the descent");
    }

    #[test]
    fn tuple_field_both_operands() {
        let e: ast::Expr<()> = ast::Expr::Pair((
            Box::new(ast::Expr::Lit(PhantomData)),
            Box::new(ast::Expr::Lit(PhantomData)),
        ));
        assert_eq!(count(&e), 3, "tuple field visits both cycle-ref operands");
    }

    #[test]
    fn tuple_field_with_leaf() {
        let e: ast::Expr<()> =
            ast::Expr::Tagged((Box::new(ast::Expr::Lit(PhantomData)), PhantomData));
        assert_eq!(count(&e), 2, "leaf tuple element is skipped, cycle ref visited");
    }
}

// A tuple nested inside a Vec/Option container holding recursive refs.
mod container_of_tuple {
    use core::marker::PhantomData;
    use syan::parse::recurse;

    #[recurse]
    mod ast {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast()]
        #[allow(clippy::type_complexity)]
        pub enum Expr<S> {
            Pair((Box<Expr<S>>, Box<Expr<S>>)),
            VecPair(Vec<(Box<Expr<S>>, Box<Expr<S>>)>),
            OptPair(Option<(Box<Expr<S>>, Box<Expr<S>>)>),
            Lit(PhantomData<S>),
        }
    }

    mod v {
        syan::visit::visitor!(crate::container_of_tuple::ast::Expr);
    }

    #[derive(Default)]
    struct Counter {
        e: usize,
    }
    impl<S> v::Visit<S> for Counter {
        fn visit_expr(&mut self, i: &ast::Expr<S>) {
            self.e += 1;
            v::visit_expr(self, i);
        }
    }

    #[test]
    fn recurse_vec_of_tuple_back_edges() {
        let e: ast::Expr<()> = ast::Expr::VecPair(vec![
            (Box::new(ast::Expr::Lit(PhantomData)), Box::new(ast::Expr::Lit(PhantomData))),
            (Box::new(ast::Expr::Lit(PhantomData)), Box::new(ast::Expr::Lit(PhantomData))),
        ]);
        let mut c = Counter::default();
        v::Visit::visit_expr(&mut c, &e);
        assert_eq!(c.e, 5, "outer Expr + 4 tuple-nested back-edges");
    }

    #[test]
    fn recurse_opt_of_tuple_back_edges() {
        let e: ast::Expr<()> = ast::Expr::OptPair(Some((
            Box::new(ast::Expr::Lit(PhantomData)),
            Box::new(ast::Expr::Lit(PhantomData)),
        )));
        let mut c = Counter::default();
        v::Visit::visit_expr(&mut c, &e);
        assert_eq!(c.e, 3, "outer Expr + 2 tuple-nested back-edges");
    }
}
