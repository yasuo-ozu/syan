// AUDIT (diagnostic): a `visitor!()` over two acyclic types where one carries a `where`-BOUNDED
// generic param the other lacks. The visitor trait is keyed on the param union, and the bound is
// applied to items quantified over that union (the inherent `.visit()`, `IntoVisitor`, …) — so the
// param-less `Plain` is quantified over `S` with an undischargeable `S: Bound`, previously surfacing
// as an opaque E0277/E0283 cascade at the `visitor!()` site. (An *unbounded* unshared param is fine —
// `visitor_generics.rs` exercises that; only a bounded one breaks.) The guard says so.

use core::marker::PhantomData;
use syan::visit::Ast;

pub trait Bound {}

#[derive(Ast)]
#[subast()]
pub struct Bounded<S>
where
    S: Bound,
{
    pub _p: PhantomData<S>,
}

#[derive(Ast)]
#[subast()]
pub struct Plain {
    pub _x: u8,
}

mod v {
    syan::visit::visitor!(crate::Bounded, crate::Plain);
}

fn main() {}
