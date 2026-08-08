// AUDIT 6 (multi-root soundness guard): a multi-root cycle is supported when its self-referential
// roots form a feedback vertex set — i.e. every cycle passes through a root, where the depth
// decrements. Here `A` and `B` self-reference (roots), but `C` and `D` form a sub-cycle (`C -> D ->
// C`) that touches neither, so the depth would never decrement along it and the generated types
// would not terminate. The macro rejects this with a clear message rather than emitting an
// infinitely-recursive type. (The supported multi-root shape lives in recurse_multiroot.rs.)

use syan::parse::recurse;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum A<S> {
        Me(Box<A<S>>),
        ToC(Box<C<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast()]
    pub enum B<S> {
        Me(Box<B<S>>),
        ToC(Box<C<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast()]
    pub enum C<S> {
        ToD(Box<D<S>>),
        ToA(Box<A<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast()]
    pub enum D<S> {
        ToC(Box<C<S>>),
        ToB(Box<B<S>>),
        Lit(PhantomData<S>),
    }
}

fn main() {}
