// AUDIT (missing diagnostic): a #[subast(path<GenericArgs>)] entry is silently accepted at the
// derive (parse_subast never checks for generic args; matchkey uses only the last segment ident),
// then — when the named type is an UNLISTED intermediate that the visitor drills inline — its path
// is used verbatim as a metadata-macro fetch target and a match scrutinee, e.g.
// `crate::Cast<()> ! { .. }`, which is illegal. The result is a cryptic "expected one of ! or ::,
// found <" pointing at the #[subast] attribute, far from the cause. Docs say paths take no generic
// args, but it's never validated. Fix: abort! at the derive when any segment carries `<...>`.
use core::marker::PhantomData;
use syan::visit::Ast;

#[derive(Debug, Ast)]
pub enum Type<S> {
    Unit(PhantomData<S>),
}

#[derive(Debug, Ast)]
#[subast(crate::Type)]
pub struct Cast<S>(pub Type<S>);

#[derive(Debug, Ast)]
#[subast(crate::Cast<()>)] // generic args silently accepted here
pub enum Expr<S> {
    Cast(Cast<S>),
    Lit(PhantomData<S>),
}

pub mod visit {
    // Cast is NOT listed -> drilled inline using its #[subast] path (which carries `<()>`).
    syan::visit::visitor!(crate::Expr, crate::Type);
}

fn main() {}
