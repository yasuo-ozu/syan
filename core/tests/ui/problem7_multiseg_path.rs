// Problem 7: multi-segment paths are not recognized as cycle references.
// `collect_refs` checks only `path.segments.first()` for the outer type name,
// so `inner::Expr<S>` is not identified as a reference to `Expr`.
// The cycle is not detected; neither type is transformed; Rust rejects the
// self-referential `Expr` as an infinitely-sized type.

use syan::parse::recurse;

#[recurse]
mod outer {
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;

    pub mod inner {
        pub use super::Expr;
    }

    // `inner::Expr<S>` is not recognized as a reference to `Expr` by the
    // cycle detector — it only checks the leading segment (`inner`, not `Expr`).
    // Result: no cycle detected, no transformation, infinite-size error.
    #[derive(Parse, Unparse)]
    pub enum Expr<S> {
        Lit(Integer),
        Nested(inner::Expr<S>),
    }
}

fn main() {}
