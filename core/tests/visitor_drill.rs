//! Drill-in: selective `visit_*` + transitive drill-through unlisted intermediates
//! (containers, chains, dead-ends, path forms).
#![allow(dead_code, clippy::module_inception)]

// Drill through a single unlisted intermediate (`Cast`) to reach a visited `Type`.
mod basic {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Debug, Ast)]
    pub enum Type<S> {
        Unit(PhantomData<S>),
    }

    #[derive(Debug, Ast)]
    #[subast(crate::basic::Type)]
    pub struct Cast<S>(pub Type<S>);

    #[derive(Debug, Ast)]
    #[subast(crate::basic::Cast)]
    pub enum Expr<S> {
        Cast(Cast<S>),
        Lit(PhantomData<S>),
    }

    pub mod visit {
        syan::visit::visitor!(super::Expr, super::Type);
    }

    fn sample() -> Expr<()> {
        Expr::Cast(Cast(Type::Unit(PhantomData)))
    }

    #[test]
    fn closure_reaches_type_through_unlisted_cast() {
        let mut types = 0usize;
        sample().visit(|_t: &Type<()>| types += 1);
        assert_eq!(types, 1, "visit_expr drilled through Cast to the Type");
    }

    #[test]
    fn struct_visitor_has_expr_and_type_methods_only() {
        // No `visit_cast` exists — Cast is reached only by drilling.
        #[derive(Default)]
        struct Counter {
            exprs: usize,
            types: usize,
        }
        impl<S> visit::Visit<S> for Counter {
            fn visit_expr(&mut self, i: &Expr<S>) {
                self.exprs += 1;
                visit::visit_expr(self, i);
            }
            fn visit_type(&mut self, i: &Type<S>) {
                self.types += 1;
                visit::visit_type(self, i);
            }
        }
        let mut c = Counter::default();
        sample().visit(&mut c);
        assert_eq!(c.exprs, 1);
        assert_eq!(c.types, 1, "reached via drilling, not a visit_cast hop");
    }

    #[test]
    fn mut_visitor_reaches_type_through_cast() {
        struct C(usize);
        impl<S> visit::VisitMut<S> for C {
            fn visit_type_mut(&mut self, i: &mut Type<S>) {
                self.0 += 1;
                visit::visit_type_mut(self, i);
            }
        }
        let mut ast = sample();
        let mut c = C(0);
        ast.visit_mut(&mut c);
        assert_eq!(c.0, 1, "mut drilling reached Type through Cast");
    }
}

// A chain of unlisted intermediates, drilling inside containers, and a finite dead-end.
mod chain {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    // Chain: Expr -> Wrap -> Cast -> Type
    #[derive(Debug, Ast)]
    pub enum Type<S> {
        Unit(PhantomData<S>),
    }

    #[derive(Debug, Ast)]
    #[subast(crate::chain::Type)]
    pub struct Cast<S>(pub Type<S>);

    #[derive(Debug, Ast)]
    #[subast(crate::chain::Cast)]
    pub struct Wrap<S>(pub Cast<S>);

    #[derive(Debug, Ast)]
    #[subast(crate::chain::Wrap)]
    pub enum Expr<S> {
        W(Wrap<S>),
        Lit(PhantomData<S>),
    }

    pub mod chain {
        syan::visit::visitor!(super::Expr, super::Type);
    }

    #[test]
    fn drills_through_a_chain_of_intermediates() {
        let ast: Expr<()> = Expr::W(Wrap(Cast(Type::Unit(PhantomData))));
        let mut types = 0usize;
        ast.visit(|_t: &Type<()>| types += 1);
        assert_eq!(types, 1, "reached Type through Wrap -> Cast");
    }

    #[derive(Debug, Ast)]
    pub enum Leaf<S> {
        U(PhantomData<S>),
    }

    #[derive(Debug, Ast)]
    #[subast(crate::chain::Leaf)]
    pub struct Item<S>(pub Leaf<S>);

    #[derive(Debug, Ast)]
    #[subast(crate::chain::Item)]
    pub struct Block<S> {
        pub items: Vec<Item<S>>,
        pub opt: Option<Item<S>>,
    }

    pub mod container {
        syan::visit::visitor!(super::Block, super::Leaf);
    }

    #[test]
    fn drills_through_intermediates_in_containers() {
        let block: Block<()> = Block {
            items: vec![Item(Leaf::U(PhantomData)), Item(Leaf::U(PhantomData))],
            opt: Some(Item(Leaf::U(PhantomData))),
        };
        let mut leaves = 0usize;
        block.visit(|_l: &Leaf<()>| leaves += 1);
        assert_eq!(leaves, 3, "2 in the Vec + 1 in the Option, each drilled through Item");
    }

    // Finite dead-end: an unlisted intermediate reaching no visited type — a no-op, not an error.
    #[derive(Debug, Ast)]
    pub struct Dead<S>(pub i64, pub PhantomData<S>);

    #[derive(Debug, Ast)]
    #[subast(crate::chain::Dead)]
    pub enum ExprD<S> {
        D(Dead<S>),
        Lit(PhantomData<S>),
    }

    pub mod deadend {
        syan::visit::visitor!(super::ExprD);
    }

    #[test]
    fn finite_dead_end_is_a_noop_not_an_error() {
        let ast: ExprD<()> = ExprD::D(Dead(7, PhantomData));
        let mut exprs = 0usize;
        ast.visit(|_e: &ExprD<()>| exprs += 1);
        assert_eq!(exprs, 1, "the root ExprD; drilling Dead reached no visited node");
    }
}

