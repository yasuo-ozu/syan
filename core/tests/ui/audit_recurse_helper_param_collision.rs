// AUDIT (hygiene): recurse(visit) helper params __V / __R / __Rec are built with Span::call_site()
// (not hygienic). A cycle type that declares a generic param literally named __V (or __R / __Rec)
// collides -> E0403 "the name `__V` is already used for a generic parameter", with the span on the
// #[recurse(visit)] attribute. The visitor!() path fresh-names its helpers (see visitor_hygiene.rs);
// the recurse(visit) path was never given the same treatment. Fix: fresh-name the helpers, or
// abort! with a clear message when a cycle type uses a reserved name.
use syan::parse::recurse;

#[recurse(visit)]
mod m {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S, __V> {
        Nest(Box<Expr<S, __V>>),
        Lit(PhantomData<(S, __V)>),
    }
}

fn main() {}
