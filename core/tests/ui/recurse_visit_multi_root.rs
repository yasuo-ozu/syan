// AUDIT 6 (now a clean error): `#[recurse(visit)]` on a multi-root cycle is
// rejected with a clear diagnostic instead of silently emitting no visitor.
//
// When more than one cycle type is self-referential, every back-edge collapses to
// one ambiguous `__Rec`, so a single depth-generic visitor cannot be generated.
// Previously the `visit` flag was silently ignored (the user then hit a confusing
// "cannot find trait `Visit`"); now the macro aborts at expansion explaining why.

use syan::parse::recurse;

#[recurse(visit)]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    // Both A and B self-reference → two effective roots → rejected.
    #[derive(Ast)]
    #[subast()]
    pub enum A<S> {
        SelfRef(Box<A<S>>),
        Cross(Box<B<S>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast()]
    pub enum B<S> {
        SelfRef(Box<B<S>>),
        Cross(Box<A<S>>),
        Lit(PhantomData<S>),
    }
}

fn main() {}
