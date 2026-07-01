//! `#[seq]`/`#[opt]` names the field's INNERMOST container. Marking a `Vec<Option<T>>` field `#[seq]`
//! (its innermost container is the `Option`) is a clear build error pointing at the mismatch, rather
//! than a cryptic `Vec<Option<T>>: SeqView<T>` trait error downstream.

use core::marker::PhantomData;
use syan::visit::Ast;

#[derive(Ast)]
pub struct Item<S>(pub PhantomData<S>);

#[derive(Ast)]
#[subast(crate::Item)]
pub struct Holder<S> {
    #[seq] // WRONG: innermost container is the `Option` — should be `#[opt]`
    pub grid: Vec<Option<Item<S>>>,
}

pub mod vis {
    syan::visit::visitor!(crate::Holder, crate::Item);
}

fn main() {
    let _ = PhantomData::<()>;
}
