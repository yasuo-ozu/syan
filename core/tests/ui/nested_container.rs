//! Nested containers in a single field (`Vec<Option<T>>`) are unsupported; the visitor rejects them
//! with a clear message rather than emitting code that fails to type-check.

use core::marker::PhantomData;
use syan::visit::Ast;

#[derive(Ast)]
pub enum Leaf<S> {
    U(PhantomData<S>),
}

#[derive(Ast)]
#[subast(crate::Leaf)]
pub struct Bad<S> {
    pub xs: Vec<Option<Leaf<S>>>,
}

pub mod vis {
    syan::visit::visitor!(crate::Bad, crate::Leaf);
}

fn main() {}
