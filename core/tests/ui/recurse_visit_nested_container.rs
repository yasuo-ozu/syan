// AUDIT 2 (now a clean error): a nested container (`Vec<Option<Expr>>`) in a `#[recurse]` cycle that
// a `visitor!()` walks is rejected with a clear message.
//
// `#[recurse]` rewrites the back-edge type fine, but `visitor!()`'s shared body lowering
// (`util::recurse_lower_field`) checks `Peeled::nested` and aborts (it can't generate `&Option<__R>`
// vs `&__R` traversal). The fix for non-nested containers — `Vec<Box<Expr>>`, `Option<Box<Expr>>`,
// `Box<Option<Expr>>` — is exercised in `visitor_recurse_containers.rs`.

use syan::parse::recurse;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        Many(Vec<Option<Expr<S>>>),
        Lit(PhantomData<S>),
    }
}

mod v {
    syan::visit::visitor!(crate::ast::Expr);
}

fn main() {}
