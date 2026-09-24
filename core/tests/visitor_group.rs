//! A `Group<T, O, C>` **type** field is walked through, not around.
//!
//! `GroupParen`/`GroupBrace`/`GroupBracket` are what `nested::group` steers a grammar towards —
//! "reach for the aliases rather than naming `O` and `C` by hand" — so a grammar that takes that
//! advice has to be visitable. It was not: the group peeled to a wrapper level like any other, and
//! nothing implemented the descent for it, so the whole node failed to compile.
//!
//! A group is descent-only. It holds exactly one `T`, so `SeqView`'s `push`/`remove` and `OptView`'s
//! `take` have nothing to mean, and `#[seq]`/`#[opt]` on such a field stays an error — pinned by
//! `tests/ui/visitor_edit_marker_boxed.rs`'s sibling case in `visitor_diagnostics`. The delimiters
//! are punctuation tokens and are never visited.
#![allow(dead_code)]

use syan::nested::group::{GroupBracket, GroupParen};
use syan::source::string::Span;
use syan::span::WithSpan;
use syan::symbol::chars;

mod ast {
    use super::*;
    use syan::visit::Ast;

    #[derive(Debug, Ast)]
    pub struct Qubit(pub i64);

    #[derive(Debug, Ast)]
    #[subast(crate::ast::Qubit)]
    pub struct Stmt {
        /// The shape the aliases produce: a group wrapping a container.
        pub listed: GroupBracket<Vec<Qubit>, Span>,
        /// Nested groups, to be sure the wrapper composes rather than being special-cased once.
        pub nested: GroupParen<GroupBracket<Option<Qubit>, Span>, Span>,
    }

    pub mod v {
        syan::visit::visitor!(crate::ast::Qubit, crate::ast::Stmt);
    }
}

use ast::{Qubit, Stmt};

fn ob() -> WithSpan<chars::OpenBracket, Span> {
    Default::default()
}
fn cb() -> WithSpan<chars::CloseBracket, Span> {
    Default::default()
}
fn op() -> WithSpan<chars::OpenParen, Span> {
    Default::default()
}
fn cp() -> WithSpan<chars::CloseParen, Span> {
    Default::default()
}

fn sample() -> Stmt {
    Stmt {
        listed: GroupBracket {
            open: ob(),
            slot: vec![Qubit(7), Qubit(1), Qubit(2)],
            close: cb(),
        },
        nested: GroupParen {
            open: op(),
            slot: GroupBracket {
                open: ob(),
                slot: Some(Qubit(9)),
                close: cb(),
            },
            close: cp(),
        },
    }
}

fn seen(s: &Stmt) -> Vec<i64> {
    let mut v = Vec::new();
    s.visit(|q: &Qubit| v.push(q.0));
    v.sort_unstable();
    v
}

#[test]
fn a_group_type_field_descends() {
    assert_eq!(seen(&sample()), vec![1, 2, 7, 9]);
}

#[test]
fn and_on_the_mut_side() {
    let mut s = sample();
    s.visit_mut(|q: &mut Qubit| q.0 *= 10);
    assert_eq!(seen(&s), vec![10, 20, 70, 90]);
}

/// An empty group is a group: the walk must not assume the slot is populated.
#[test]
fn an_empty_group_visits_nothing() {
    let s = Stmt {
        listed: GroupBracket {
            open: ob(),
            slot: vec![],
            close: cb(),
        },
        nested: GroupParen {
            open: op(),
            slot: GroupBracket {
                open: ob(),
                slot: None,
                close: cb(),
            },
            close: cp(),
        },
    };
    assert_eq!(seen(&s), Vec::<i64>::new());
}
