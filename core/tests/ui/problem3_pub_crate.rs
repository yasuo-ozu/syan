// Problem 3: only `pub` types participate in cycle detection.
// A `pub(crate)` self-recursive type is invisible to #[recurse]; Rust then
// rejects it as a recursive type of infinite size.

use syan::parse::recurse;

#[recurse]
mod m {
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;

    #[derive(Parse, Unparse)]
    pub(crate) enum Expr<S> {
        Lit(Integer),
        Nested(Expr<S>),
    }
}

fn main() {}
