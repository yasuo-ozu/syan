//! Container-edit views (`visit_<t>_seq` / `visit_<t>_opt`) are generated ONLY for a field explicitly
//! marked `#[seq]` / `#[opt]` — there is no auto-detection from the container type. Here `Leaf` is held
//! in a `Vec` and an `Option`, but neither field is marked, so no view method exists (only the in-place
//! `visit_leaf_mut`). Overriding `visit_leaf_seq` / `visit_leaf_opt` is a "not a member of trait" error.

use core::marker::PhantomData;
use syan::visit::{Ast, OptView, SeqView};

#[derive(Ast)]
pub enum Leaf<S> {
    A(PhantomData<S>),
}

#[derive(Ast)]
#[subast(crate::Leaf)]
pub struct Wrap<S> {
    pub items: Vec<Leaf<S>>,   // not `#[seq]` -> no `visit_leaf_seq`
    pub last: Option<Leaf<S>>, // not `#[opt]` -> no `visit_leaf_opt`
}

pub mod vis {
    syan::visit::visitor!(crate::Wrap, crate::Leaf);
}

struct V;
impl<S> vis::VisitMut<S> for V {
    fn visit_leaf_seq<W: SeqView<Leaf<S>>>(&mut self, _v: &mut W) {}
    fn visit_leaf_opt<W: OptView<Leaf<S>>>(&mut self, _v: &mut W) {}
}

fn main() {
    let _ = PhantomData::<()>;
}
