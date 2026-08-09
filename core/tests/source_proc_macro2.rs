//! `source::proc_macro2::Stream` as a *stream*, specifically the state that is not the cursor.
//!
//! The token stream carries one piece of derived state beyond its position: `is_joint`, the spacing
//! of the last atom served, which is what `skip_sep` reports. A rollback that restores the position
//! but not `is_joint` leaves the stream lying about separation — silently, and only after a failed
//! speculative parse, which is the hardest kind of bug to notice.

use proc_macro2::TokenStream;
use syan::parse::ParseStream;
use syan::source::proc_macro2::Stream;

fn stream_of(src: &str) -> Stream {
    Stream::new(src.parse::<TokenStream>().unwrap())
}

/// `a::b` lexes as `Ident, Punct(':' Joint), Punct(':' Alone), Ident`.
#[test]
fn rollback_restores_joint_spacing() {
    let mut s = stream_of("a::b");
    s.next(); // Ident `a` — Alone, so a separator is allowed after it
    assert!(s.skip_sep(), "after an ident, `skip_sep` reports separable");

    let r: Result<(), &str> = s.dup(|d| {
        d.next(); // the JOINT ':' — flips is_joint
        assert!(!d.skip_sep(), "after a joint punct, not separable");
        Err("speculative parse fails here")
    });
    assert!(r.is_err());

    assert!(
        s.skip_sep(),
        "a failed dup must restore `is_joint` along with the position — \
         leaving it set makes the stream claim `a` is joint to what follows"
    );
}

#[test]
fn commit_keeps_joint_spacing() {
    let mut s = stream_of("a::b");
    s.next(); // `a`
    let r: Result<(), &str> = s.dup(|d| {
        d.next(); // joint ':'
        Ok(())
    });
    assert!(r.is_ok());
    assert!(
        !s.skip_sep(),
        "a committed dup must keep the spacing its consumption established"
    );
}

#[test]
fn nested_rollback_restores_the_enclosing_scopes_spacing() {
    let mut s = stream_of("a::b");
    let r: Result<(), &str> = s.dup(|outer| {
        outer.next(); // `a` — Alone
        let _: Result<(), &str> = outer.dup(|inner| {
            inner.next(); // joint ':'
            Err("inner fails")
        });
        assert!(
            outer.skip_sep(),
            "the inner rollback must restore the OUTER scope's spacing"
        );
        Ok(())
    });
    assert!(r.is_ok());
    assert!(s.skip_sep());
}

/// Position and spacing must not drift apart: after a rollback the stream must serve the same tokens
/// it would have served had the transaction never run.
#[test]
fn failed_dup_is_a_no_op_on_tokens_and_spacing() {
    let mut s = stream_of("a::b");
    let baseline: Vec<String> = {
        let mut fresh = stream_of("a::b");
        std::iter::from_fn(|| fresh.next())
            .map(|t| t.to_string())
            .collect()
    };

    let _: Result<(), &str> = s.dup(|d| {
        while d.next().is_some() {}
        Err("ate everything")
    });

    let after: Vec<String> = std::iter::from_fn(|| s.next())
        .map(|t| t.to_string())
        .collect();
    assert_eq!(after, baseline);
}
