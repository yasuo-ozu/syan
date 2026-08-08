// AUDIT (diagnostic): `visitor!()` over a MULTI-ROOT `#[recurse]` cycle that lists only ONE root and
// omits a co-root must abort cleanly. A multi-root cycle gives each root its own depth dimension
// (`__ARec<S, __R0, __R1>`), and a `VisitRec` impl is emitted only for a *listed* root's node — so an
// omitted co-root `B` leaves `A`'s `__R1: VisitRec` bound unsatisfiable, previously surfacing as a
// cryptic `VisitRec is not implemented for __BRec<…>` wall. A root defines a depth dimension and
// cannot be drilled, so every root must be listed; the guard says so.

use syan::parse::recurse;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast(crate::ast::B)]
    pub enum A<S> {
        SelfA(Box<A<S>>), // back-edge to root A
        ToB(Box<B<S>>),   // edge to root B
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast(crate::ast::A)]
    pub enum B<S> {
        ToA(Box<A<S>>),   // edge to root A
        SelfB(Box<B<S>>), // back-edge to root B
        Lit(PhantomData<S>),
    }
}

mod v {
    syan::visit::visitor!(crate::ast::A); // omits co-root B
}

fn main() {}
