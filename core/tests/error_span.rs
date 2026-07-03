use syan::error::{Error, ParseError};
use syan::source::string::Span;

fn at(loc: usize) -> Span {
    Span {
        line: 1,
        col: loc + 1,
        loc,
    }
}

#[test]
fn span_survives_new_and_span_of() {
    let e = ParseError::new(at(7), "boom");
    let recovered = e.span_of::<Span>().expect("span was stored");
    assert_eq!(recovered.loc, 7);
    assert_eq!(e.to_string(), "boom");
}

#[test]
fn span_of_wrong_type_is_none() {
    // Stored as `string::Span`; asking for the unit span recovers nothing.
    let e = ParseError::new(at(3), "boom");
    assert!(e.span_of::<()>().is_none());
    assert!(e.span_of::<Span>().is_some());
}

#[test]
fn add_sub_error_merges_to_furthest() {
    // Positional `migrate` is pick-larger-loc, so incremental aggregation is furthest-progress.
    let mut parent = ParseError::new(at(0), "cannot parse");
    for loc in [3usize, 11, 6] {
        parent.add_sub_error(ParseError::new(at(loc), "branch"));
    }
    assert_eq!(parent.span_of::<Span>().unwrap().loc, 11);
}

#[test]
fn from_cause_folds_to_furthest_progress() {
    // The `Error::from_cause` aggregate span is the furthest child (loc 11), not the union.
    let causes = [3usize, 11, 6]
        .into_iter()
        .map(|loc| ParseError::new(at(loc), "branch"))
        .collect::<Vec<_>>();
    let aggregate = <ParseError as Error>::from_cause(causes);
    assert_eq!(aggregate.span_of::<Span>().unwrap().loc, 11);
    assert_eq!(aggregate.to_string(), "cannot parse");
}

#[test]
fn from_cause_empty_carries_no_span() {
    let aggregate = <ParseError as Error>::from_cause(Vec::new());
    assert!(aggregate.span_of::<Span>().is_none());
}

#[test]
fn clone_and_debug_survive_erasure() {
    let mut e = ParseError::new(at(4), "root");
    e.add_sub_error(ParseError::new(at(9), "child"));
    let cloned = e.clone();
    assert_eq!(cloned.span_of::<Span>().unwrap().loc, 9);
    // Debug reaches through the erased span without panicking.
    let _ = format!("{e:?}");
}

#[cfg(feature = "proc_macro2")]
#[test]
fn heterogeneous_span_types_recover_independently() {
    use syan::source::proc_macro2::Span as Pm2Span;

    // Every built-in span, including the `!Send + !Sync` pm2 span, stores and recovers under the
    // `'static`-only erasure.
    let string_err = ParseError::new(at(5), "string span");
    assert_eq!(string_err.span_of::<Span>().unwrap().loc, 5);
    assert!(string_err.span_of::<Pm2Span>().is_none());

    let pm2_err = ParseError::new(Pm2Span::default(), "pm2 span");
    assert!(pm2_err.span_of::<Pm2Span>().is_some());
    assert!(pm2_err.span_of::<Span>().is_none());
}
