// Problem 1: trait impl for a cycle type is not transformed by #[recurse].
// The macro renames `Expr` to `__ExprRec`; the trait impl still says `Expr<S>`,
// which no longer exists as a type definition — only as a public alias.
// However, `impl Trait for TypeAlias<S>` is not allowed in Rust, so this would
// fail even if the alias existed.  The compile error exposes the gap.

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

    // Trait impl targets `Expr<S>` — after #[recurse] this name is gone.
    impl<S> std::fmt::Display for Expr<S> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "expr")
        }
    }
}

fn main() {}
