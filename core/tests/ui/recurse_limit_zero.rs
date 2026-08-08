// AUDIT 1: `#[recurse(limit = 0)]` panics the proc-macro.
// `recurse()` computes the depth chain with `for _ in 0..(recursion_depth - 1)`.
// With `limit = 0`, `recursion_depth - 1` underflows `usize` and the macro panics
// ("attempt to subtract with overflow") instead of reporting a clean error or
// treating 0 as "terminator only". A panicking attribute macro is a poor failure
// mode (no span, opaque message). `limit = 1` is the smallest sound value.

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
