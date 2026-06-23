// AUDIT (compile error): #[derive(Spanned)] generates non-compiling code for the natural span-
// parameterized node shape (fields like `WithSpan<u32, S>`). It invents a fresh `__Syan_Span: Span`,
// sets `type Span = __Syan_Span`, and migrates each field's span into it — but the only predicate
// tying anything to `__Syan_Span` is added for BARE UNBOUNDED type-param fields. For a composite or
// bounded field, `__Syan_Span` is unconstrained (E0207) and the migrate gets the field's own span
// type where `__Syan_Span` is expected (E0308). Only an all-bare-param struct works.
use syan::span::{Span, Spanned, WithSpan};

#[derive(Spanned)]
pub struct Node<S: Span> {
    a: WithSpan<u32, S>,
    b: WithSpan<u64, S>,
}

fn main() {}
