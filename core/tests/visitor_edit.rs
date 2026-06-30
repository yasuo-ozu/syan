//! Structural-edit coverage for the container-view model (Design B): a node held Vec-like / Option-like
//! by its parent gets a `visit_<t>_seq` / `visit_<t>_opt` method whose argument is a `SeqView` / `OptView`
//! of the owning collection, edited **in place** (no clone). A node only ever in a *fixed* slot gets no
//! such method (just the in-place `visit_<t>_mut`). Box-wrapped elements (`Vec<Box<T>>`) are
//! box-transparent. Covers `Vec`/`Option`, `edit_each`/`push`/`set`/`take`, a `#[recurse]` cycle, and a
//! regression that a plain `visit_*_mut`-only visitor still mutates every element.
#![allow(dead_code)]

use core::marker::PhantomData;

// ── fixed position: a followed type held directly in a field gets only `visit_*_mut` (no seq/opt) ──
mod fixed {
    use super::*;
    use syan::visit::Ast;

    #[derive(Debug, Ast)]
    pub enum Leaf<S> {
        A(PhantomData<S>),
        B(PhantomData<S>),
    }

    #[derive(Debug, Ast)]
    #[subast(crate::fixed::Leaf)]
    pub struct Wrap<S> {
        pub inner: Leaf<S>, // fixed slot — descended in place, never structurally edited
    }

    pub mod v {
        syan::visit::visitor!(super::Wrap, super::Leaf);
    }

    // Only `visit_leaf_mut` exists for `Leaf` (it is never held in a Vec/Option) — mutate it in place.
    struct ToB(usize);
    impl<S> v::VisitMut<S> for ToB {
        fn visit_leaf_mut(&mut self, l: &mut Leaf<S>) {
            self.0 += 1;
            *l = Leaf::B(PhantomData);
        }
    }

    #[test]
    fn fixed_field_visited_in_place() {
        let mut w: Wrap<()> = Wrap { inner: Leaf::A(PhantomData) };
        let mut to_b = ToB(0);
        w.visit_mut(&mut to_b);
        assert_eq!(to_b.0, 1, "the fixed Leaf was visited once");
        assert!(matches!(w.inner, Leaf::B(_)), "and mutated in place");
    }
}

// ── plain `visit_*_mut` is unchanged: a `()`-returning override still runs for every Vec element ──
mod plain_mut {
    use super::*;
    use syan::visit::Ast;

    #[derive(Debug, Ast)]
    pub struct N<S>(pub i64, pub PhantomData<S>);

    #[derive(Debug, Ast)]
    #[subast(crate::plain_mut::N)]
    pub struct Holder<S> {
        pub items: Vec<N<S>>,
    }

    pub mod v {
        syan::visit::visitor!(super::Holder, super::N);
    }

    struct Doubler;
    impl<S> v::VisitMut<S> for Doubler {
        // No view override — the default `visit_n_seq` descends each element through `visit_n_mut`.
        fn visit_n_mut(&mut self, n: &mut N<S>) {
            n.0 *= 2;
        }
    }

    #[test]
    fn visit_mut_only_still_mutates_every_element() {
        let mut h: Holder<()> = Holder {
            items: vec![N(1, PhantomData), N(2, PhantomData), N(3, PhantomData)],
        };
        h.visit_mut(&mut Doubler);
        let vals: Vec<i64> = h.items.iter().map(|n| n.0).collect();
        assert_eq!(vals, vec![2, 4, 6], "all doubled in place, nothing removed");
    }
}

// ── Vec + Option views on a plain struct: edit_each / push / set / take ──────────────────────────
mod views {
    use super::*;
    use syan::visit::{Ast, OptView, SeqView};

    #[derive(Debug, Ast)]
    pub struct Stmt<S>(pub i64, pub PhantomData<S>);

    #[derive(Debug, Ast)]
    #[subast(crate::views::Stmt)]
    pub struct Block<S> {
        #[seq]
        pub stmts: Vec<Stmt<S>>,
        #[opt]
        pub tail: Option<Stmt<S>>,
    }

