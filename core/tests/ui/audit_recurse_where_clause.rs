// AUDIT (compile error, now ENGINE-SCOPED): a where-clause on a `#[recurse]` cycle type that needs the
// depth-limited engine (i.e. derives `Parse`) is carried onto the internal `__ExprRec` and the
// `__ToNat`/delegated-`Parse` conversion, but is NOT threaded onto those generated items' own bounds,
// so `where S: Clone` is required by the conversion's `-> Expr<S>` yet undischarged -> E0277. (An
// Ast-only cycle needs no engine and handles a where-clause fine — see `recurse_no_engine.rs`.)
// Fix: thread the where-clause through the engine/conversion, or abort! clearly.
use syan::parse::{recurse, Parse, Unparse};

#[recurse]
mod m {
    use core::marker::PhantomData;
    use syan::parse::{Parse, Unparse};

    #[derive(Parse, Unparse)]
    pub enum Expr<S>
    where
        S: Clone,
    {
        Nest(Box<Expr<S>>),
        Lit(PhantomData<S>),
    }
}

fn main() {}
