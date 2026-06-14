//! Stage 9: overriding `visit_*_seq_mut` / `visit_*_opt_mut` lets a visitor reduce (remove) or
//! append AST nodes in `Vec` / `Option` positions, because it receives `&mut Vec` / `&mut Option`.

use core::marker::PhantomData;
use syan::visit::{visitor, Ast};

#[derive(Debug, Ast)]
pub struct Stmt<S>(pub i64, pub PhantomData<S>);

#[derive(Debug, Ast)]
pub struct Block<S> {
    pub stmts: Vec<Stmt<S>>,
    pub tail: Option<Stmt<S>>,
}

#[visitor(Block, Stmt)]
pub mod visit {}

use visit::VisitableMut;

struct Editor;
impl<S> visit::VisitMut<S> for Editor {
    fn visit_stmt_seq_mut(&mut self, seq: &mut Vec<Stmt<S>>) {
        seq.retain(|s| s.0 != 0); // reduce: drop zero statements
        for s in seq.iter_mut() {
            self.visit_stmt_mut(s); // still descend into survivors
        }
        seq.push(Stmt(99, PhantomData)); // append a synthesized statement
    }
    fn visit_stmt_opt_mut(&mut self, opt: &mut Option<Stmt<S>>) {
        *opt = None; // remove the optional node entirely
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