    pub mod v {
        syan::visit::visitor!(super::Block, super::Stmt);
    }

    // Drop 0s, replace 2 -> 102, and append a 7 sentinel; fill/replace the Option tail.
    struct Editor;
    impl<S> v::VisitMut<S> for Editor {
        fn visit_stmt_seq<V: SeqView<Stmt<S>>>(&mut self, v: &mut V) {
            v.edit_each(|c| match c.get().0 {
                0 => c.remove(),
                2 => c.replace(Stmt(102, PhantomData)),
                _ => {}
            });
            v.push(Stmt(7, PhantomData)); // insert into the (possibly now-shorter) collection
        }
        fn visit_stmt_opt<O: OptView<Stmt<S>>>(&mut self, v: &mut O) {
            match v.get().map(|s| s.0) {
                Some(0) => v.clear(),
                None => v.set(Stmt(5, PhantomData)), // fill an empty slot
                _ => {}
            }
        }
    }

    #[test]
    fn seq_edit_each_and_push() {
        let mut b: Block<()> = Block {
            stmts: vec![
                Stmt(0, PhantomData),
                Stmt(1, PhantomData),
                Stmt(2, PhantomData),
                Stmt(0, PhantomData),
            ],
            tail: None,
        };
        b.visit_mut(&mut Editor);
        let vals: Vec<i64> = b.stmts.iter().map(|s| s.0).collect();
        assert_eq!(vals, vec![1, 102, 7], "0s removed, 2->102, 7 appended");
        assert_eq!(b.tail.as_ref().map(|s| s.0), Some(5), "empty Option filled with 5");
    }

    #[test]
    fn opt_take_clears() {
        let mut b: Block<()> = Block { stmts: vec![], tail: Some(Stmt(0, PhantomData)) };
        b.visit_mut(&mut Editor);
        assert_eq!(b.stmts.iter().map(|s| s.0).collect::<Vec<_>>(), vec![7], "push into empty Vec");
        assert!(b.tail.is_none(), "a zero tail was cleared");
    }
}

// ── edits through `Vec<Box<_>>` inside a `#[recurse]` cycle (box-transparent view) ───────────────
mod rec {
    use super::*;
    use syan::parse::recurse;
    use syan::visit::SeqView;

    #[recurse]
    mod ast {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Debug, Ast)]
        #[subast()]
        #[allow(clippy::vec_box)]
        pub enum Expr<S> {
            Many(#[seq] Vec<Box<Expr<S>>>), // self-recursive Vec-like slot -> visit_expr_seq
            Lit(i64, PhantomData<S>),
        }
    }

    mod v {
        syan::visit::visitor!(crate::rec::ast::Expr);
    }

    // Remove Lit(0), replace Lit(2) -> Lit(99); recurse into a nested Many via the per-node visit.
    struct Editor;
    impl<S> v::VisitMut<S> for Editor {
        fn visit_expr_seq<V: SeqView<ast::Expr<S>>>(&mut self, v: &mut V) {
            v.edit_each(|c| match c.get() {
                ast::Expr::Lit(0, _) => c.remove(),
                ast::Expr::Lit(2, _) => c.replace(ast::Expr::Lit(99, PhantomData)),
                _ => v::visit_expr_mut(self, c.get_mut()), // descend nested `Many`
            });
        }
    }

    fn lits(e: &ast::Expr<()>) -> Vec<i64> {
        match e {
            ast::Expr::Many(xs) => xs.iter().flat_map(|x| lits(x)).collect(),
            ast::Expr::Lit(n, _) => vec![*n],
        }
    }

    #[test]
    fn vec_of_box_edits_in_cycle() {
        let mut e: ast::Expr<()> = ast::Expr::Many(vec![
            Box::new(ast::Expr::Lit(0, PhantomData)), // removed
            Box::new(ast::Expr::Lit(1, PhantomData)), // kept
            Box::new(ast::Expr::Lit(2, PhantomData)), // -> 99
            Box::new(ast::Expr::Many(vec![
                Box::new(ast::Expr::Lit(0, PhantomData)), // removed (nested)
                Box::new(ast::Expr::Lit(5, PhantomData)),
                Box::new(ast::Expr::Lit(2, PhantomData)), // -> 99 (nested)
            ])),
        ]);
        e.visit_mut(&mut Editor);
        assert_eq!(lits(&e), vec![1, 99, 5, 99]);
    }
}

