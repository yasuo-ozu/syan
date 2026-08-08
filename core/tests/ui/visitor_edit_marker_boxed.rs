//! `#[seq]` on a `Box`-wrapped container (`Box<Vec<Leaf>>`) — edit views require a *bare* single
//! container (`Vec<Head>` / `Option<Head>`); a wrapped or nested container isn't an edit target (it
//! still descends). Clean abort pointing at the field type, not a cryptic `SeqView` trait error.
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
    pub items: Box<Vec<Leaf<S>>>,
}

pub mod vis {
    syan::visit::visitor!(crate::Wrap, crate::Leaf);
}

fn main() {
    let _ = PhantomData::<()>;
}
