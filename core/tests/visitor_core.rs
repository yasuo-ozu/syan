//! Visitor fundamentals: basic descent, visit_mut, hygiene, local types, where-clauses, generics.
#![allow(dead_code)]

// Basic descent through Box; struct visitors, single closures, tuples of closures.
mod basic {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Debug, Ast)]
    #[subast(crate::basic::Stmt)]
    pub enum Expr<S> {
        Stmt(Box<Stmt<S>>),
        Other(PhantomData<S>),
    }

    #[derive(Debug, Ast)]
    #[subast(crate::basic::Expr)]
    pub enum Stmt<S> {
        Expr(Box<Expr<S>>),
        Other(PhantomData<S>),
    }

    pub mod visit {
        syan::visit::visitor!(super::Expr, super::Stmt);
    }

    fn sample() -> Expr<()> {
        Expr::Stmt(Box::new(Stmt::Expr(Box::new(Expr::Other(PhantomData)))))
    }

    #[derive(Default)]
    struct Counter {
        exprs: usize,
        stmts: usize,
    }

    impl<S> visit::Visit<S> for Counter {
        fn visit_expr(&mut self, i: &Expr<S>) {
            self.exprs += 1;
            visit::visit_expr(self, i);
        }
        fn visit_stmt(&mut self, i: &Stmt<S>) {
            self.stmts += 1;
            visit::visit_stmt(self, i);
        }
    }

    #[test]
    fn struct_visitor_counts_nodes() {
        let ast = sample();
        let mut counter = Counter::default();
        ast.visit(&mut counter);
        assert_eq!(counter.exprs, 2, "outer Expr::Stmt + inner Expr::Other");
        assert_eq!(counter.stmts, 1, "the single Stmt::Expr");
    }

    #[test]
    fn single_closure_visitor() {
        let mut exprs = 0usize;
        sample().visit(|_e: &Expr<()>| exprs += 1);
        assert_eq!(exprs, 2);

        let mut stmts = 0usize;
        sample().visit(|_s: &Stmt<()>| stmts += 1);
        assert_eq!(stmts, 1);
    }

    #[test]
    fn tuple_of_closures_single_traversal() {
        let mut exprs = 0usize;
        let mut stmts = 0usize;
        sample().visit((
            |_s: &Stmt<()>| stmts += 1,
            |_e: &Expr<()>| exprs += 1,
        ));
        assert_eq!(exprs, 2);
        assert_eq!(stmts, 1);
    }
}

// visit_mut mirror: mutate nodes in place via closures and struct visitors.
mod mutable {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Debug, Ast)]
    pub enum Expr<S> {
        Add(Box<Expr<S>>, Box<Expr<S>>),
        Lit(i64, PhantomData<S>),
    }

    pub mod visit {
        syan::visit::visitor!(super::Expr);
    }

    fn sample() -> Expr<()> {
        Expr::Add(
            Box::new(Expr::Lit(1, PhantomData)),
            Box::new(Expr::Add(
                Box::new(Expr::Lit(2, PhantomData)),
                Box::new(Expr::Lit(3, PhantomData)),
            )),
        )
    }

    fn sum(e: &Expr<()>) -> i64 {
        let mut s = 0;
        e.visit(|x: &Expr<()>| {
            if let Expr::Lit(n, _) = x {
                s += *n;
            }
        });
        s
    }

    #[test]
    fn mut_closure_increments_literals() {
        let mut ast = sample();
        ast.visit_mut(|x: &mut Expr<()>| {
            if let Expr::Lit(n, _) = x {
                *n += 1;
            }
        });
        assert_eq!(sum(&ast), (1 + 1) + (2 + 1) + (3 + 1));
    }

    #[test]
    fn struct_mut_visitor_doubles() {
        struct Doubler;
        impl<S> visit::VisitMut<S> for Doubler {
            fn visit_expr_mut(&mut self, i: &mut Expr<S>) {
                if let Expr::Lit(n, _) = i {
                    *n *= 2;
                }
                visit::visit_expr_mut(self, i);
            }
        }
        let mut ast = sample();
        ast.visit_mut(&mut Doubler);
        assert_eq!(sum(&ast), 2 + 4 + 6);
    }
}