// ── `#[recurse]` cycle, Option-like slot: visit_expr_opt prunes a node in place ──────────────────
mod rec_opt {
    use super::*;
    use syan::parse::recurse;
    use syan::visit::OptView;

    #[recurse]
    mod ast {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Debug, Ast)]
        #[subast()]
        pub enum Expr<S> {
            Opt(#[opt] Option<Box<Expr<S>>>), // self-recursive Option-like slot -> visit_expr_opt
            Lit(i64, PhantomData<S>),
        }
    }

    mod v {
        syan::visit::visitor!(crate::rec_opt::ast::Expr);
    }

    // Drop a `Lit(0)` child (→ `None`), else descend into it.
    struct Pruner;
    impl<S> v::VisitMut<S> for Pruner {
        fn visit_expr_opt<O: OptView<ast::Expr<S>>>(&mut self, v: &mut O) {
            if matches!(v.get(), Some(ast::Expr::Lit(0, _))) {
                v.take();
            } else if let Some(e) = v.get_mut() {
                v::visit_expr_mut(self, e);
            }
        }
    }

    // Render as a string: `O(..)` / `O()` for Opt(Some)/Opt(None), `n` for Lit(n).
    fn shape(e: &ast::Expr<()>) -> String {
        match e {
            ast::Expr::Opt(Some(b)) => format!("O({})", shape(b)),
            ast::Expr::Opt(None) => "O()".to_string(),
            ast::Expr::Lit(n, _) => n.to_string(),
        }
    }

    #[test]
    fn opt_prune_in_cycle() {
        // O( O( 0 ) )  -> the inner Lit(0) is taken, leaving O( O() ).
        let mut e: ast::Expr<()> = ast::Expr::Opt(Some(Box::new(ast::Expr::Opt(Some(Box::new(
            ast::Expr::Lit(0, PhantomData),
        ))))));
        e.visit_mut(&mut Pruner);
        assert_eq!(shape(&e), "O(O())", "the deep Lit(0) was pruned through the cycle");
    }

    #[test]
    fn opt_keeps_nonzero() {
        let mut e: ast::Expr<()> = ast::Expr::Opt(Some(Box::new(ast::Expr::Lit(3, PhantomData))));
        e.visit_mut(&mut Pruner);
        assert_eq!(shape(&e), "O(3)", "a non-zero leaf is kept");
    }
}

// ── drill-in: a type held Vec-/Option-like INSIDE an unlisted (drilled) intermediate still gets the
//    view methods — usage is collected through the drill walk. ────────────────────────────────────
mod drill {
    use super::*;
    use syan::visit::{Ast, OptView, SeqView};

    #[derive(Debug, Ast)]
    pub struct Leaf<S>(pub i64, pub PhantomData<S>);

    // Unlisted intermediate (drilled through, no `visit_mid`): holds `Leaf` Vec-like AND Option-like.
    #[derive(Debug, Ast)]
    #[subast(crate::drill::Leaf)]
    pub struct Mid<S> {
        #[seq]
        pub leaves: Vec<Leaf<S>>,
        #[opt]
        pub last: Option<Leaf<S>>,
    }

    #[derive(Debug, Ast)]
    #[subast(crate::drill::Mid)]
    pub struct Top<S> {
        pub mid: Mid<S>, // Mid sits in a fixed slot; the visitor drills through it
    }

    pub mod v {
        // `Mid` is NOT listed → drilled; `Leaf` is reached (and edited) through it.
        syan::visit::visitor!(super::Top, super::Leaf);
    }

