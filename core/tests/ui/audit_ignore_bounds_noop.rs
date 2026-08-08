// AUDIT (silent no-op): #[ignore_bounds] is registered as a derive helper attribute (so it parses)
// but the code that would honor it is commented out in both extract_parse and extract_unparse — it
// is a silent no-op. Every non-default field unconditionally pushes `field_ty: Parse` into the
// where-clause. This probe proves the bound is still emitted: the assertion `S<NoParse>: Parse`
// fails to compile (it would hold if #[ignore_bounds] dropped the `T: Parse` bound for field `b`).
// Fix: honor the attribute (skip the predicate), or error/warn when it is used.
use proc_macro2::TokenTree;
use syan::parse::Parse;
use syan::source::proc_macro2::literal::Integer;

#[derive(Parse)]
pub struct S<T> {
    a: Integer,
    #[ignore_bounds]
    b: T,
}

pub struct NoParse;

fn _assert()
where
    S<NoParse>: Parse<TokenTree>,
{
}

fn main() {}
