// AUDIT 7 (complex / non-identity generic argument on a back-edge to the root): a back-edge to the
// recursion root collapses to the single depth parameter `__Rec`, so it must repeat the root's own
// parameters verbatim. A back-edge that wraps a param (`Expr<Vec<S>>`) would make the recursion
// *non-regular* (the param grows at every level), which the single-`__Rec` depth machinery cannot
// express — previously the argument was silently dropped (miscompile). It is now rejected with a
// clear message. (An *identity* back-edge `Expr<S>` is of course fine — see recurse_generics.rs.)

use syan::parse::recurse;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        Nest(Box<Expr<S>>),      // identity back-edge — fine
        Wrap(Box<Expr<Vec<S>>>), // non-identity back-edge — REJECTED
        Lit(PhantomData<S>),
    }
}

fn main() {}