    struct Editor;
    impl<S> v::VisitMut<S> for Editor {
        fn visit_leaf_seq<V: SeqView<Leaf<S>>>(&mut self, v: &mut V) {
            v.retain_mut(|l| l.0 != 0); // drop zeros from the drilled Vec
        }
        fn visit_leaf_opt<O: OptView<Leaf<S>>>(&mut self, v: &mut O) {
            if matches!(v.get(), Some(l) if l.0 == 0) {
                v.clear(); // drop a zero from the drilled Option
            }
        }
    }

    #[test]
    fn edits_through_a_drilled_intermediate() {
        let mut top: Top<()> = Top {
            mid: Mid {
                leaves: vec![
                    Leaf(0, PhantomData),
                    Leaf(1, PhantomData),
                    Leaf(0, PhantomData),
                    Leaf(2, PhantomData),
                ],
                last: Some(Leaf(0, PhantomData)),
            },
        };
        top.visit_mut(&mut Editor);
        assert_eq!(
            top.mid.leaves.iter().map(|l| l.0).collect::<Vec<_>>(),
            vec![1, 2],
            "Vec<Leaf> inside the drilled Mid edited via visit_leaf_seq"
        );
        assert!(top.mid.last.is_none(), "Option<Leaf> inside the drilled Mid cleared via visit_leaf_opt");
    }
}

// ── multi-type `#[recurse]` cycle: edit the cross-edge `Vec<Box<Stmt>>` from `visit_stmt_seq` ────────
mod rec_cross {
    use super::*;
    use syan::parse::recurse;
    use syan::visit::SeqView;

    #[recurse]
    mod ast {
        use core::marker::PhantomData;
        use syan::visit::Ast;

        #[derive(Debug, Ast)]
        #[subast(crate::rec_cross::ast::Stmt)]
        #[allow(clippy::vec_box)]
        pub enum Expr<S> {
            Block(#[seq] Vec<Box<Stmt<S>>>), // cross-edge: holds `Stmt` Vec-like -> visit_stmt_seq
            Lit(i64, PhantomData<S>),
        }

        #[derive(Debug, Ast)]
        #[subast(crate::rec_cross::ast::Expr)]
        pub enum Stmt<S> {
            Expr(Box<Expr<S>>), // back-edge to the root
            Nop(i64, PhantomData<S>),
        }
    }

    mod v {
        syan::visit::visitor!(crate::rec_cross::ast::Expr, crate::rec_cross::ast::Stmt);
    }

    // Drop `Nop(0)` statements anywhere in the cycle; descend through the rest.
    struct Editor;
    impl<S> v::VisitMut<S> for Editor {
        fn visit_stmt_seq<V: SeqView<ast::Stmt<S>>>(&mut self, v: &mut V) {
            v.edit_each(|c| match c.get() {
                ast::Stmt::Nop(0, _) => c.remove(),
                _ => v::visit_stmt_mut(self, c.get_mut()), // descend (Stmt::Expr -> nested Block)
            });
        }
    }

    fn nops(e: &ast::Expr<()>) -> Vec<i64> {
        match e {
            ast::Expr::Block(ss) => ss
                .iter()
                .flat_map(|s| match &**s {
                    ast::Stmt::Nop(n, _) => vec![*n],
                    ast::Stmt::Expr(inner) => nops(inner),
                })
                .collect(),
            ast::Expr::Lit(..) => vec![],
        }
    }

    #[test]
    fn cross_edge_seq_edit_in_multitype_cycle() {
        let mut e: ast::Expr<()> = ast::Expr::Block(vec![
            Box::new(ast::Stmt::Nop(0, PhantomData)), // removed
            Box::new(ast::Stmt::Nop(1, PhantomData)), // kept
            Box::new(ast::Stmt::Expr(Box::new(ast::Expr::Block(vec![
                Box::new(ast::Stmt::Nop(0, PhantomData)), // removed (nested, via the back-edge)
                Box::new(ast::Stmt::Nop(5, PhantomData)), // kept
            ])))),
        ]);
        e.visit_mut(&mut Editor);
        assert_eq!(nops(&e), vec![1, 5], "Nop(0) dropped at both cycle depths");
    }
}
