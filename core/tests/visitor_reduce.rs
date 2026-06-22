//! Reduce/append: a mut visitor edits `Vec` / `Option` AST positions by overriding the *parent*
//! node's `visit_*_mut` (it owns the `&mut Vec` / `&mut Option`), then descending.

use core::marker::PhantomData;
use syan::visit::Ast;

#[derive(Debug, Ast)]
pub struct Stmt<S>(pub i64, pub PhantomData<S>);

#[derive(Debug, Ast)]
#[subast(crate::Stmt)]
pub struct Block<S> {
    pub stmts: Vec<Stmt<S>>,
    pub tail: Option<Stmt<S>>,
}

pub mod visit {
    syan::visit::visitor!(super::Block, super::Stmt);
}

struct Editor;
impl<S> visit::VisitMut<S> for Editor {
    fn visit_block_mut(&mut self, b: &mut Block<S>) {
        b.stmts.retain(|s| s.0 != 0); // reduce: drop zero statements
        b.tail = None; // remove the optional node entirely
        b.stmts.push(Stmt(99, PhantomData)); // append a synthesized statement
        visit::visit_block_mut(self, b); // descend into the (edited) children
    }
}

#[test]
fn seq_mut_reduce_and_append() {
    let mut block: Block<()> = Block {
        stmts: vec![
            Stmt(0, PhantomData),
            Stmt(1, PhantomData),
            Stmt(0, PhantomData),
            Stmt(2, PhantomData),
        ],
        tail: Some(Stmt(7, PhantomData)),
    };
    block.visit_mut(&mut Editor);

    let vals: Vec<i64> = block.stmts.iter().map(|s| s.0).collect();
    assert_eq!(vals, vec![1, 2, 99], "zeros dropped, 99 appended");
    assert!(block.tail.is_none(), "optional node removed");
}
