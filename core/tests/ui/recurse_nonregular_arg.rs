// A cycle edge that WRAPS one of the referring type's own parameters (`Expr<S>` -> `Expr<Vec<S>>`)
// is non-regular recursion: the argument grows at every level, so the instantiation family
// `Expr<S>, Expr<Vec<S>>, Expr<Vec<Vec<S>>>, ...` is infinite and its impls cannot be generated.
//
// syan does NOT pre-check this (the front-end guard was removed); the rejection comes from decycle's
// `REACHABLE_OBLIGATIONS_CAP`. Pinned here because that message is matchable again: the type it
// names is decycle's own nesting alias, which is DETERMINISTIC (`__DecycleNat_<T>_<mod>`) rather
// than the random per-expansion nonce syan used to generate.
//
// Only `Parse` is derived, deliberately: the abort names the trait whose obligation walk hit the cap,
// and with two routed traits present WHICH one gets there first varies run to run (decycle iterates a
// `HashMap` of traits), which made this golden flake about one run in three.
//
// Passing parameters through unchanged (`Box<Expr<S>>`) and substituting a parameter-FREE concrete
// type (`Box<Stmt<S, u8>>`, see recurse_generics.rs) both remain supported.

use syan::parse::recurse;

#[recurse]
mod ast {
    use core::marker::PhantomData;
    use syan::parse::Parse;

    #[derive(Parse)]
    pub enum Expr<S> {
        Nest(Box<Expr<S>>),      // identity — fine
        Wrap(Box<Expr<Vec<S>>>), // grows `S` every level — REJECTED
        Lit(PhantomData<S>),
    }
}

fn main() {}
