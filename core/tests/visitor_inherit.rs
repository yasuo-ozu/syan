//! Visitor inheritance `visitor!(base => New)`: wider arity, multi-level, and over `#[recurse]` bases.
#![allow(dead_code)]

// A `New` visitor inherits its base's methods (supertrait) and adds methods for the new types.
mod basic {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    pub enum Type<S> {
        Unit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast(crate::basic::Type)]
    pub enum Expr<S> {
        Typed(Box<Type<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast(crate::basic::Expr)]
    pub enum Stmt<S> {
        E(Box<Expr<S>>),
        Empty(PhantomData<S>),
    }

    pub mod base {
        syan::visit::visitor!(super::Type, super::Expr);
    }

    pub mod ext {
        syan::visit::visitor!(super::base => super::Stmt);
    }

    #[derive(Default)]
    struct Counter {
        types: u32,
        exprs: u32,
        stmts: u32,
    }

    impl<S> base::Visit<S> for Counter {
        fn visit_type(&mut self, i: &Type<S>) {
            self.types += 1;
            base::visit_type(self, i);
        }
        fn visit_expr(&mut self, i: &Expr<S>) {
            self.exprs += 1;
            base::visit_expr(self, i);
        }
    }

    impl<S> ext::Visit<S> for Counter {
        fn visit_stmt(&mut self, i: &Stmt<S>) {
            self.stmts += 1;
            ext::visit_stmt(self, i);
        }
    }

    #[test]
    fn inheriting_visitor_descends_into_base_types() {
        let ast: Stmt<()> = Stmt::E(Box::new(Expr::Typed(Box::new(Type::Unit(PhantomData)))));
        let mut counter = Counter::default();
        ast.visit(&mut counter);
        assert_eq!(counter.stmts, 1);
        assert_eq!(counter.exprs, 1, "reached the inherited Expr method");
        assert_eq!(counter.types, 1, "reached the inherited Type method");
    }
}

// Extension whose generic union is *wider* than the base's: the new trait must reference the
// supertrait at the *base's* arity (`base::Visit<S>`), not the widened union (`base::Visit<S, T>`
// would be E0107).
mod arity {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    pub enum Type<S> {
        Unit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast(crate::arity::Type)]
    pub enum Expr<S> {
        Typed(Box<Type<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast(crate::arity::Expr)]
    pub enum Stmt<S, T> {
        E(Box<Expr<S>>),
        Tagged(PhantomData<T>),
    }

    pub mod base {
        syan::visit::visitor!(crate::arity::Type, crate::arity::Expr);
    }

    pub mod ext {
        syan::visit::visitor!(crate::arity::base => crate::arity::Stmt);
    }

    #[derive(Default)]
    struct Counter {
        types: u32,
        exprs: u32,
        stmts: u32,
    }

    impl<S> base::Visit<S> for Counter {
        fn visit_type(&mut self, i: &Type<S>) {
            self.types += 1;
            base::visit_type(self, i);
        }
        fn visit_expr(&mut self, i: &Expr<S>) {
            self.exprs += 1;
            base::visit_expr(self, i);
        }
    }

    impl<S, T> ext::Visit<S, T> for Counter {
        fn visit_stmt(&mut self, i: &Stmt<S, T>) {
            self.stmts += 1;
            ext::visit_stmt(self, i);
        }
    }

    #[test]
    fn inheriting_visitor_with_extra_generic_param() {
        let ast: Stmt<(), ()> = Stmt::E(Box::new(Expr::Typed(Box::new(Type::Unit(PhantomData)))));
        let mut counter = Counter::default();
        ast.visit(&mut counter);
        assert_eq!(counter.stmts, 1);
        assert_eq!(counter.exprs, 1, "reached the inherited Expr method");
        assert_eq!(counter.types, 1, "reached the inherited Type method");
    }

    #[test]
    fn inheriting_closure_over_wider_arity() {
        let ast: Stmt<(), ()> = Stmt::Tagged(PhantomData);
        let mut stmts = 0usize;
        ast.visit(|_s: &Stmt<(), ()>| stmts += 1);
        assert_eq!(stmts, 1);
    }
}

// Multi-level `base => mid => top`: `top`'s `Driver` must satisfy *every* transitive supertrait
// (`base::Visit` too), carried through the `@an` ancestor chain in `__syan_visited`.
mod multilevel {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    pub enum Type<S> {
        Unit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast(crate::multilevel::Type)]
    pub enum Expr<S> {
        Typed(Box<Type<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast(crate::multilevel::Expr)]
    pub enum Stmt<S> {
        E(Box<Expr<S>>),
        Empty(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast(crate::multilevel::Stmt)]
    pub enum Item<S> {
        S(Box<Stmt<S>>),
        Nil(PhantomData<S>),
    }

