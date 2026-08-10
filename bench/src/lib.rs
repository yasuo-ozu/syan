//! One arithmetic-expression grammar, four implementations, measured like for like.
//!
//! See `README.md` for the fairness rules and the measured results.

// A derived parse tree has fields that exist to *drive parsing* — the operator tokens, the group
// delimiters — and are never read afterwards, because `lower_*` only needs the discriminant. That
// is the normal shape of a syan grammar, not an oversight.
#![allow(dead_code)]
pub mod alloc;
pub mod ast;
pub mod chumsky_impl;
pub mod input;
pub mod nom_impl;
pub mod syan_char;
pub mod syan_token;
