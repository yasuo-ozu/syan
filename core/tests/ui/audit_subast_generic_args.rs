// A `#[visiting(path<GenericArgs>)]` entry is rejected at the derive with a clear, actionable message: a
// subast path names a type by path ONLY (it is used verbatim as a metadata-macro fetch target,
// `path! { .. }`, and a drill scrutinee, where `<..>` is illegal). Previously the generic args were
// accepted silently and surfaced far away as a cryptic "expected one of `!` or `::`, found `<`".
use core::marker::PhantomData;
use syan::visit::Visiting;

#[derive(Debug, Visiting)]
pub enum Type<S> {
    Unit(PhantomData<S>),
}

#[derive(Debug, Visiting)]
#[visiting(crate::Type)]
pub struct Cast<S>(pub Type<S>);

#[derive(Debug, Visiting)]
#[visiting(crate::Cast<()>)] // generic args — rejected at the derive
pub enum Expr<S> {
    Cast(Cast<S>),
    Lit(PhantomData<S>),
}

fn main() {}
