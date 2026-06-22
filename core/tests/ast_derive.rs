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

#[derive(Ast)]
pub struct Inner<S>(pub PhantomData<S>);

// `#[subast(..)]` records this type's sub-AST children + their resolvable paths.
#[derive(Ast)]
#[subast(self::Inner)]
pub struct Outer<S> {
    pub child: Inner<S>,
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
fn repeater_is_implemented_on_the_type_itself() {
    // `#[derive(Ast)]` impls `Repeater<N>` on the AST type directly (no separate leaker host).
    // `Wrap { inner: Expr<S> }` has one context-dependent field, so `Repeater<0>` resolves.
    fn leaked0<T: syan::visit::Repeater<0>>() -> core::marker::PhantomData<T::Type> {
        core::marker::PhantomData
    }
    let _ = leaked0::<Wrap<()>>();
}

#[test]
fn metadata_macro_round_trips_definition() {
    // A local callback that receives the forwarded `@ast { <item> }` and re-parses the embedded
    // definition into a real (renamed) type, proving the metadata macro carries a syn-parseable
    // copy of the original definition and that the path-callback invocation works.
    macro_rules! rebuild {
        (@ast { $item:item } @subast { $($subast:tt)* }) => {
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

#[test]
fn subast_allowlist_round_trips() {
    // The metadata macro carries the `#[subast]` allowlist as `<path> as <matchkey>` entries. The
    // callback matches a single `$path:path as $key:ident`, proving both the format and that the
    // path resolves to a real type.
    macro_rules! check {
        (@ast { $item:item } @subast { $path:path as $key:ident }) => {
            use $path as SubastAlias;
            const SUBAST_KEY: &str = stringify!($key);
        };
    }
    Outer! { @ast check {} }
    let _: Option<SubastAlias<()>> = None;
    assert_eq!(SUBAST_KEY, "Inner");
}
