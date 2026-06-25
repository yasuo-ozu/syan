//! AUDIT finding: a tuple NESTED INSIDE a container (`Vec<(Leaf, Leaf)>`, `Option<(Leaf, Leaf)>`, …)
//! was silently treated as a leaf — its tuple elements were never visited (no diagnostic, just a low
//! visit count). `peel` had no `Type::Tuple` arm, so a container element that is a tuple peeled to
//! `None` ⇒ the whole field was a leaf. Tuple destructuring only ran on the top-level field type.
#![allow(dead_code)]

use core::marker::PhantomData;
use syan::visit::Ast;

#[derive(Ast)]
#[subast()]
pub struct Leaf<S> {
    pub _p: PhantomData<S>,
}

#[derive(Ast)]
#[subast(crate::Leaf)]
pub struct Holder<S> {
    pub top: (Leaf<S>, Leaf<S>),                  // control: top-level tuple (already worked)
    pub vec_of_tuples: Vec<(Leaf<S>, Leaf<S>)>,   // bug: Vec of tuples
    pub opt_of_tuple: Option<(Leaf<S>, Leaf<S>)>, // bug: Option of tuple
    pub boxed_tuple: Box<(Leaf<S>, Leaf<S>)>,     // bug: Box of tuple
}

mod v {
    syan::visit::visitor!(crate::Leaf, crate::Holder);
}

fn leaf<S>() -> Leaf<S> {
    Leaf { _p: PhantomData }
}

#[test]
fn container_of_tuple_visits_elements() {
    let h: Holder<()> = Holder {
        top: (leaf(), leaf()),                                   // 2
        vec_of_tuples: vec![(leaf(), leaf()), (leaf(), leaf())], // 4
        opt_of_tuple: Some((leaf(), leaf())),                    // 2
        boxed_tuple: Box::new((leaf(), leaf())),                 // 2
    };
    let mut n = 0usize;
    h.visit(|_: &Leaf<()>| n += 1);
    assert_eq!(n, 10, "a container-of-tuple must visit its tuple elements");
}

#[test]
fn container_of_tuple_visits_elements_mut() {
    let mut h: Holder<()> = Holder {
        top: (leaf(), leaf()),
        vec_of_tuples: vec![(leaf(), leaf())],
        opt_of_tuple: Some((leaf(), leaf())),
        boxed_tuple: Box::new((leaf(), leaf())),
    };
    let mut n = 0usize;
    h.visit_mut(|_: &mut Leaf<()>| n += 1);
    assert_eq!(n, 8, "the &mut side must also reach tuple elements inside containers");
}