    pub mod base {
        syan::visit::visitor!(crate::multilevel::Type, crate::multilevel::Expr);
    }
    pub mod mid {
        syan::visit::visitor!(crate::multilevel::base => crate::multilevel::Stmt);
    }
    pub mod top {
        syan::visit::visitor!(crate::multilevel::mid => crate::multilevel::Item);
    }

    fn sample() -> Item<()> {
        Item::S(Box::new(Stmt::E(Box::new(Expr::Typed(Box::new(
            Type::Unit(PhantomData),
        ))))))
    }

    #[derive(Default)]
    struct Counter {
        types: u32,
        exprs: u32,
        stmts: u32,
        items: u32,
    }

    impl<S> base::Visit<S> for Counter {
        fn visit_type(&mut self, i: &Type<S>) {
            self.types += 1;
            base::visit_type(self, i);
        }
        fn visit_expr(&mut self, i: &Expr<S>) {
            self.exprs += 1;
            base::visit_expr(self, i);
        }
    }
    impl<S> mid::Visit<S> for Counter {
        fn visit_stmt(&mut self, i: &Stmt<S>) {
            self.stmts += 1;
            mid::visit_stmt(self, i);
        }
    }
    impl<S> top::Visit<S> for Counter {
        fn visit_item(&mut self, i: &Item<S>) {
            self.items += 1;
            top::visit_item(self, i);
        }
    }

    #[test]
    fn three_level_struct_visitor_descends_through_all_ancestors() {
        let mut c = Counter::default();
        sample().visit(&mut c);
        assert_eq!(c.items, 1);
        assert_eq!(c.stmts, 1, "reached the direct parent (mid) method");
        assert_eq!(c.exprs, 1, "reached the grandparent (base) method");
        assert_eq!(c.types, 1, "reached the grandparent (base) method");
    }

    #[test]
    fn three_level_closure_uses_transitive_driver() {
        let mut items = 0usize;
        sample().visit(|_i: &Item<()>| items += 1);
        assert_eq!(items, 1);
    }

    // 3-level chain AND arity widening at the leaf: top2's union is <S, T> while mid/base are <S>;
    // each transitive ancestor impl must be quantified over only its own param (S), leaving T out.
    #[derive(Ast)]
    #[subast(crate::multilevel::Stmt)]
    pub enum Item2<S, T> {
        S(Box<Stmt<S>>),
        Tag(PhantomData<T>),
    }

    pub mod top2 {
        syan::visit::visitor!(crate::multilevel::mid => crate::multilevel::Item2);
    }

    impl<S, T> top2::Visit<S, T> for Counter {
        fn visit_item2(&mut self, i: &Item2<S, T>) {
            self.items += 1;
            top2::visit_item2(self, i);
        }
    }

    #[test]
    fn three_level_with_arity_widening() {
        let ast: Item2<(), ()> = Item2::S(Box::new(Stmt::E(Box::new(Expr::Typed(Box::new(
            Type::Unit(PhantomData),
        ))))));
        let mut c = Counter::default();
        ast.visit(&mut c);
        assert_eq!((c.items, c.stmts, c.exprs, c.types), (1, 1, 1, 1));
    }
}

// Inheritance over a former-`#[recurse]` cycle: with natural types the base is an ordinary acyclic
// visitor, so (a) an acyclic New extends it and (b) a second independent cycle extends it.
mod over_recurse {
    use core::marker::PhantomData;
    use syan::parse::recurse;
    use syan::visit::Ast;

    #[recurse]
    mod ast {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast()]
        pub enum Expr<S> {
            Bin(Box<Expr<S>>),
            Lit(PhantomData<S>),
        }
    }

    mod base {
        syan::visit::visitor!(crate::over_recurse::ast::Expr);
    }

    // (a) acyclic New extends recurse base
    #[derive(Ast)]
    #[subast(crate::over_recurse::ast::Expr)]
    pub struct Program<S> {
        pub body: ast::Expr<S>,
    }

    mod nv {
        syan::visit::visitor!(crate::over_recurse::base => crate::over_recurse::Program);
    }

    #[derive(Default)]
    struct Walker {
        p: usize,
        e: usize,
    }

    impl<S> nv::Visit<S> for Walker {
        fn visit_program(&mut self, i: &Program<S>) {
            self.p += 1;
            nv::visit_program(self, i); // drills body → crosses into the inherited recurse visit_expr
        }
    }

