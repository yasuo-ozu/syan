//! Macro audit — *silent-wrong* findings, demonstrated as runtime tests.
//!
//! Each test below COMPILES CLEANLY (no error, no warning) yet produces a wrong result — the most
//! insidious class of macro bug. Compile-error / panic findings live in `macro_audit_test.rs`
//! (+ `ui/audit_*.rs`).
//!
//! Two styles live here, deliberately:
//!
//! * The `symbol!` tests PIN the current (buggy) behavior with a `BUG:` comment stating the correct
//!   expectation, so the day a fix lands the assertion flips and points here. They pass today.
//! * The `visitor_map_value` tests assert the CORRECT expectation and therefore **FAIL**. They are
//!   executable bug reports: `cargo test` is red until the bug is fixed, at which point they simply
//!   go green and need no edit. The cost is a red suite; the benefit is that running the tests
//!   demonstrates the defect instead of merely recording it.
//!
//! The two symbol! encoding bugs and the visitor map-value skip all remain open.
#![allow(dead_code)]

// ── BUG: symbol! re-encodes non-decimal / underscored int literals to canonical decimal ──────────
// The `LitInt` branch builds the symbol slot from `litint.base10_digits()`, discarding the base
// prefix and digit separators. A `Symbol!` is a type-level *name*, so the written spelling should be
// preserved (or non-decimal literals rejected).
#[test]
fn symbol_reencodes_int_literals() {
    use syan::symbol::Symbol;
    // BUG: each of these should preserve the written spelling (or be rejected); instead the literal
    // is silently normalized to decimal.
    assert_eq!(
        <Symbol![0xff]>::default().to_string(),
        "255",
        "0xff should stay \"0xff\""
    );
    assert_eq!(
        <Symbol![0b101]>::default().to_string(),
        "5",
        "0b101 should stay \"0b101\""
    );
    assert_eq!(
        <Symbol![0o17]>::default().to_string(),
        "15",
        "0o17 should stay \"0o17\""
    );
    assert_eq!(
        <Symbol![1_000]>::default().to_string(),
        "1000",
        "1_000 should stay \"1_000\""
    );
}

// ── BUG: symbol! leaks a raw identifier's `r#` prefix into the symbol string ─────────────────────
// The `Ident` branch uses `ident.to_string()`, which for a raw ident yields "r#type" (the `#` is
// encoded via `chars::Pound`). A raw ident is exactly how one names a symbol after a keyword, so the
// common `Symbol![r#type]` case is mis-encoded; the `r#` should be stripped.
#[test]
fn symbol_leaks_raw_ident_prefix() {
    use syan::symbol::Symbol;
    // BUG: should be "type"; the `r#` prefix leaks through.
    assert_eq!(
        <Symbol![r#type]>::default().to_string(),
        "r#type",
        "raw-ident prefix should be stripped"
    );
}

// ── BUG: visitor!() silently skips a node held in a map's VALUE slot ─────────────────────────────
// `peel` (macro/util.rs) classifies a field by walking wrapper levels *without matching any
// container by name* — an elegant design that makes user wrappers work with no registration. It
// finds the head through `first_ty_arg(seg)?`: the FIRST type argument. That is correct for every
// container with a `SeqView`/`OptView` impl — `Vec<T>`, `VecDeque<T>`, `Option<T>`, `Box<T>`,
// `Attempt<T>` and `Punctuated<T, P>` — because in all six the element is parameter 0. A map is the
// first container where it is not.
//
// So for `HashMap<String, Node>`: `HashMap` is not a head, `first_ty_arg` yields `String`, `String`
// is not a head and has no type arguments, `?` short-circuits, and the whole field is classified a
// LEAF. The generated `visit_holder` body is literally `{}`.
//
// Nothing can warn about it as written: "this field is a leaf" is the same answer `i64` and `String`
// give, so "leaf because it holds no nodes" and "leaf because I looked in the wrong slot" are
// indistinguishable at the point of the decision.
//
// This generalises past maps: `Result<Node, E>` is traversed and `Result<E, Node>` is not.
mod visitor_map_value {
    use syan::visit::Ast;

    #[derive(Debug, Ast)]
    #[subast()]
    pub enum Node {
        Leaf(i64),
    }

    #[derive(Debug, Ast)]
    #[subast(crate::visitor_map_value::Node)]
    pub struct HashHolder {
        pub field: std::collections::HashMap<String, Node>,
    }

    #[derive(Debug, Ast)]
    #[subast(crate::visitor_map_value::Node)]
    pub struct BTreeHolder {
        pub field: std::collections::BTreeMap<i64, Node>,
    }

    // The control: `Vec` is the same arity-1 shape and is traversed, so the difference below is the
    // slot, not "syan cannot see into containers".
    #[derive(Debug, Ast)]
    #[subast(crate::visitor_map_value::Node)]
    pub struct VecHolder {
        pub field: Vec<Node>,
    }

    mod v {
        syan::visit::visitor!(
            crate::visitor_map_value::Node,
            crate::visitor_map_value::HashHolder,
            crate::visitor_map_value::BTreeHolder,
            crate::visitor_map_value::VecHolder
        );
    }

    /// FAILS until `peel` looks past the first type argument. Reaches 0 of 2.
    #[test]
    fn a_hashmap_values_nodes_are_visited() {
        let mut m = std::collections::HashMap::new();
        m.insert("a".to_string(), Node::Leaf(1));
        m.insert("b".to_string(), Node::Leaf(2));
        let h = HashHolder { field: m };
        let mut n = 0usize;
        h.visit(|_x: &Node| n += 1);
        assert_eq!(
            n, 2,
            "both map VALUES are `Node`s and should be visited; `peel` inspected the `String` KEY \
             (first type argument) instead, classified the field a leaf, and generated an empty \
             `visit_hash_holder` body"
        );
    }

    /// FAILS for the same reason as the `HashMap` case; the key being `i64` rather than `String`
    /// changes nothing, since neither is a head. Reaches 0 of 1.
    #[test]
    fn a_btreemap_values_nodes_are_visited() {
        let mut m = std::collections::BTreeMap::new();
        m.insert(1i64, Node::Leaf(1));
        let h = BTreeHolder { field: m };
        let mut n = 0usize;
        h.visit(|_x: &Node| n += 1);
        assert_eq!(
            n, 1,
            "the map VALUE is a `Node` and should be visited; `peel` inspected the `i64` KEY"
        );
    }

    #[test]
    fn a_vec_of_the_same_node_is_traversed() {
        let h = VecHolder {
            field: vec![Node::Leaf(1), Node::Leaf(2)],
        };
        let mut n = 0usize;
        h.visit(|_x: &Node| n += 1);
        // Not a bug — the contrast that isolates the slot as the cause.
        assert_eq!(n, 2);
    }
}
