//! Generated helper params are mixed-site-hygienic, so a visited type may declare generic params
//! literally named `__V`/`__T`/`__H`/`__F`/`__A`/`__B` without colliding with the generated
//! `Visit`/`Driver`/`Hook`/`Chain`/tuple machinery.
#![allow(non_camel_case_types)]

use core::marker::PhantomData;
use syan::visit::Ast;

#[derive(Debug, Ast)]
pub enum Node<__V, __T, __H, __F, __A, __B> {
    Rec(Box<Node<__V, __T, __H, __F, __A, __B>>),
    Leaf(PhantomData<(__V, __T, __H, __F, __A, __B)>),
}

pub mod visit {
    syan::visit::visitor!(crate::Node);
}

type N = Node<(), (), (), (), (), ()>;

fn sample() -> N {
    Node::Rec(Box::new(Node::Leaf(PhantomData)))
}

#[test]
fn closure_visitor_with_helper_named_params() {
    let mut n = 0usize;
    sample().visit(|_x: &N| n += 1);
    assert_eq!(n, 2, "outer Rec + inner Leaf");
}

// The indexed tuple-closure helper family (`__F0`/`__T0`/…) is protected by `fresh_prefix`.
#[derive(Debug, Ast)]
pub enum Tup<__F0, __T0> {
    Rec(Box<Tup<__F0, __T0>>),
    Leaf(PhantomData<(__F0, __T0)>),
}

pub mod vtup {
    syan::visit::visitor!(crate::Tup);
}

type T = Tup<(), ()>;

#[test]
fn indexed_helper_named_params_via_tuple_of_closures() {
    let ast: T = Tup::Rec(Box::new(Tup::Leaf(PhantomData)));
    let mut a = 0usize;
    let mut b = 0usize;
    // The tuple-of-closures path instantiates the `__F0`/`__T0` tuple impls, which must not collide
    // with the visited type's own `__F0`/`__T0` params.
    ast.visit((|_x: &T| a += 1, |_x: &T| b += 1));
    assert_eq!((a, b), (2, 2));
}

// Value bindings (the generated receiver `this` / scrutinee `i`) are span-isolated from user idents,
// so a visited type may have followed fields literally named `this` and `i`.
#[derive(Debug, Ast)]
pub enum Leaf<S> {
    U(PhantomData<S>),
}

#[derive(Debug, Ast)]
#[subast(crate::Leaf)]
pub struct Names<S> {
    pub this: Box<Leaf<S>>,
    pub i: Leaf<S>,
}

pub mod vnames {
    syan::visit::visitor!(crate::Names, crate::Leaf);
}

#[test]
fn fields_named_this_and_i_are_traversed() {
    let n: Names<()> = Names {
        this: Box::new(Leaf::U(PhantomData)),
        i: Leaf::U(PhantomData),
    };
    let mut leaves = 0usize;
    n.visit(|_l: &Leaf<()>| leaves += 1);
    assert_eq!(leaves, 2, "both the `this` and `i` fields were visited");
}

#[test]
fn tuple_and_struct_visitors_with_helper_named_params() {
    // Tuple-of-closures path (Chain + tuple impls) and the &mut-struct path both compile and run.
    struct Counter(usize);
    impl<__V, __T, __H, __F, __A, __B> visit::Visit<__V, __T, __H, __F, __A, __B> for Counter {
        fn visit_node(&mut self, i: &Node<__V, __T, __H, __F, __A, __B>) {
            self.0 += 1;
            visit::visit_node(self, i);
        }
    }
    let mut c = Counter(0);
    sample().visit(&mut c);
    assert_eq!(c.0, 2);

    let mut a = 0usize;
    let mut b = 0usize;
    sample().visit((|_x: &N| a += 1, |_x: &N| b += 1));
    assert_eq!((a, b), (2, 2));
}
