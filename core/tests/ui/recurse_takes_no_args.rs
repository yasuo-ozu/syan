// `#[recurse]` no longer takes any arguments — the former `limit = N` was removed (`Unparse`/`Spanned`
// are now unbounded for group-free cycles, and the `Parse` engine uses a fixed internal depth). Passing
// any argument is a clean compile error, not a proc-macro panic.

use syan::parse::recurse;

#[recurse(limit = 0)]
mod m {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        Nested(Box<Expr<S>>),
        Lit(PhantomData<S>),
    }
}

fn main() {}
