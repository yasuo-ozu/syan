//! `#[seq]` on a fixed-size array — arrays traverse as sequences but have no `SeqView` impl (they can't
//! be structurally edited). This is a clean error, not a cryptic `[Leaf; N]: SeqView<Leaf>` trait error.
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
    pub items: [Leaf<S>; 3],
}

pub mod vis {
    syan::visit::visitor!(crate::Wrap, crate::Leaf);
}

fn main() {
    let _ = PhantomData::<()>;
}