// Hygiene: visited types may name params/fields like the generated machinery (`__V`, `this`, `i`).
#[allow(non_camel_case_types)]
mod hygiene {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Debug, Ast)]
    pub enum Node<__V, __T, __H, __F, __A, __B> {
        Rec(Box<Node<__V, __T, __H, __F, __A, __B>>),
        Leaf(PhantomData<(__V, __T, __H, __F, __A, __B)>),
    }

    pub mod visit {
        syan::visit::visitor!(crate::hygiene::Node);
    }

    type N = Node<(), (), (), (), (), ()>;

    fn sample() -> N {
        Node::Rec(Box::new(Node::Leaf(PhantomData)))
    }

    #[test]
    fn closure_visitor_with_helper_named_params() {
        let mut n = 0usize;
        sample().visit(|_x: &N| n += 1);
        assert_eq!(n, 2, "outer Rec + inner Leaf");
    }

    // The indexed tuple-closure helper family (`__F0`/`__T0`/…) is protected by `fresh_prefix`.
    #[derive(Debug, Ast)]
    pub enum Tup<__F0, __T0> {
        Rec(Box<Tup<__F0, __T0>>),
        Leaf(PhantomData<(__F0, __T0)>),
    }

    pub mod vtup {
        syan::visit::visitor!(crate::hygiene::Tup);
    }

    type T = Tup<(), ()>;

    #[test]
    fn indexed_helper_named_params_via_tuple_of_closures() {
        let ast: T = Tup::Rec(Box::new(Tup::Leaf(PhantomData)));
        let mut a = 0usize;
        let mut b = 0usize;
        // The tuple-of-closures path instantiates `__F0`/`__T0` tuple impls, which must not collide
        // with the visited type's own `__F0`/`__T0` params.
        ast.visit((|_x: &T| a += 1, |_x: &T| b += 1));
        assert_eq!((a, b), (2, 2));
    }

    // Value bindings (the generated receiver `this` / scrutinee `i`) are span-isolated from user
    // idents, so a visited type may have followed fields literally named `this` and `i`.
    #[derive(Debug, Ast)]
    pub enum Leaf<S> {
        U(PhantomData<S>),
    }

    #[derive(Debug, Ast)]
    #[subast(crate::hygiene::Leaf)]
    pub struct Names<S> {
        pub this: Box<Leaf<S>>,
        pub i: Leaf<S>,
    }

    pub mod vnames {
        syan::visit::visitor!(crate::hygiene::Names, crate::hygiene::Leaf);
    }

    #[test]
    fn fields_named_this_and_i_are_traversed() {
        let n: Names<()> = Names {
            this: Box::new(Leaf::U(PhantomData)),
            i: Leaf::U(PhantomData),
        };
        let mut leaves = 0usize;
        n.visit(|_l: &Leaf<()>| leaves += 1);
        assert_eq!(leaves, 2, "both the `this` and `i` fields were visited");
    }

    #[test]
    fn tuple_and_struct_visitors_with_helper_named_params() {
        struct Counter(usize);
        impl<__V, __T, __H, __F, __A, __B> visit::Visit<__V, __T, __H, __F, __A, __B> for Counter {
            fn visit_node(&mut self, i: &Node<__V, __T, __H, __F, __A, __B>) {
                self.0 += 1;
                visit::visit_node(self, i);
            }
        }
        let mut c = Counter(0);
        sample().visit(&mut c);
        assert_eq!(c.0, 2);
    }
}

// A field whose type is local/only-imported: it stays a leaf and its type is never re-emitted.
#[allow(dead_code)]
mod local_types {
    mod helper {
        #[derive(Debug)]
        pub struct Local;
    }

    mod ast {
        use super::helper::Local;
        use core::marker::PhantomData;
        use syan::visit::Ast;

        // `Local` is intentionally a leaf; `Rec` is followed via implicit self-recursion.
        #[derive(Debug, Ast)]
        #[subast()]
        pub enum Expr<S> {
            Lit(Local, PhantomData<S>),
            Rec(Box<Expr<S>>),
        }
    }

    mod vis {
        syan::visit::visitor!(super::ast::Expr);
    }

