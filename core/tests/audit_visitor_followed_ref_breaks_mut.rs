//! AUDIT (visit_mut, acyclic `visitor!()`) — BULK compile test, RED until fixed.
//!
//! A *followed* shared-reference field (`&'a T`, `T` a followed type) breaks the auto-generated mutable
//! side, so the whole visitor fails to compile — even for shared-only use. `util::peel` sees through
//! `Type::Reference` transparently and records nothing about the borrow; `visitor!()` always emits BOTH
//! sides, and the mut side then emits `&mut **r` / `.iter_mut()` through a `&` → E0596 "cannot borrow
//! `**r` as mutable, as it is behind a `&` reference". (A reference field whose head is NOT followed —
//! e.g. `&str` — is a leaf and compiles fine; owned `Box<T>` works on both sides.)
//!
//! This file FAILS TO BUILD today — that E0596 *is* the audit finding. It builds once the mut side
//! treats a followed `&T`/`&[T]` as a shared-only leaf (`Holder` is listed first so the union params
//! are lifetime-first, isolating this from the separate union-param ordering issue).
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
