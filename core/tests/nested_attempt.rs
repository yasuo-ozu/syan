//! `Parse::attempt()` (and its field-type form `Attempt<T>`) parses atomically: on failure it **rewinds**
//! the stream (unlike a plain `T`, which leaves it half-consumed) while still **propagating the error**
//! (unlike `Option<T>`, which becomes `None`).
use syan::nested::Attempt;
use syan::parse::{IntoParseStream, Parse, Unparse};
use syan::source::proc_macro2::literal::{Bool, Integer};
use template_quote::quote;

#[test]
fn method_attempt_wraps_value() {
    // `value.attempt()` wraps a parsed value into the `Attempt` marker (sugar for `Attempt(value)`).
    let int = Integer { value: "5".into(), suffix: None };
    let a: Attempt<Integer> = int.attempt();
    assert_eq!(a.value, "5"); // via `Deref`
}

#[test]
fn success_holds_inner_and_round_trips() {
    let a: Attempt<Integer> = Parse::parse(quote! { 5 }).unwrap();
    assert_eq!(a.value, "5"); // via `Deref`
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    a.unparse(&mut (&mut out)).unwrap();
    assert_eq!(out.iter().map(|t| t.to_string()).collect::<String>(), "5");
}

#[test]
fn failure_propagates_error_unlike_option() {
    // `Option<Integer>` on `true` → `Ok(None)` (error swallowed); `Attempt<Integer>` → `Err`.
    let opt: Option<Integer> = Parse::parse(quote! { true }).unwrap();
    assert!(opt.is_none());
    let att: Result<Attempt<Integer>, _> = Parse::parse(quote! { true });
    assert!(att.is_err());
}

#[test]
fn failure_rewinds_stream() {
    // `Attempt<(Integer, Bool)>` on `5 6`: parses `Integer` (5), then `Bool` fails on `6`, so the attempt
    // REWINDS to the start. A plain `(Integer, Bool)` would leave the stream past `5`; after the rewind a
    // following `(Integer, Integer)` sees both `5` and `6`.
    let mut stream = quote! { 5 6 }.into_parse_stream();
    let first: Result<Attempt<(Integer, Bool)>, _> = Parse::parse(&mut stream);
    assert!(first.is_err());
    let pair: (Integer, Integer) = Parse::parse(&mut stream).unwrap();
    assert_eq!(pair.0.value, "5");
    assert_eq!(pair.1.value, "6");
}

// A `visitor!()` walks straight through `Attempt` (a transparent `Deref` wrapper, like `Box`) to its
// inner AST type — directly and behind containers.
mod vis {
    use core::marker::PhantomData;
    use syan::nested::Attempt;
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast()]
    pub struct Leaf<S>(pub PhantomData<S>);

    #[derive(Ast)]
    #[subast(crate::vis::Leaf)]
    pub struct Holder<S> {
        pub a: Attempt<Leaf<S>>,
        pub va: Vec<Attempt<Leaf<S>>>,
        pub av: Attempt<Vec<Leaf<S>>>,
    }

    syan::visit::visitor!(crate::vis::Leaf, crate::vis::Holder);
}

#[test]
fn visitor_peels_through_attempt() {
    use core::marker::PhantomData;
    use vis::{Holder, Leaf};
    let h: Holder<()> = Holder {
        a: Attempt(Leaf(PhantomData)),
        va: vec![Attempt(Leaf(PhantomData)), Attempt(Leaf(PhantomData))],
        av: Attempt(vec![Leaf(PhantomData)]),
    };
    let mut n = 0usize;
    h.visit(|_: &Leaf<()>| n += 1);
    assert_eq!(n, 4, "1 (Attempt) + 2 (Vec<Attempt>) + 1 (Attempt<Vec>)");
}
