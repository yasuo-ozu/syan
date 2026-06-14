//! Stage 2: `#[derive(Ast)]` emits the marker impl, the metadata callback macro, and the
//! macro-namespace re-export under the type's own name.

use core::marker::PhantomData;
use syan::visit::Ast;

#[derive(Ast)]
pub enum Expr<S> {
    Stmt(Box<Stmt<S>>),
    Other(PhantomData<S>),
}

#[derive(Ast)]
pub enum Stmt<S> {
    Expr(Box<Expr<S>>),
    Other(PhantomData<S>),
}

#[derive(Ast)]
pub struct Wrap<S> {
    pub inner: Expr<S>,
}

/// `Ast` (the marker trait) is implemented.
fn assert_is_ast<T: Ast>() {}

#[test]
fn marker_trait_is_implemented() {
    assert_is_ast::<Expr<()>>();
    assert_is_ast::<Stmt<()>>();
    assert_is_ast::<Wrap<()>>();
}

#[test]
fn metadata_macro_round_trips_definition() {
    // A local callback that receives the forwarded `@ast { <item> }` and re-parses the embedded
    // definition into a real (renamed) type, proving the metadata macro carries a syn-parseable
    // copy of the original definition and that the path-callback invocation works.
    macro_rules! rebuild {
        (@ast { $item:item }) => {
            // The forwarded tokens parse as a single item; re-emit it in a fresh module so it does
            // not clash with the original `Expr`.
            #[allow(dead_code)]
            mod rebuilt {
                use super::*;
                $item
            }
        };
    }

    // `Expr!` resolves to the re-exported metadata macro (macro namespace), distinct from the
    // `Expr` enum (type namespace).
    Expr! { @ast rebuild {} }
    let _ = rebuilt::Expr::Other(PhantomData::<()>);
}
