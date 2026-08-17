//! The benchmark is only meaningful if all five backends parse the same language into the same
//! tree. This is the gate: same `eval()`, same node count, same accept/reject decision.

use syan_bench::{ast::Expr, chumsky_impl, combine_impl, input, nom_impl, syan_char, syan_token};

fn all(src: &str) -> Vec<(&'static str, Result<Expr, String>)> {
    vec![
        ("nom", nom_impl::parse(src)),
        ("chumsky", chumsky_impl::parse(src)),
        ("combine", combine_impl::parse(src)),
        ("syan-char/ranked", syan_char::ranked::parse(src)),
        ("syan-token/ranked", syan_token::ranked::lex_then_parse(src)),
        ("syan-char/structural", syan_char::structural::parse(src)),
        (
            "syan-token/structural",
            syan_token::structural::lex_then_parse(src),
        ),
    ]
}

fn assert_agree(src: &str) {
    let rs = all(src);
    let ok: Vec<&str> = rs
        .iter()
        .filter(|(_, r)| r.is_ok())
        .map(|(n, _)| *n)
        .collect();
    let bad: Vec<&str> = rs
        .iter()
        .filter(|(_, r)| r.is_err())
        .map(|(n, _)| *n)
        .collect();
    assert!(
        ok.is_empty() || bad.is_empty(),
        "{src:?}: disagreement on acceptance — ok={ok:?} err={bad:?}\n{}",
        rs.iter()
            .map(|(n, r)| format!("  {n}: {r:?}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    if bad.is_empty() {
        let evals: Vec<(&str, i64)> = rs
            .iter()
            .map(|(n, r)| (*n, r.as_ref().unwrap().eval()))
            .collect();
        let first = evals[0].1;
        assert!(
            evals.iter().all(|(_, v)| *v == first),
            "{src:?}: eval disagreement {evals:?}"
        );
        let nodes: Vec<(&str, usize)> = rs
            .iter()
            .map(|(n, r)| (*n, r.as_ref().unwrap().nodes()))
            .collect();
        assert!(
            nodes.iter().all(|(_, v)| *v == nodes[0].1),
            "{src:?}: node-count disagreement {nodes:?} (associativity or precedence differs)"
        );
    }
}

#[test]
fn handwritten_cases() {
    for src in [
        "1",
        "1 + 2",
        "1 + 2 * 3",
        "1 * 2 + 3",
        "( 1 + 2 ) * 3",
        "1 - 2 - 3", // left-assoc: -4, not 2
        "8 / 4 / 2", // left-assoc: 1, not 4
        "( ( ( 7 ) ) )",
        "12 + 345 * 6",
        "1+2*3", // no spaces
    ] {
        assert_agree(src);
    }
}

#[test]
fn generated_cases() {
    for n in [1usize, 2, 5, 17, 64] {
        assert_agree(&input::flat(n));
    }
    for d in [0usize, 1, 3, 8] {
        assert_agree(&input::nested(d));
    }
    for d in [0usize, 1, 2, 5] {
        assert_agree(&input::tree(d));
    }
}

#[test]
fn precedence_and_associativity_are_the_same() {
    // pinned values, so a backend cannot be "consistent" by all being wrong together
    assert_eq!(nom_impl::parse("1 - 2 - 3").unwrap().eval(), -4);
    assert_eq!(chumsky_impl::parse("1 - 2 - 3").unwrap().eval(), -4);
    assert_eq!(combine_impl::parse("1 - 2 - 3").unwrap().eval(), -4);
    assert_eq!(syan_char::ranked::parse("1 - 2 - 3").unwrap().eval(), -4);
    assert_eq!(
        syan_token::ranked::lex_then_parse("1 - 2 - 3")
            .unwrap()
            .eval(),
        -4
    );
    assert_eq!(nom_impl::parse("1 + 2 * 3").unwrap().eval(), 7);
    assert_eq!(chumsky_impl::parse("1 + 2 * 3").unwrap().eval(), 7);
    assert_eq!(combine_impl::parse("1 + 2 * 3").unwrap().eval(), 7);
    assert_eq!(syan_char::ranked::parse("1 + 2 * 3").unwrap().eval(), 7);
    assert_eq!(
        syan_token::ranked::lex_then_parse("1 + 2 * 3")
            .unwrap()
            .eval(),
        7
    );
}

#[test]
fn all_reject_bad_input() {
    for src in ["", "1 +", "( 1", "1 )", "1 + &", "&"] {
        let rs = all(src);
        let ok: Vec<&str> = rs
            .iter()
            .filter(|(_, r)| r.is_ok())
            .map(|(n, _)| *n)
            .collect();
        assert!(ok.is_empty(), "{src:?} was ACCEPTED by {ok:?}");
    }
}
