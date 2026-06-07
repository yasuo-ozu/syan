// Problem 5: when BOTH types in a mutual cycle self-reference, both are added
// to `effective_roots`.  `transform_type` then replaces every occurrence of
// EITHER type with `__Rec`, leaving the span parameter `S` completely unused
// inside the transformed definitions → E0392.

use syan::parse::recurse;

#[recurse]
mod m {
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;

    // Tree self-references via Branch and Forest self-references via Multi.
    // Both land in `effective_roots`; all occurrences of Tree/Forest become
    // `__Rec`, so `S` appears in no field → "unused type parameter" error.

    #[derive(Parse, Unparse)]
    pub enum Tree<S> {
        Leaf(Integer),
        Branch(Tree<S>, Tree<S>),   // both become __Rec
        Wrapped(Forest<S>),          // also becomes __Rec
    }

    #[derive(Parse, Unparse)]
    pub enum Forest<S> {
        Empty,
        Single(Tree<S>),   // becomes __Rec
        Multi(Forest<S>),  // self-ref, becomes __Rec
    }
}

fn main() {}
