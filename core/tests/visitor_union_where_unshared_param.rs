//! A `visitor!()` over two acyclic types where one (`Bounded<S> where S: Bound`) carries a
//! `where`-bounded generic param the other (`Plain`) lacks. The visitor keys its trait on the **shared**
//! params and makes the unshared bounded param a per-method generic (`visit_bounded<S>(… ) where S:
//! Bound`), so the param-less `Plain` is never quantified over `S`. This goes **struct-only** (a closure
//! can't be `for<S>` generic), like the heterogeneous concrete-fill case. (An *unbounded* unshared param
//! instead stays in the union + closure path — `visitor_generics.rs`.)
#![allow(dead_code)]

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
    // The generated `where S: Bound` lands in this module, so the user trait must be in scope here
    // (a where-bound naming a user trait by bare path needs importing — true for shared bounds too).
    use crate::Bound;
    syan::visit::visitor!(crate::Bounded, crate::Plain);
}

struct MyType;
impl Bound for MyType {}

#[derive(Default)]
struct Counter {
    bounded: usize,
    plain: usize,
}

// The trait is keyed on the shared params (none here); `visit_bounded` carries `S` as a method generic.
impl v::Visit for Counter {
    fn visit_bounded<S: Bound>(&mut self, i: &Bounded<S>) {
        self.bounded += 1;
        v::visit_bounded(self, i);
    }
    fn visit_plain(&mut self, i: &Plain) {
        self.plain += 1;
        v::visit_plain(self, i);
    }
}

#[test]
fn visits_both_via_struct_visitor() {
    let b: Bounded<MyType> = Bounded { _p: PhantomData };
    let p = Plain { _x: 7 };
    let mut c = Counter::default();
    b.visit(&mut c);
    p.visit(&mut c);
    assert_eq!(c.bounded, 1);
    assert_eq!(c.plain, 1, "the param-less `Plain` is visited without choosing an `S`");
}
