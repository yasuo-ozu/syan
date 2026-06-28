//! An Ast-only `#[recurse]` cycle (no `Parse`/`Unparse`/`Spanned`) needs no depth-limited engine, so
//! `#[recurse]` emits only the natural recursive types. Two limitations that the engine causes are
//! therefore absent for such cycles (they remain for engine-needing cycles — see
//! `ui/audit_recurse_where_clause.rs` / `ui/audit_recurse_terminator_collision.rs`):
//!   - a `where`-clause on a cycle type is fine (it stays on the natural type; no engine to mis-thread);
//!   - a user type named `ExprTerm` does not collide (no terminator is generated).
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::recurse;
use syan::visit::Ast;

#[recurse]
mod m {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    // where-clause on a cycle type — no engine, so it just stays on the natural `Expr<S>`.
    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S>
    where
        S: Clone,
    {
        Nest(Box<Expr<S>>),
        Lit(PhantomData<S>),
    }

    // A user type whose name would collide with a generated terminator — but no terminator is
    // generated for an engine-free cycle, so this is fine.
    pub struct ExprTerm;
}

fn assert_ast<T: Ast>() {}

#[test]
fn ast_only_cycle_with_where_clause_and_exprterm_compiles() {
    assert_ast::<m::Expr<()>>();
    let _e: m::Expr<()> = m::Expr::Lit(PhantomData);
    let _t = m::ExprTerm; // the user's own type, not shadowed by a generated terminator
}
