//! Two different types whose paths end in the same ident, each declared by a different visitor in
//! one inheritance chain. Neither the per-`visitor!` check nor the `#[subast]` ones see them
//! together, so this is where the chain-wide check has to catch it.
use core::marker::PhantomData;
use syan::visit::Ast;
pub mod m1 { #[derive(syan::visit::Ast, Debug)] pub enum Foo<S> { A(core::marker::PhantomData<S>) } }
pub mod m2 { #[derive(syan::visit::Ast, Debug)] pub enum Foo<S> { B(core::marker::PhantomData<S>) } }
#[derive(Ast, Debug)]
pub struct Root<S> { pub p: PhantomData<S> }
pub mod v1 { syan::visit::visitor!(crate::m1::Foo); }
pub mod v2 { syan::visit::visitor!(crate::v1 => crate::m2::Foo); }
pub mod v3 { syan::visit::visitor!(crate::v2 => crate::Root); }

fn main() {}
