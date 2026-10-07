//! Extending a visitor that is in heterogeneous (method-generic) mode.
//!
//! A base whose non-shared parameter carries a bound keys its trait on the shared subset and makes
//! the rest per-method generics, which also makes its methods `where Self: Sized`. An extender has
//! to match that: its forwarding impls must satisfy the base's as a supertrait, and a `?Sized`
//! visitor cannot.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::visit::Ast;

pub trait Bound {}
impl Bound for () {}

#[derive(Ast)]
pub struct Bounded<S>
where
    S: crate::Bound,
{
    pub p: PhantomData<S>,
}

#[derive(Ast)]
pub struct Plain {
    pub n: u8,
}

#[derive(Ast)]
#[subast(crate::Bounded, crate::Plain)]
pub struct Top<S>
where
    S: crate::Bound,
{
    pub a: Bounded<S>,
    pub b: Plain,
}

// `S` is unshared (only `Bounded` has it) and bounded, so this goes method-mode.
pub mod base {
    syan::visit::visitor!(crate::Bounded, crate::Plain);
}
pub mod ext {
    syan::visit::visitor!(crate::base => crate::Top);
}

fn sample() -> Top<()> {
    Top {
        a: Bounded { p: PhantomData },
        b: Plain { n: 7 },
    }
}

#[derive(Default)]
struct Counter {
    bounded: u32,
    plain: u32,
    top: u32,
}

impl base::Visit for Counter {
    fn visit_bounded<S: crate::Bound>(&mut self, i: &Bounded<S>) {
        self.bounded += 1;
        base::visit_bounded(self, i);
    }
    fn visit_plain(&mut self, i: &Plain) {
        self.plain += 1;
        base::visit_plain(self, i);
    }
}
impl base::VisitMut for Counter {}
impl<S: crate::Bound> ext::Visit<S> for Counter {
    fn visit_top(&mut self, i: &Top<S>) {
        self.top += 1;
        ext::visit_top(self, i);
    }
}

#[test]
fn an_extended_method_mode_base_walks() {
    let mut c = Counter::default();
    ext::Visit::visit_top(&mut c, &sample());
    assert_eq!((c.bounded, c.plain, c.top), (1, 1, 1));
}

#[test]
fn a_method_mode_visitor_can_be_passed_by_reference() {
    let mut c = Counter::default();
    base::Visit::visit_plain(&mut &mut c, &Plain { n: 1 });
    assert_eq!(c.plain, 1);
}

#[test]
fn a_method_mode_visitor_can_be_boxed_and_tupled() {
    let mut b: Box<Counter> = Box::default();
    base::Visit::visit_plain(&mut *b, &Plain { n: 1 });
    assert_eq!(b.plain, 1);

    let mut pair = (Counter::default(), Counter::default());
    base::Visit::visit_plain(&mut pair, &Plain { n: 1 });
    assert_eq!((pair.0.plain, pair.1.plain), (1, 1));
}

// The extender itself is not in method mode, so its own closure machinery is intact.
#[test]
fn a_closure_still_works_on_the_extender() {
    let mut tops = 0;
    sample().visit(|_t: &Top<()>| tops += 1);
    assert_eq!(tops, 1);
}
