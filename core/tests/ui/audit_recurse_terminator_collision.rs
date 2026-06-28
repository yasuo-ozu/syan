// AUDIT (hygiene, now ENGINE-SCOPED): the generated terminator struct `ExprTerm`
// (format!("{root}Term"), Span::call_site()) is emitted by `#[recurse]` into the user's module when the
// cycle needs the depth-limited engine (i.e. derives `Parse`). A user item named `ExprTerm` in the same
// module then collides -> E0428. (An Ast-only cycle needs no engine, so no `ExprTerm` is emitted and a
// user `ExprTerm` is fine — see `recurse_no_engine.rs`.) `XxxTerm` (no leading underscore, derived from
// the root name) is the plausible accidental clash. Fix: unguessable generated names, or abort! on clash.
use syan::parse::{recurse, Parse, Unparse};

#[recurse]
mod m {
    use core::marker::PhantomData;
    use syan::parse::{Parse, Unparse};

    #[derive(Parse, Unparse)]
    pub enum Expr<S> {
        Nest(Box<Expr<S>>),
        Lit(PhantomData<S>),
    }

    pub struct ExprTerm; // collides with the generated terminator `pub struct ExprTerm<S>(…);`
}

fn main() {}
