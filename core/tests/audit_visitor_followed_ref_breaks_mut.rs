//! AUDIT E (visit_mut, acyclic `visitor!()`) — fixed → BULK regression test.
//!
//! A *followed* shared-reference field (`&'a T`, `T` a followed type) used to break the auto-generated
//! mutable side, failing the whole visitor even for shared-only use: `util::peel` saw through
//! `Type::Reference` transparently, so the mut side emitted `&mut **r` through a `&` → E0596 "cannot
//! borrow `**r` as mutable, as it is behind a `&` reference". (A reference field whose head is NOT
//! followed — e.g. `&str` — is a leaf and compiled fine; owned `Box<T>` works on both sides.)
//!
//! Fixed: `peel` now flags a shared-ref head (`Peeled::shared_ref`) and the mut side treats it as a
//! leaf (no `&mut head` through a `&`), while the shared side still visits it. This compiles. (`Holder`
//! is listed first so the union params are lifetime-first, isolating this from the param-ordering fix.)
#![allow(dead_code)]

use syan::visit::Ast;

#[derive(Ast)]
#[subast()]
pub struct Leaf<S> {
    pub _p: core::marker::PhantomData<S>,
}

#[derive(Ast)]
#[subast(crate::Leaf)]
pub struct Holder<'a, S> {
    pub r: &'a Leaf<S>,
}

mod v {
    syan::visit::visitor!(crate::Holder, crate::Leaf);
}

#[test]
fn followed_shared_ref_field_is_visitable() {
    // Reaching this body means the auto-generated `visit_mut` handled the followed `&Leaf` field.
}
