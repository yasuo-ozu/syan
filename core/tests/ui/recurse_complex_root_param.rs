// AUDIT 7 (complex / non-identity generic argument on a back-edge to the root, ENGINE-SCOPED): a
// back-edge to the recursion root collapses to the depth parameter `__Rec` in the depth-limited engine,
// so it must repeat the root's own parameters verbatim. A back-edge that wraps a param (`Expr<Vec<S>>`)
// is *non-regular* recursion (the param grows at every level), which the engine cannot express, so it
// is rejected when the cycle needs the engine (derives `Parse`). (An *identity* back-edge `Expr<S>` is
// fine — see recurse_generics.rs. An Ast-only cycle needs no engine and a non-regular natural type is
// valid Rust, so it is not rejected there.)

use syan::parse::{recurse, Parse, Unparse};

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::parse::{Parse, Unparse};

    #[derive(Parse, Unparse)]
    pub enum Expr<S> {
        Nest(Box<Expr<S>>),      // identity back-edge — fine
        Wrap(Box<Expr<Vec<S>>>), // non-identity back-edge — REJECTED
        Lit(PhantomData<S>),
    }
}

fn main() {}
