// AUDIT 5 (now a clean error): a lifetime parameter on a cycle type is rejected
// with a clear message.
//
// `#[recurse]` threads recursion depth through *type* parameters only; lifetimes
// (and const generics) are not threaded and would be dropped from the regenerated
// aliases (formerly a confusing E0106 "missing lifetime specifier"). The macro now
// detects a lifetime/const param on a cycle type and aborts, naming the type.

use syan::parse::recurse;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<'a, S> {
        Nested(Box<Expr<'a, S>>),
        Lit(PhantomData<(&'a (), S)>),
    }
}

fn main() {}
