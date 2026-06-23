// AUDIT (compile error): #[derive(Unparse)] (and #[derive(Spanned)]) silently DROP the user's
// where-clause from the generated impl header — only the macro's synthesized predicates are emitted.
// The impl Self type then fails its well-formedness obligation with a cryptic E0277
// "the trait bound `T: Clone` is not satisfied ... required by a bound in `UnparseWhere`", attributed
// to the derive. (The Parse derive panics on a where-clause instead; see
// audit_parse_where_clause_panic.rs.) Fix: restate the where predicates in the impl header.
use syan::parse::Unparse;
use syan::span::WithSpan;

#[derive(Unparse)]
pub struct UnparseWhere<S, T>
where
    T: Clone,
{
    a: WithSpan<u32, S>,
    b: T,
}

fn main() {}
