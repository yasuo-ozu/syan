//! Name hygiene for `#[recurse]`.
//!
//! (a) An Ast-only cycle (no `Parse`/`Unparse`/`Spanned`) needs no depth-limited engine, so `#[recurse]`
//!     emits only the natural recursive types — a `where`-clause stays on the natural type and no
//!     terminator is generated.
//! (b) The generated internal names (engine `__XxxRec`, terminator `XxxTerm`, depth default
//!     `__XxxDefault`, conversion traits `__ToNat`/`__FromNat`) carry a per-expansion nonce, so a user
//!     type whose name would clash with one of them (e.g. `ExprTerm`) does NOT collide — even for an
//!     engine-needing (`Parse`-deriving) cycle. (Was `ui/audit_recurse_terminator_collision.rs`.)
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::parse::{recurse, Parse};
use syan::visit::Ast;
use template_quote::quote;

// (a) engine-free cycle: where-clause + user `ExprTerm` both fine.
#[recurse]
mod ast_only {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S>
    where
        S: Clone,
    {
        Nest(Box<Expr<S>>),
        Lit(PhantomData<S>),
    }

    pub struct ExprTerm;
}

// (b) engine-needing cycle: a user `ExprTerm` would have collided with the old (unstamped) terminator;
// the nonce keeps them apart.
#[recurse]
mod engine {
    use core::marker::PhantomData;
    use syan::parse::{Parse, Unparse};

    // `Lit` is tried first: `Nest(Box<Expr>)` consumes no leading token, so a `Nest`-first grammar is
    // left-recursive — with the now-unbounded `Parse` re-entry that would recurse forever (a standard
    // recursive-descent limitation the old depth cap silently masked by truncating).
    #[derive(Parse, Unparse)]
    pub enum Expr<S> {
        Lit(::syan::source::proc_macro2::literal::Integer, PhantomData<S>),
        Nest(Box<Expr<S>>),
    }

    // Same name as the generated terminator's *stem* — no clash now that it's `__ExprTerm_<nonce>`.
    pub struct ExprTerm;
}

fn assert_ast<T: Ast>() {}

#[test]
fn ast_only_cycle_with_where_clause_and_exprterm_compiles() {
    assert_ast::<ast_only::Expr<()>>();
    let _e: ast_only::Expr<()> = ast_only::Expr::Lit(PhantomData);
    let _t = ast_only::ExprTerm;
}

#[test]
fn engine_cycle_user_exprterm_does_not_collide() {
    let _t = engine::ExprTerm; // the user's own type, distinct from the nonce-stamped terminator
    // The engine cycle still parses (delegated through the nonce-stamped engine). `5` parses to a valid
    // `Expr` (the `Nest`-first grammar wraps it; the point is it compiles and parses, not the variant).
    let _e: engine::Expr<()> = Parse::parse(quote! { 5 }).unwrap();
}
