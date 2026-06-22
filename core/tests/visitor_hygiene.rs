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
