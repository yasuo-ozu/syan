//! The container-edit views over the non-`Vec` sequence containers (`#[seq]` on `VecDeque`, `Punctuated`),
//! and that an **unmarked** `Vec`/`Option` slot is still traversed by a closure (its `visit_*_mut` hook
//! fires per element via the ordinary descent — no view method is generated for an unmarked field).
#![allow(dead_code)]

use core::marker::PhantomData;

// ── VecDeque ─────────────────────────────────────────────────────────────────────────────────────
mod vecdeque {
    use super::*;
    use std::collections::VecDeque;
    use syan::visit::{Ast, SeqView};

    #[derive(Debug, Ast)]
    pub struct Stmt<S>(pub i64, pub PhantomData<S>);

    #[derive(Debug, Ast)]
    #[subast(crate::vecdeque::Stmt)]
    pub struct Holder<S> {
        #[seq]
        pub items: VecDeque<Stmt<S>>,
    }

    pub mod v {
        syan::visit::visitor!(super::Holder, super::Stmt);
    }

    struct Editor;
    impl<S> v::VisitMut<S> for Editor {
        fn visit_stmt_seq<V: SeqView<Stmt<S>>>(&mut self, v: &mut V) {
            for s in v.view_iter_mut() {
                if s.0 == 2 {
                    *s = Stmt(102, PhantomData);
                }
            }
            v.retain_mut(|s| s.0 != 0);
            v.push(Stmt(9, PhantomData));
        }
    }

    #[test]
    fn vecdeque_edits_and_push() {
        let mut h: Holder<()> = Holder {
            items: VecDeque::from(vec![
                Stmt(0, PhantomData),
                Stmt(1, PhantomData),
                Stmt(2, PhantomData),
            ]),
        };
        h.visit_mut(&mut Editor);
        assert_eq!(
            h.items.iter().map(|s| s.0).collect::<Vec<_>>(),
            vec![1, 102, 9]
        );
    }
}

// ── Punctuated ─────────────────────────────────────────────────────────────────────────────────
mod punct {
    use super::*;
    use syan::nested::punctuated::Punctuated;
    use syan::visit::{Ast, SeqView};

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct Comma;

    #[derive(Debug, Ast)]
    pub struct Stmt<S>(pub i64, pub PhantomData<S>);

    #[derive(Debug, Ast)]
    #[subast(crate::punct::Stmt)]
    pub struct Holder<S> {
        #[seq]
        pub items: Punctuated<Stmt<S>, Comma>,
    }

    pub mod v {
        syan::visit::visitor!(super::Holder, super::Stmt);
    }

    struct Editor;
    impl<S> v::VisitMut<S> for Editor {
        fn visit_stmt_seq<V: SeqView<Stmt<S>>>(&mut self, v: &mut V) {
            v.retain_mut(|s| s.0 != 0); // drop zeros
            v.push(Stmt(9, PhantomData)); // `Comma: Default` synthesizes the separator
        }
    }

    fn vals<S>(p: &Punctuated<Stmt<S>, Comma>) -> Vec<i64> {
        p.iter().map(|s| s.0).collect()
    }

    #[test]
    fn punctuated_retain_and_push() {
        let mut items: Punctuated<Stmt<()>, Comma> = Punctuated::default();
        items.push(Stmt(0, PhantomData));
        items.push(Stmt(1, PhantomData));
        items.push(Stmt(0, PhantomData));
        items.push(Stmt(2, PhantomData));
        let mut h = Holder { items };
        h.visit_mut(&mut Editor);
        assert_eq!(
            vals(&h.items),
            vec![1, 2, 9],
            "zeros dropped, 9 appended through the separator"
        );
    }
}

// ── a closure still visits every element of a Vec / Option slot (via the default seq/opt descent) ──
mod closure_over_slot {
    use super::*;
    use syan::visit::Ast;

    #[derive(Debug, Ast)]
    pub struct Stmt<S>(pub i64, pub PhantomData<S>);

    #[derive(Debug, Ast)]
    #[subast(crate::closure_over_slot::Stmt)]
    pub struct Block<S> {
        pub stmts: Vec<Stmt<S>>,
        pub tail: Option<Stmt<S>>,
    }

    pub mod v {
        syan::visit::visitor!(super::Block, super::Stmt);
    }

    #[test]
    fn closure_runs_for_every_seq_and_opt_element() {
        let mut block: Block<()> = Block {
            stmts: vec![
                Stmt(1, PhantomData),
                Stmt(2, PhantomData),
                Stmt(3, PhantomData),
            ],
            tail: Some(Stmt(10, PhantomData)),
        };
        // The `Vec`/`Option` fields are UNMARKED (no `#[seq]`/`#[opt]`), so there is no view method —
        // they are traversed by the ordinary descent, and the closure's `visit_stmt_mut` hook fires for
        // each element all the same.
        block.visit_mut(|s: &mut Stmt<()>| s.0 += 100);
        assert_eq!(
            block.stmts.iter().map(|s| s.0).collect::<Vec<_>>(),
            vec![101, 102, 103]
        );
        assert_eq!(block.tail.as_ref().map(|s| s.0), Some(110));
    }

    #[test]
    fn closure_counts_all_elements() {
        let block: Block<()> = Block {
            stmts: vec![Stmt(1, PhantomData), Stmt(2, PhantomData)],
            tail: Some(Stmt(3, PhantomData)),
        };
        let mut n = 0usize;
        // immutable closure over the shared side, same traversal shape.
        block.visit(|_s: &Stmt<()>| n += 1);
        assert_eq!(n, 3, "two in the Vec + one in the Option");
    }
}
