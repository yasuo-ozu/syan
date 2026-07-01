//! Structural edits from a `visit_mut` visitor: a node opts in by overriding the container-edit hook
//! `visit_stmt_seq` / `visit_stmt_opt`, which receive a [`SeqView`] / [`OptView`] of the owning `Vec` /
//! `Option` and edit it **in place** (no clone) — so "drop this `Stmt`" lives in `visit_stmt_seq`, not in
//! a hand-written `visit_block_mut`. The plain in-place `visit_*_mut` interface is unchanged (its default
//! still runs), and the older parent-override style (editing the `&mut Vec`/`&mut Option` directly) works.

use core::marker::PhantomData;
use syan::visit::{Ast, OptView, SeqView};

#[derive(Debug, Ast)]
pub struct Stmt<S>(pub i64, pub PhantomData<S>);

#[derive(Debug, Ast)]
#[subast(crate::Stmt)]
pub struct Block<S> {
    #[seq]
    pub stmts: Vec<Stmt<S>>,
    #[opt]
    pub tail: Option<Stmt<S>>,
}

pub mod visit {
    syan::visit::visitor!(super::Block, super::Stmt);
}

// ── child-level edits: the container-edit views remove/replace in the parent's `Vec`/`Option` ──
struct Editor;
impl<S> visit::VisitMut<S> for Editor {
    fn visit_stmt_seq<V: SeqView<Stmt<S>>>(&mut self, v: &mut V) {
        for s in v.view_iter_mut() {
            if s.0 == 2 {
                *s = Stmt(102, PhantomData); // replace this node in place
            }
        }
        v.retain_mut(|s| s.0 != 0); // drop zero statements
    }
    fn visit_stmt_opt<O: OptView<Stmt<S>>>(&mut self, v: &mut O) {
        match v.get().map(|s| s.0) {
            Some(0) => v.clear(),                          // drop a zero tail
            Some(2) => v.set(Stmt(102, PhantomData)),      // replace the tail
            _ => {}
        }
    }
}

#[test]
fn child_level_edits_on_vec_and_option() {
    let mut block: Block<()> = Block {
        stmts: vec![
            Stmt(0, PhantomData),
            Stmt(1, PhantomData),
            Stmt(0, PhantomData),
            Stmt(2, PhantomData),
        ],
        tail: Some(Stmt(0, PhantomData)),
    };
    block.visit_mut(&mut Editor);

    let vals: Vec<i64> = block.stmts.iter().map(|s| s.0).collect();
    assert_eq!(vals, vec![1, 102], "zeros removed, 2 replaced by 102");
    assert!(block.tail.is_none(), "the `Option` tail (a zero) was removed");
}

#[test]
fn replace_then_keep_in_option() {
    let mut block: Block<()> = Block { stmts: vec![], tail: Some(Stmt(2, PhantomData)) };
    block.visit_mut(&mut Editor);
    assert_eq!(block.tail.as_ref().map(|s| s.0), Some(102), "the `Option` tail (a 2) was replaced");
}

// ── back-compat: overriding the *parent* and editing its `&mut Vec`/`&mut Option` directly still works ──
struct ParentEditor;
impl<S> visit::VisitMut<S> for ParentEditor {
    fn visit_block_mut(&mut self, b: &mut Block<S>) {
        b.stmts.retain(|s| s.0 != 0);
        b.stmts.push(Stmt(99, PhantomData));
        b.tail = None;
        visit::visit_block_mut(self, b); // descend
    }
}

#[test]
fn parent_override_still_works() {
    let mut block: Block<()> = Block {
        stmts: vec![Stmt(0, PhantomData), Stmt(1, PhantomData), Stmt(2, PhantomData)],
        tail: Some(Stmt(7, PhantomData)),
    };
    block.visit_mut(&mut ParentEditor);
    let vals: Vec<i64> = block.stmts.iter().map(|s| s.0).collect();
    assert_eq!(vals, vec![1, 2, 99]);
    assert!(block.tail.is_none());
}
