//! `#[seq]` on a field whose element type is not a visited type — the marker would route nowhere and
//! silently no-op. Clean error instead of silent drop (previously surfaced only as a later E0407).
use core::marker::PhantomData;
use syan::visit::Ast;

#[derive(Ast)]
pub enum Leaf<S> {
    A(PhantomData<S>),
}

#[derive(Ast)]
#[subast(crate::Leaf)]
pub struct Wrap<S> {
    #[seq]
    pub items: Vec<String>, // `String` is not a visited AST type
    pub leaf: Leaf<S>,
}

pub mod vis {
    syan::visit::visitor!(crate::Wrap, crate::Leaf);
}

fn main() {
    let _ = PhantomData::<()>;
}
