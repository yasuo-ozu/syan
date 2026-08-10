// Parses `proc_macro2` tokens throughout; the whole suite is skipped without the optional
// dependency that provides them.
#![cfg(feature = "proc_macro2")]

//! Prefix-dedup must not split a `#[group]` run.
//!
//! Found while building the §1 regression test, not in `known-gaps-rustyfi-port.md`.
//!
//! When ≥2 variants share a leading run of identical fields, `extract_parse_inner` parses that
//! prefix ONCE and gives each variant only its suffix. `generate_substruct` absorbs the contiguous
//! run of `#[group(..)]`-carrying fields that FOLLOW a holder — but it can only see the fields in
//! the slice it is handed. So when the holder was shared (hence in the prefix) and its content was
//! per-variant (hence in the suffix), the holder was parsed as an ordinary field up front and the
//! content aborted with `Cannot find member g in struct E` — on a declaration that is perfectly
//! correct.
//!
//! The fix walks the prefix/suffix split point back out of any group run, so at worst the enum falls
//! back to the per-variant scheme.

use syan::nested::group::GroupParen;
use syan::parse::{Parse, Unparse};
use syan::source::proc_macro2::literal::Integer;
use syan::symbol::Token;
use template_quote::quote;

/// Holder shared (prefix), content per-variant (suffix) — the split that used to abort.
#[derive(Parse, Unparse)]
pub enum Split<S> {
    Int {
        g: GroupParen<(), S>,
        #[group(self.g)]
        int: Integer,
    },
    Bang {
        g: GroupParen<(), S>,
        #[group(self.g)]
        bang: Token![S => !],
    },
}

#[test]
fn a_shared_holder_with_per_variant_content_parses() {
    let a: Split<_> = Parse::parse(quote! { (7) }).unwrap();
    assert!(matches!(a, Split::Int { .. }));
    let b: Split<_> = Parse::parse(quote! { (!) }).unwrap();
    assert!(matches!(b, Split::Bang { .. }));
}

#[test]
fn both_arms_round_trip() {
    for src in [quote! { (7) }, quote! { (!) }] {
        let e: Split<_> = Parse::parse(src.clone()).unwrap();
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

/// The other side of the fix: when the WHOLE group run is shared it stays in the prefix and
/// prefix-dedup still applies. Only the split case is walked back.
#[derive(Parse, Unparse)]
pub enum WholeRunShared<S> {
    Plus {
        g: GroupParen<(), S>,
        #[group(self.g)]
        v: Integer,
        tail: Token![S => +],
    },
    Minus {
        g: GroupParen<(), S>,
        #[group(self.g)]
        v: Integer,
        tail: Token![S => -],
    },
}

#[test]
fn a_fully_shared_group_run_still_dedups() {
    let a: WholeRunShared<_> = Parse::parse(quote! { (7) + }).unwrap();
    assert!(matches!(a, WholeRunShared::Plus { .. }));
    let b: WholeRunShared<_> = Parse::parse(quote! { (7) - }).unwrap();
    assert!(matches!(b, WholeRunShared::Minus { .. }));

    let src = quote! { (7) - };
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    b.unparse(&mut (&mut out)).unwrap();
    assert_eq!(
        out.into_iter()
            .collect::<proc_macro2::TokenStream>()
            .to_string(),
        src.to_string(),
    );
}
