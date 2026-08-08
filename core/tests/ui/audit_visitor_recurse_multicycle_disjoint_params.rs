// AUDIT B (fixed → regression): one `visitor!()` spanning two independent cycles whose roots have
// DISJOINT generic params (`Expr<S>` + `Foo<T>`) is now rejected with a clean `abort!`.
//
// Was a miscompile: `generate_module_mixed` keyed the depth trait on the global union `{S, T}` and
// applied it to each per-cycle terminator (`impl<S, T, ..> VisitRec<S, T, __V> for ExprTerm<S>` — but
// `ExprTerm` takes only `<S>`), giving an E0107 + E0277 cascade. Now a guard requires all roots across
// the spanned cycles to share identical params, else this clear error. (A single cycle — incl.
// heterogeneous non-root extras — and same-param multi-cycle are unaffected; see
// `visitor_recurse_multicycle_via_visitor.rs`.)

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

    #[derive(Ast)]
    #[subast()]
    pub enum Foo<T> {
        Nest(Box<Foo<T>>),
        Lit(PhantomData<T>),
    }
}

mod v {
    syan::visit::visitor!(crate::m::Expr, crate::m::Foo);
}

fn main() {}
