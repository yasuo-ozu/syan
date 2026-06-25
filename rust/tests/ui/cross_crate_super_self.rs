// AUDIT / residual hole (3-crate, cross-crate inheritance): a downstream visitor extending an
// upstream intermediate that recorded its ancestor via a `super::`/`self::`-RELATIVE path fails — the
// relative ancestor can't be requalified into the downstream crate.
//
// Chain: `syan` (macro) → `syan-rust` (defines `base` + `mid_ss`, where `mid_ss` is
// `visitor!(super::base => ItemSs)`) → THIS fixture crate (`visitor!(syan_rust::inherit::mid_ss => Down)`).
// `mid_ss` recorded its `base` ancestor as `super::base` (relative to its own module upstream); the
// `__syan_visited` macro replays that verbatim downstream, where `super::base` resolves against THIS
// crate's module tree (it becomes `super::syan_rust::inherit::base`) and is unresolvable → E0432/E0277.
//
// Fundamental: a proc-macro has no module-path awareness, so it can't rewrite a `super`/`self`-relative
// ancestor to an absolute path the way it requalifies a leading `crate::` (→ the base's host crate).
// Fix / workaround: the upstream intermediate must use a `crate::`-rooted entry path (as `mid` does —
// see `cross_crate_inherit_multilevel.rs`, which works). This file documents the limit.

use core::marker::PhantomData;
use syan::visit::Ast;
use syan_rust::inherit::ItemSs;

#[derive(Debug, Ast)]
#[subast(syan_rust::inherit::ItemSs)]
pub enum Down<S> {
    It(Box<ItemSs<S>>),
    Nil(PhantomData<S>),
}

mod nv {
    syan::visit::visitor!(syan_rust::inherit::mid_ss => crate::Down);
}

fn main() {}