// Regression fixes: aliased #[subast] on a visited type, a user type named `Option`, Box<Option<T>>.
mod fixes {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    mod other {
        use core::marker::PhantomData;
        use syan::visit::Ast;
        #[derive(Debug, Ast)]
        pub enum Real<S> {
            U(PhantomData<S>),
        }
    }

    use other::Real;
    use other::Real as Aliased;

    #[derive(Debug, Ast)]
    #[subast(crate::fixes::other::Real as Aliased)]
    pub enum Expr<S> {
        R(Aliased<S>),
        Lit(PhantomData<S>),
    }

    pub mod va {
        syan::visit::visitor!(crate::fixes::Expr, crate::fixes::other::Real);
    }

    #[test]
    fn aliased_subast_entry_dispatches_to_real_method() {
        let ast: Expr<()> = Expr::R(Aliased::U(PhantomData));
        let mut reals = 0usize;
        ast.visit(|_r: &Real<()>| reals += 1);
        assert_eq!(reals, 1, "field `Aliased` lowered to visit_real, no duplicate fetch");
    }

    #[derive(Debug, Ast)]
    pub enum Leaf<S> {
        U(PhantomData<S>),
    }

    mod weird {
        use super::Leaf;
        use syan::visit::Ast;
        // A user type whose name collides with the `Option` container keyword.
        #[derive(Debug, Ast)]
        #[subast(crate::fixes::Leaf)]
        pub struct Option<S>(pub Leaf<S>);
    }

    #[derive(Debug, Ast)]
    #[subast(crate::fixes::weird::Option)]
    pub enum Outer<S> {
        O(weird::Option<S>),
        Lit(PhantomData<S>),
    }

    pub mod vw {
        syan::visit::visitor!(crate::fixes::Outer, crate::fixes::Leaf);
    }

    #[test]
    fn user_type_named_like_a_container_is_a_node() {
        let ast: Outer<()> = Outer::O(weird::Option(Leaf::U(PhantomData)));
        let mut leaves = 0usize;
        ast.visit(|_l: &Leaf<()>| leaves += 1);
        assert_eq!(leaves, 1, "drilled through the user `Option` type to its Leaf");
    }

    // A distinct leaf type, since two visitors over the same type in one crate would emit
    // duplicate inherent `visit`/`visit_mut`.
    #[derive(Debug, Ast)]
    pub enum LeafB<S> {
        U(PhantomData<S>),
    }

    #[derive(Debug, Ast)]
    #[subast(crate::fixes::LeafB)]
    pub struct Holder<S> {
        pub boxed_opt: Box<Option<LeafB<S>>>,
    }

    pub mod vh {
        syan::visit::visitor!(crate::fixes::Holder, crate::fixes::LeafB);
    }

    #[test]
    fn box_around_option_derefs_in_if_let() {
        let some: Holder<()> = Holder {
            boxed_opt: Box::new(Some(LeafB::U(PhantomData))),
        };
        let mut n = 0usize;
        some.visit(|_l: &LeafB<()>| n += 1);
        assert_eq!(n, 1, "visited the LeafB inside Box<Option<_>>");

        let none: Holder<()> = Holder {
            boxed_opt: Box::new(None),
        };
        let mut n = 0usize;
        none.visit(|_l: &LeafB<()>| n += 1);
        assert_eq!(n, 0);
    }

    #[test]
    fn box_around_option_mut_side() {
        struct C(usize);
        impl<S> vh::VisitMut<S> for C {
            fn visit_leaf_b_mut(&mut self, i: &mut LeafB<S>) {
                self.0 += 1;
                vh::visit_leaf_b_mut(self, i);
            }
        }
        let mut h: Holder<()> = Holder {
            boxed_opt: Box::new(Some(LeafB::U(PhantomData))),
        };
        let mut c = C(0);
        h.visit_mut(&mut c);
        assert_eq!(c.0, 1);
    }
}

// Two same-last-segment intermediates (`a::Cast`/`b::Cast`) fetched distinctly by full path.
mod paths {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Debug, Ast)]
    pub enum Type<S> {
        Unit(PhantomData<S>),
    }

    pub mod a {
        use syan::visit::Ast;
        #[derive(Debug, Ast)]
        #[subast(crate::paths::Type)]
        pub struct Cast<S>(pub crate::paths::Type<S>);
    }

    pub mod b {
        use syan::visit::Ast;
        #[derive(Debug, Ast)]
        #[subast(crate::paths::Type)]
        pub struct Cast<S>(pub crate::paths::Type<S>);
    }

    use a::Cast;
    use b::Cast as BCast;

    #[derive(Debug, Ast)]
    #[subast(crate::paths::a::Cast, crate::paths::b::Cast as BCast)]
    pub enum Expr<S> {
        A(Cast<S>),
        B(BCast<S>),
    }

    pub mod visit {
        syan::visit::visitor!(super::Expr, super::Type);
    }

    #[test]
    fn distinct_same_named_intermediates_both_drilled() {
        let ast: Expr<()> = Expr::A(Cast(Type::Unit(PhantomData)));
        let mut n = 0usize;
        ast.visit(|_t: &Type<()>| n += 1);
        assert_eq!(n, 1, "drilled through a::Cast");

        let ast: Expr<()> = Expr::B(BCast(Type::Unit(PhantomData)));
        let mut n = 0usize;
        ast.visit(|_t: &Type<()>| n += 1);
        assert_eq!(n, 1, "drilled through b::Cast (fetched distinctly from a::Cast)");
    }
}
