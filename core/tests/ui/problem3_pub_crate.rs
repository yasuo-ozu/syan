// A `pub(crate)` self-recursive type. Visibility no longer affects cycle detection: the cycle is read
// out of the derive's own where-bounds (`analysis::analyze_items`), and the derive emits those for any
// visibility. So this IS a cycle now — but a by-value one, so it is still rejected, by rustc's own
// `E0072` (decycle's `check_by_value_type_cycle` was removed upstream: rustc's message names every type
// in the cycle and suggests the indirection, which no pre-check improved on).
//
// (Before, syan walked field types and only followed `pub` items, which made this shape invisible to
// `#[recurse]` and left rustc to reject it alone.)

use syan::parse::recurse;

#[recurse]
mod m {
    use syan::parse::{Parse, Unparse};
    use syan::literal::Integer;

    #[derive(Parse, Unparse)]
    pub(crate) enum Expr<S> {
        Lit(Integer),
        Nested(Expr<S>),
    }
}

fn main() {}
