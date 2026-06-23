// AUDIT (panic): #[derive(Parse)] panics on any type carrying a where-clause.
// macro/attribute.rs extract_parse opens with `assert!(generics.where_clause.is_none())`, and
// lib.rs::parse_derive forwards the generics verbatim (never stripping the clause). So any
// where-clause makes the Parse derive panic at expansion with the opaque message
// "assertion failed: generics.where_clause.is_none()" and no span on the clause. (Unparse/Spanned
// instead silently DROP the clause — see audit_unparse_where_clause.rs.)
// Fix: thread the where predicates into the generated impl, or abort! with a clear spanned message.
use syan::parse::Parse;
use syan::source::proc_macro2::literal::Integer;

trait Marker {}

#[derive(Parse)]
pub struct ParseWhere<S, T>
where
    T: Marker,
{
    a: Integer,
    _p: core::marker::PhantomData<(S, T)>,
}

fn main() {}
