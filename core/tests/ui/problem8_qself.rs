// Problem 8: qself paths in method signatures are not transformed.
// In an inherent impl block, #[recurse] rewrites `fn foo(self) -> Expr<S>` to
// `fn foo(self) -> __Rec`.  But a return type written as `<Expr<S> as Id>::Output`
// has a qself component; `transform_type` only handles `Type::Path { qself: None }`,
// so the qself stays as `<Expr<S> as Id>::Output` — which resolves to the concrete
// alias type, not to `__Rec`.  When `self: __ExprRec<S, __Rec>` is returned into
// a slot of type `__ExprRec<S, __ExprDefault<S>>`, the types do not match.

use syan::parse::recurse;

// Blanket `Id` trait so `<Expr<S> as Id>::Output` resolves to `Expr<S>`.
trait Id { type Output; }
impl<T> Id for T { type Output = T; }

#[recurse]
mod m {
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;
    use super::Id;

    #[derive(Parse, Unparse)]
    pub enum Expr<S> {
        Lit(Integer),
        Nested(Expr<S>),
    }

    impl<S> Expr<S> {
        // Without the qself issue this would be fine: both `self` and the return
        // type would be `__Rec`.  But the return type is NOT transformed, so it
        // stays as `<Expr<S> as Id>::Output` = concrete `Expr<S>` alias, while
        // `self` has type `__ExprRec<S, __Rec>`.  When `__Rec ≠ __ExprDefault<S>`
        // the types diverge → mismatched types error.
        pub fn identity(self) -> <Expr<S> as Id>::Output {
            self
        }
    }
}

fn main() {}
