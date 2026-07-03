//! Container/tuple traversal shapes on ordinary (non-recurse) ASTs: nested containers,
//! container-of-tuple, tuple fields.
#![allow(dead_code)]

// Nested containers (`Vec<Option<T>>`, `Option<Vec<T>>`, `Vec<Vec<T>>`) are traversed.
mod nested {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub struct Leaf<S> {
        pub _p: PhantomData<S>,
    }

    #[derive(Ast)]
    #[subast(crate::nested::Leaf)]
    pub struct Holder<S> {
        pub vo: Vec<Option<Leaf<S>>>,
        pub ov: Option<Vec<Leaf<S>>>,
        pub vv: Vec<Vec<Leaf<S>>>,
    }

    mod v {
        syan::visit::visitor!(crate::nested::Leaf, crate::nested::Holder);
    }

    fn leaf<S>() -> Leaf<S> {
        Leaf { _p: PhantomData }
    }

    #[test]
    fn nested_containers_are_traversed() {
        let h: Holder<()> = Holder {
            vo: vec![Some(leaf()), None, Some(leaf())],
            ov: Some(vec![leaf(), leaf()]),
            vv: vec![vec![leaf()], vec![leaf(), leaf()]],
        };
        let mut n = 0usize;
        h.visit(|_: &Leaf<()>| n += 1);
        assert_eq!(n, 7, "2 (Vec<Option>) + 2 (Option<Vec>) + 3 (Vec<Vec>)");
    }

    #[test]
    fn nested_containers_visit_mut() {
        let mut h: Holder<()> = Holder { vo: vec![Some(leaf())], ov: None, vv: vec![] };
        let mut n = 0usize;
        h.visit_mut(|_: &mut Leaf<()>| n += 1);
        assert_eq!(n, 1);
    }

    #[syan::parse::recurse]
    mod rec {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast()]
        pub enum Expr<S> {
            Many(Vec<Option<Expr<S>>>),
            Lit(PhantomData<S>),
        }
    }

    mod rv {
        syan::visit::visitor!(crate::nested::rec::Expr);
    }

    #[derive(Default)]
    struct Counter(usize);
    impl<S> rv::Visit<S> for Counter {
        fn visit_expr(&mut self, i: &rec::Expr<S>) {
            self.0 += 1;
            rv::visit_expr(self, i);
        }
    }

    #[test]
    fn recurse_nested_container_is_traversed() {
        let e: rec::Expr<()> = rec::Expr::Many(vec![
            Some(rec::Expr::Lit(PhantomData)),
            None,
            Some(rec::Expr::Lit(PhantomData)),
        ]);
        let mut c = Counter::default();
        rv::Visit::visit_expr(&mut c, &e);
        assert_eq!(c.0, 3, "outer Expr + 2 inner (Vec<Option<Expr>> back-edges)");
    }
}

// A tuple nested inside a container (`Vec<(A,B)>`, `Option<(A,B)>`, `Box<(A,B)>`) has its elements visited.
mod container_of_tuple {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub struct Leaf<S> {
        pub _p: PhantomData<S>,
    }

    #[derive(Ast)]
    #[subast(crate::container_of_tuple::Leaf)]
    pub struct Holder<S> {
        pub top: (Leaf<S>, Leaf<S>),
        pub vec_of_tuples: Vec<(Leaf<S>, Leaf<S>)>,
        pub opt_of_tuple: Option<(Leaf<S>, Leaf<S>)>,
        pub boxed_tuple: Box<(Leaf<S>, Leaf<S>)>,
    }

    mod v {
        syan::visit::visitor!(crate::container_of_tuple::Leaf, crate::container_of_tuple::Holder);
    }

    fn leaf<S>() -> Leaf<S> {
        Leaf { _p: PhantomData }
    }

    #[test]
    fn container_of_tuple_visits_elements() {
        let h: Holder<()> = Holder {
            top: (leaf(), leaf()),
            vec_of_tuples: vec![(leaf(), leaf()), (leaf(), leaf())],
            opt_of_tuple: Some((leaf(), leaf())),
            boxed_tuple: Box::new((leaf(), leaf())),
        };
        let mut n = 0usize;
        h.visit(|_: &Leaf<()>| n += 1);
        assert_eq!(n, 10, "a container-of-tuple must visit its tuple elements");
    }

    #[test]
    fn container_of_tuple_visits_elements_mut() {
        let mut h: Holder<()> = Holder {
            top: (leaf(), leaf()),
            vec_of_tuples: vec![(leaf(), leaf())],
            opt_of_tuple: Some((leaf(), leaf())),
            boxed_tuple: Box::new((leaf(), leaf())),
        };
        let mut n = 0usize;
        h.visit_mut(|_: &mut Leaf<()>| n += 1);
        assert_eq!(n, 8, "the &mut side must also reach tuple elements inside containers");
    }
}

// A tuple-typed field is destructured and each element lowered (top-level + nested tuples).
mod tuple_field {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    pub enum Ty<S> {
        Unit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast(crate::tuple_field::Ty)]
    pub enum Expr<S> {
        Pair((Ty<S>, Ty<S>)),
        // Nested tuple with a leaf element exercises recursion + `_` binding for non-followed members.
        Triple((Ty<S>, (PhantomData<S>, Ty<S>))),
        Lit(PhantomData<S>),
    }

    pub mod v {
        syan::visit::visitor!(crate::tuple_field::Expr, crate::tuple_field::Ty);
    }

    #[test]
    fn tuple_field_visits_each_element() {
        let e = Expr::Pair((Ty::Unit(PhantomData), Ty::Unit(PhantomData)));
        let mut n = 0usize;
        e.visit(|_: &Ty<()>| n += 1);
        assert_eq!(n, 2, "both tuple elements should be visited");
    }

    #[test]
    fn nested_tuple_with_leaf_element() {
        let e = Expr::Triple((Ty::Unit(PhantomData), (PhantomData, Ty::Unit(PhantomData))));
        let mut n = 0usize;
        e.visit(|_: &Ty<()>| n += 1);
        assert_eq!(n, 2, "the two Ty elements (skipping the PhantomData leaf) should be visited");
    }
}
