// The only argument `#[recurse]` accepts is `structural` (choosing decycle's unroll engine over the
// default ranked one). The former `limit = N` was removed — depth is not a compile-time parameter any
// more. Anything else is a clean compile error naming what IS allowed, not a proc-macro panic.

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
