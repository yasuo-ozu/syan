// Problem 6: where clauses on cycle-type definitions are not updated.
// The clause is copied verbatim into `__ExprRec<S, __Rec>`; any reference to
// `Expr<S>` inside it resolves to the PUBLIC ALIAS (fixed depth), not to
// `__ExprRec<S, __Rec>` itself.  The bound is therefore independent of __Rec,
// meaning the struct can only be used when the fixed-depth alias satisfies the
// clause — ignoring the actual depth being instantiated.

use syan::parse::recurse;

trait Marker {}

#[recurse]
mod m {
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;
    use super::Marker;

    // The where clause references `Expr<S>` (the cycle type).
    // After #[recurse] it ends up on `__ExprRec<S, __Rec>` unchanged,
    // so the bound is `Expr<S>: Marker` where `Expr<S>` = the concrete alias,
    // not `__ExprRec<S, __Rec>`.  The requirement is therefore always on the
    // depth-4 alias, even when using depth-1 or depth-3 instantiations.
    #[derive(Parse, Unparse)]
    pub enum Expr<S> where Expr<S>: Marker {
        Lit(Integer),
        Nested(Expr<S>),
    }
}

fn main() {}
