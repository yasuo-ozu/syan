// Parses `proc_macro2` tokens throughout; the whole suite is skipped without the optional
// dependency that provides them.
#![cfg(feature = "proc_macro2")]

//! Substruct names must be unique per **variant** — §1 of `known-gaps-rustyfi-port.md`.
//!
//! `generate_substruct` named its output `__SyanSubstructOf{shape}_{field}_{ident}_{nonce}`, where
//! `ident` is the *enum's* name and `nonce` is one value per derive invocation. So the name was a
//! function of `(shape, group-field name, enum name)` only, and two variants whose `#[group]`
//! holders happened to share a field name were indistinguishable.
//!
//! The port hit this three times — `TypeAtom::{Record, RecordOpen}` both naming their holder `rec`,
//! and `TypeApp::{Inline,Block,Math}CmdTy` all naming theirs `list`. Before the fix this file emits
//! E0428 (name defined twice), E0119 (conflicting impls), and then `struct … does not have a field
//! named 'right'` on a line that is perfectly correct — the survivor carrying the other variant's
//! fields, which is a badly misleading first symptom.
//!
//! The variants below carry a distinguishing FIRST field on purpose: with a shared leading field the
//! enum takes the prefix-dedup path, which has an unrelated defect of its own (a `#[group]` holder in
//! the shared prefix loses the suffix fields that name it — `Cannot find member g in struct E`). That
//! is not what this file is about.

use syan::literal::Integer;
use syan::nested::group::GroupParen;
use syan::parse::{Parse, Unparse};
use syan::symbol::Token;
use template_quote::quote;

#[derive(Parse, Unparse)]
pub enum E<S> {
    Left {
        tag: Token![S => +],
        g: GroupParen<(), S>,
        #[group(self.g)]
        left: Integer,
    },
    Right {
        tag: Token![S => -],
        g: GroupParen<(), S>,
        #[group(self.g)]
        right: Integer,
    },
}

#[test]
fn two_variants_may_share_a_group_field_name() {
    let l: E<_> = Parse::parse(quote! { + (1) }).unwrap();
    assert!(matches!(l, E::Left { .. }), "the `+` arm must win");
    let r: E<_> = Parse::parse(quote! { - (2) }).unwrap();
    assert!(matches!(r, E::Right { .. }), "the `-` arm must win");
}

/// The collision did not merely fail to compile — the survivor drove the *other* variant's code.
/// Round-tripping both arms is what pins that each variant kept its own substruct.
#[test]
fn each_variant_round_trips_through_its_own_substruct() {
    for src in [quote! { + (1) }, quote! { - (2) }] {
        let e: E<_> = Parse::parse(src.clone()).unwrap();
        let mut out = Vec::<proc_macro2::TokenTree>::new();
        e.unparse(&mut (&mut out)).unwrap();
        assert_eq!(
            out.into_iter()
                .collect::<proc_macro2::TokenStream>()
                .to_string(),
            src.to_string(),
        );
    }
}

/// A struct's substructs are named without any variant component, so a struct and an enum in the
/// same module still cannot collide with each other.
#[derive(Parse, Unparse)]
pub struct S<Sp> {
    pub g: GroupParen<(), Sp>,
    #[group(self.g)]
    pub only: Integer,
}

#[test]
fn a_struct_still_works() {
    let s: S<_> = Parse::parse(quote! { (7) }).unwrap();
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    s.unparse(&mut (&mut out)).unwrap();
    assert_eq!(
        out.into_iter()
            .collect::<proc_macro2::TokenStream>()
            .to_string(),
        "(7)",
    );
}
