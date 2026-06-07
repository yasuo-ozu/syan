// Problem 2: free functions in a #[recurse] module are not transformed.
// A recursive function that calls itself with the inner `__Rec`-typed field
// fails because the field type is `__Rec` (unbounded) but the function
// parameter expects the concrete public alias `Expr<S>`.

use syan::parse::recurse;

#[recurse]
mod m {
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;

    #[derive(Parse, Unparse)]
    pub enum Expr<S> {
        Lit(Integer),
        Nested(Expr<S>),
    }

    // This function is NOT given a `__Rec` type parameter by #[recurse].
    // Inside `Nested(inner)`, `inner` has type `__Rec`, which cannot be
    // passed to `count_nodes` expecting `&Expr<S>`.
    pub fn count_nodes<S>(e: &Expr<S>) -> usize {
        match e {
            Expr::Lit(_) => 1,
            Expr::Nested(inner) => 1 + count_nodes(inner),
        }
    }
}

fn main() {}
