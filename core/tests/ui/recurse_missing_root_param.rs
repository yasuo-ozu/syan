// AUDIT 4 (heterogeneous generics — the rejection direction): a cycle type may carry params BEYOND
// the root's, but it must declare ALL of the root's params.
//
// `#[recurse]` spells each cycle type's `__Rec` default as the root's depth chain `__RootDefault<root
// params>`, so the root's params must be in scope in every cycle type. Here the root `Expr<S, T>`
// (alphabetically first of the mutually-recursive pair) has a param `T` that `Stmt<S>` lacks, so the
// default is unspellable — rejected with a clear message naming the missing parameter. (The reverse,
// a non-root with an *extra* param, IS supported — see recurse_generics.rs.)

use syan::parse::recurse;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S, T> {
        Nest(Box<Stmt<S>>),
        Lit(PhantomData<(S, T)>),
    }

    #[derive(Ast)]
    #[subast()]
    pub enum Stmt<S> {
        Back(Box<Expr<S, u8>>),
        Nop(PhantomData<S>),
    }
}

fn main() {}
