// AUDIT 2 (now a clean error): a nested container (`Vec<Option<Expr>>`) inside a
// `#[recurse(visit)]` cycle is rejected with a clear message.
//
// `recurse_dispatch_field` checks `Peeled::nested` and aborts, matching the
// `visitor!()` builder (which also rejects nested containers). Previously it
// ignored `nested` and emitted mistyped traversal code (`&Option<__R>` vs `&__R`).
// The fix for non-nested containers — `Vec<Box<Expr>>`, `Option<Box<Expr>>`,
// `Box<Option<Expr>>` — is exercised in `visitor_recurse_containers.rs`.

use syan::parse::recurse;

#[recurse(visit)]
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

fn main() {}
