//! Regression: `#[group(self.f)]` substruct helper-name collision between two enum VARIANTS that carry
//! a same-named, same-shaped grouped field. The generated helper's name used to hash only the field
//! name + the owning type + the derive nonce — none of which vary across variants — so two variants
//! each grouping a field named `val` under a container named `grp` emitted the same
//! `__SyanSubstructOf_grp_..._<nonce>` struct twice (E0428). Fixed in
//! `macro/attribute/substruct.rs::generate_substruct` by mixing the enclosing variant ident (and the
//! field's position) into the name.
#![allow(dead_code)]

use syan::nested::group::GroupParen;
use syan::parse::{Parse, Unparse};
use syan::source::proc_macro2::literal::Integer;
use syan::symbol::Token;
use template_quote::quote;
use type_macro_derive_tricks::macro_derive;

type Sp = syan::source::proc_macro2::Span;

/// `Plus`/`Minus` each group a field named `val` under a container named `grp` — identical shape,
/// different variant. The leading `marker` field differs (`+` vs `-`), so the enum `Parse` derive can't
/// prefix-dedup the two variants away into one shared parse (which would sidestep the collision).
#[macro_derive(Parse, Unparse, Debug)]
pub enum Node<S> {
    Plus {
        marker: Token![S => +],
        grp: GroupParen<(), S>,
        #[group(self.grp)]
        val: Integer,
    },
    Minus {
        marker: Token![S => -],
        grp: GroupParen<(), S>,
        #[group(self.grp)]
        val: Integer,
    },
}

fn round_trip(node: &Node<Sp>) -> String {
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    node.unparse(&mut (&mut out)).unwrap();
    out.into_iter().collect::<proc_macro2::TokenStream>().to_string()
}

#[test]
fn variant_group_substructs_dont_collide() {
    let plus: Node<_> = Parse::parse(quote!(+ ( 1 ))).unwrap();
    match &plus {
        Node::Plus { val, .. } => assert_eq!(val.value, "1"),
        Node::Minus { .. } => panic!("expected Plus"),
    }
    let s = round_trip(&plus);
    assert!(s.contains('+') && s.contains('1'), "{s}");

    let minus: Node<_> = Parse::parse(quote!(- ( 2 ))).unwrap();
    match &minus {
        Node::Minus { val, .. } => assert_eq!(val.value, "2"),
        Node::Plus { .. } => panic!("expected Minus"),
    }
    let s = round_trip(&minus);
    assert!(s.contains('-') && s.contains('2'), "{s}");
}