    #[test]
    fn local_leaf_field_type() {
        use ast::Expr;
        use helper::Local;
        let tree: Expr<()> = Expr::Rec(Box::new(Expr::Lit(Local, core::marker::PhantomData)));
        let mut n = 0usize;
        tree.visit(|_e: &Expr<()>| n += 1);
        assert_eq!(n, 2);
    }
}

// A visited type's `where`-clause is threaded onto every generated item that names the type.
#[allow(dead_code)]
mod where_clause {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    pub trait Bound {}
    impl Bound for () {}

    // The bound is written as a resolvable path because the generated items live in `mod v` — the
    // same canonical-path requirement the `#[subast]` paths have.
    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S>
    where
        S: crate::where_clause::Bound,
    {
        Nest(Box<Expr<S>>),
        Leaf(PhantomData<S>),
    }

    pub mod v {
        syan::visit::visitor!(crate::where_clause::Expr);
    }

    #[test]
    fn where_bounded_visitor_compiles_and_runs() {
        let e: Expr<()> = Expr::Nest(Box::new(Expr::Leaf(PhantomData)));
        let mut n = 0usize;
        e.visit(|_: &Expr<()>| n += 1);
        assert_eq!(n, 2, "outer Nest + inner Leaf");
    }
}

// A single visitor over types with different generic arities; the trait is keyed on the union.
mod generics {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast(crate::generics::BinOp)]
    pub enum Expr<S, Tokens> {
        Bin(Box<Expr<S, Tokens>>, BinOp<S>, Box<Expr<S, Tokens>>),
        Lit(i64, PhantomData<(S, Tokens)>),
    }

    #[derive(Ast)]
    pub enum BinOp<S> {
        Add(PhantomData<S>),
        Mul(PhantomData<S>),
    }

    pub mod visit {
        syan::visit::visitor!(super::Expr, super::BinOp);
    }

    #[test]
    fn visitor_over_mixed_arity_types() {
        let ast: Expr<(), ()> = Expr::Bin(
            Box::new(Expr::Lit(1, PhantomData)),
            BinOp::Add(PhantomData),
            Box::new(Expr::Bin(
                Box::new(Expr::Lit(2, PhantomData)),
                BinOp::Mul(PhantomData),
                Box::new(Expr::Lit(3, PhantomData)),
            )),
        );

        let mut exprs = 0usize;
        let mut ops = 0usize;
        ast.visit((
            |_e: &Expr<(), ()>| exprs += 1,
            |_o: &BinOp<()>| ops += 1,
        ));
        assert_eq!(exprs, 5, "2 Bin + 3 Lit");
        assert_eq!(ops, 2, "Add + Mul");
    }
}

// A where-bounded param not shared by all visited types becomes a per-method generic (struct-only).
#[allow(dead_code)]
mod union_where_unshared {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    pub trait Bound {}

    #[derive(Ast)]
    #[subast()]
    pub struct Bounded<S>
    where
        S: Bound,
    {
        pub _p: PhantomData<S>,
    }

    #[derive(Ast)]
    #[subast()]
    pub struct Plain {
        pub _x: u8,
    }

    mod v {
        // The generated `where S: Bound` lands here, so the user trait must be in scope (a where-bound
        // naming a user trait by bare path needs importing).
        use crate::union_where_unshared::Bound;
        syan::visit::visitor!(crate::union_where_unshared::Bounded, crate::union_where_unshared::Plain);
    }

    struct MyType;
    impl Bound for MyType {}

    #[derive(Default)]
    struct Counter {
        bounded: usize,
        plain: usize,
    }

    // The trait is keyed on the shared params (none); `visit_bounded` carries `S` as a method generic.
    impl v::Visit for Counter {
        fn visit_bounded<S: Bound>(&mut self, i: &Bounded<S>) {
            self.bounded += 1;
            v::visit_bounded(self, i);
        }
        fn visit_plain(&mut self, i: &Plain) {
            self.plain += 1;
            v::visit_plain(self, i);
        }
    }

    #[test]
    fn visits_both_via_struct_visitor() {
        let b: Bounded<MyType> = Bounded { _p: PhantomData };
        let p = Plain { _x: 7 };
        let mut c = Counter::default();
        b.visit(&mut c);
        p.visit(&mut c);
        assert_eq!(c.bounded, 1);
        assert_eq!(c.plain, 1, "the param-less `Plain` is visited without choosing an `S`");
    }
}
