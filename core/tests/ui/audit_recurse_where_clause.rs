// AUDIT (compile error): a where-clause on a #[recurse] cycle type is copied verbatim onto the
// internal `__ExprRec` but is NOT threaded onto the regenerated public alias, depth default, or the
// `visitor!()`-generated Visit / VisitRec traits/impls. So `where S: Clone` stays a bound on
// `__ExprRec` while the visitor traits that instantiate it do not satisfy it -> cryptic E0277
// "required by a bound in `__ExprRec`". recurse never inspects generics.where_clause. Removing the
// clause makes the identical type compile. Fix: thread it through, or abort! clearly.
use syan::parse::recurse;

#[recurse]
mod m {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S>
    where
        S: Clone,
    {
        Nest(Box<Expr<S>>),
        Lit(PhantomData<S>),
    }
}

mod v {
    syan::visit::visitor!(crate::m::Expr);
}

fn main() {}
