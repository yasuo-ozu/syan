// AUDIT 4 (now a clean error): a non-root cycle type with its OWN extra generic
// parameter is rejected with a clear message.
//
// `Expr<S>` is the root; `Stmt<S, T>` carries an extra param `T`. The visitor
// derives every signature from the ROOT's params only, so a non-root type whose
// params are not a prefix of the root's cannot be named correctly. The generator
// now checks this up front and aborts, naming the offending type and parameter,
// rather than emitting wrong-arity `__StmtRec` signatures.
//
// (The base `#[recurse]` path — without `visit` — still compiles but silently
// binds `Stmt`'s `T` to a depth alias in the public `Stmt<S>` alias; that latent
// base-path gap is out of scope here.)

use syan::parse::recurse;

#[recurse(visit)]
mod ast {
    use core::marker::PhantomData;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub enum Expr<S> {
        Stmt(Box<Stmt<S, u32>>),
        Lit(PhantomData<S>),
    }

    #[derive(Ast)]
    #[subast()]
    pub enum Stmt<S, T> {
        Back(Box<Expr<S>>),
        Val(PhantomData<(S, T)>),
    }
}

fn main() {}
