// AUDIT (hygiene): the generated terminator struct `ExprTerm` (format!("{root}Term"),
// Span::call_site()) is emitted by `#[recurse]` into the user's module. A user item named `ExprTerm`
// in the same module collides -> E0428, with the #[recurse] attribute flagged as the prior
// definition. (The `__XxxDefault` alias and `__XxxRec` node types collide the same way but are
// underscore-prefixed and less likely.) `XxxTerm` (no leading underscore, derived from the root name)
// is the most plausible accidental clash. Fix: unguessable/hygienic generated names, or abort! on a clash.
use syan::parse::recurse;

#[recurse]
mod m {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        Nest(Box<Expr<S>>),
        Lit(PhantomData<S>),
    }

    pub struct ExprTerm; // collides with the generated terminator `pub struct ExprTerm;`
}

fn main() {}