    impl<S> base::Visit<S> for Walker {
        fn visit_expr(&mut self, i: &ast::Expr<S>) {
            self.e += 1;
            base::visit_expr(self, i);
        }
    }

    #[test]
    fn acyclic_extends_recurse() {
        let prog: Program<()> = Program {
            body: ast::Expr::Bin(Box::new(ast::Expr::Lit(PhantomData))),
        };
        let mut w = Walker::default();
        prog.visit(&mut w);
        assert_eq!((w.p, w.e), (1, 2), "Program + 2 Exprs (Bin + inner Lit)");
    }

    // (b) recurse New extends recurse base
    #[recurse]
    mod new_ast {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast()]
        pub enum Stmt<S> {
            Seq(Box<Stmt<S>>),
            Nop(PhantomData<S>),
        }
    }

    mod nv2 {
        syan::visit::visitor!(crate::over_recurse::base => crate::over_recurse::new_ast::Stmt);
    }

    #[derive(Default)]
    struct Both {
        e: usize,
        s: usize,
    }

    impl<S> nv2::Visit<S> for Both {
        fn visit_stmt(&mut self, i: &new_ast::Stmt<S>) {
            self.s += 1;
            nv2::visit_stmt(self, i);
        }
    }
    impl<S> base::Visit<S> for Both {
        fn visit_expr(&mut self, i: &ast::Expr<S>) {
            self.e += 1;
            base::visit_expr(self, i);
        }
    }

    #[test]
    fn recurse_extends_recurse() {
        let s: new_ast::Stmt<()> = new_ast::Stmt::Seq(Box::new(new_ast::Stmt::Nop(PhantomData)));
        let e: ast::Expr<()> = ast::Expr::Bin(Box::new(ast::Expr::Lit(PhantomData)));
        let mut b = Both::default();
        nv2::Visit::visit_stmt(&mut b, &s);
        base::Visit::visit_expr(&mut b, &e);
        assert_eq!(
            (b.s, b.e),
            (2, 2),
            "2 Stmts + 2 Exprs, one visitor over both cycles"
        );
    }
}

// Multi-level inheritance over a former-`#[recurse]` cycle through an acyclic intermediate:
// `base (Expr cycle) => mid (Program) => new (Module)` — plain three-level supertrait inheritance.
mod over_recurse_mid {
    use core::marker::PhantomData;
    use syan::parse::recurse;
    use syan::visit::Ast;

    #[recurse]
    mod ast {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Ast)]
        #[subast()]
        pub enum Expr<S> {
            Bin(Box<Expr<S>>),
            Lit(PhantomData<S>),
        }
    }

    mod base {
        syan::visit::visitor!(crate::over_recurse_mid::ast::Expr);
    }

    #[derive(Ast)]
    #[subast(crate::over_recurse_mid::ast::Expr)]
    pub struct Program<S> {
        pub body: ast::Expr<S>,
    }

    mod mid {
        syan::visit::visitor!(crate::over_recurse_mid::base => crate::over_recurse_mid::Program);
    }

    #[derive(Ast)]
    #[subast(crate::over_recurse_mid::Program)]
    pub struct Module<S> {
        pub prog: Program<S>,
    }

    mod nv {
        syan::visit::visitor!(crate::over_recurse_mid::mid => crate::over_recurse_mid::Module);
    }

    #[derive(Default)]
    struct Walker {
        m: usize,
        p: usize,
        e: usize,
    }

    impl<S> nv::Visit<S> for Walker {
        fn visit_module(&mut self, i: &Module<S>) {
            self.m += 1;
            nv::visit_module(self, i);
        }
    }
    impl<S> mid::Visit<S> for Walker {
        fn visit_program(&mut self, i: &Program<S>) {
            self.p += 1;
            mid::visit_program(self, i);
        }
    }
    impl<S> base::Visit<S> for Walker {
        fn visit_expr(&mut self, i: &ast::Expr<S>) {
            self.e += 1;
            base::visit_expr(self, i);
        }
    }

    #[test]
    fn three_level_over_recurse_base() {
        let m: Module<()> = Module {
            prog: Program {
                body: ast::Expr::Bin(Box::new(ast::Expr::Lit(PhantomData))),
            },
        };
        let mut w = Walker::default();
        m.visit(&mut w);
        assert_eq!(
            (w.m, w.p, w.e),
            (1, 1, 2),
            "Module + Program + 2 Exprs (Bin + inner Lit)"
        );
    }
}
