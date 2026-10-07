//! Two sibling bases each contributing a type whose path ends in the same ident. Neither base has
//! ever seen the other, so only the visitor extending both can notice.
use core::marker::PhantomData;
use syan::visit::Ast;

pub mod m1 {
    #[derive(syan::visit::Ast)]
    pub enum Foo<S> {
        A(core::marker::PhantomData<S>),
    }
}
pub mod m2 {
    #[derive(syan::visit::Ast)]
    pub enum Foo<S> {
        B(core::marker::PhantomData<S>),
    }
}

#[derive(Ast)]
pub struct T<S> {
    pub p: PhantomData<S>,
}

pub mod v1 {
    syan::visit::visitor!(crate::m1::Foo);
}
pub mod v2 {
    syan::visit::visitor!(crate::m2::Foo);
}
pub mod w {
    syan::visit::visitor!(crate::v1, crate::v2 => crate::T);
}

fn main() {}
