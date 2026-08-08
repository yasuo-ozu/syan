// Problem 5: the cycle (Forest, Tree) self-references by value (no Box), so #[recurse]'s by-value-cycle
// guard aborts before any transform; rustc's E0072/type-param/derive errors then cascade on the untransformed module.

use syan::parse::recurse;

#[recurse]
mod m {
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;

    // Tree self-references via Branch and Forest self-references via Multi; both are by-value edges.

    #[derive(Parse, Unparse)]
    pub enum Tree<S> {
        Leaf(Integer),
        Branch(Tree<S>, Tree<S>),
        Wrapped(Forest<S>),
    }

    #[derive(Parse, Unparse)]
    pub enum Forest<S> {
        Empty,
        Single(Tree<S>),
        Multi(Forest<S>),
    }
}

fn main() {}
