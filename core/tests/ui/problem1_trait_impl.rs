// Problem 1: `Expr<S>` self-references by value (no Box), so #[recurse]'s by-value-cycle guard aborts
// before any transform; rustc's E0072/type-param/derive errors then cascade on the untransformed module.

use syan::parse::recurse;
use syan::source::proc_macro2::literal::Integer;

#[recurse]
mod m {
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;

    #[derive(Parse, Unparse)]
    pub enum Expr<S> {
        Lit(Integer),
        Nested(Expr<S>),
    }

    // The trait impl passes through verbatim onto the natural type.
    impl<S> std::fmt::Display for Expr<S> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "expr")
        }
    }
}

fn main() {}
