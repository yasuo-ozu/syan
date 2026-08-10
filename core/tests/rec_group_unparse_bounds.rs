// Parses `proc_macro2` tokens throughout; the whole suite is skipped without the optional
// dependency that provides them.
#![cfg(feature = "proc_macro2")]

//! Regression: the `Unparse` where-predicate synthesized for a `#[group(..)]` holder — §0(a) of
//! `known-gaps-rustyfi-port.md`, found porting a 173-type grammar.
//!
//! `extract_unparse` pushed `FieldTy: Unparse<Atom>` for **every** field, including the holder. A
//! holder never leaves through `Unparse` — it goes out via `GroupUnparse::unparse_group`, and the two
//! predicates covering that are pushed on the substruct branch. The extra bound is not merely
//! redundant: `GroupUnparse` is implemented generically over the delimiter types (`group.rs:184`)
//! but `Unparse for Group<..>` is **not** (only the `proc_macro2` `WithSpan<chars::OpenParen, _>`
//! shapes at `group.rs:19-78`), so any holder with delimiters of its own cannot satisfy it at any
//! atom.
//!
//! `Angle` below stands in for the port's `ParenGroup<()>`: a holder implementing `GroupShape` and
//! `GroupUnparse` at ONE CONCRETE atom and nothing else. Before the fix this file does not compile.

use syan::nested::group::{GroupShape, GroupUnparse};
use syan::parse::unparse::Emitter;
use syan::parse::{IntoParseStream, Parse, ParseStream, Unparse};
use syan::source::proc_macro2::Span;
use template_quote::quote;

use proc_macro2::TokenTree;

/// A `< … >`-delimited holder. Deliberately implements the two group traits **only** — no `Unparse`,
/// exactly like a consumer-defined delimiter pair.
#[derive(Debug, Default, Clone)]
pub struct Angle;

fn punct_is(tt: &TokenTree, ch: char) -> bool {
    matches!(tt, TokenTree::Punct(p) if p.as_char() == ch)
}

impl GroupShape<TokenTree> for Angle {
    fn parse_group<Slot>(
        stream: impl IntoParseStream<Atom = TokenTree>,
    ) -> Result<(Slot, Self), syan::error::ParseError>
    where
        Slot: Parse<TokenTree>,
    {
        let mut stream = stream.into_parse_stream();
        match stream.next() {
            Some(tt) if punct_is(&tt, '<') => {}
            Some(tt) => {
                stream.push(tt);
                return Err(syan::error::ParseError::new(
                    Span::default(),
                    "expected `<`",
                ));
            }
            None => return Err(syan::error::ParseError::new(Span::default(), "eof")),
        }
        let slot = Slot::parse(&mut stream).map_err(syan::error::Error::into_parse_error)?;
        match stream.next() {
            Some(tt) if punct_is(&tt, '>') => Ok((slot, Angle)),
            Some(tt) => {
                stream.push(tt);
                Err(syan::error::ParseError::new(
                    Span::default(),
                    "expected `>`",
                ))
            }
            None => Err(syan::error::ParseError::new(Span::default(), "eof")),
        }
    }
}

impl GroupUnparse<TokenTree> for Angle {
    fn unparse_group<Slot, E>(
        &self,
        slot: &Slot,
        sink: &mut E,
    ) -> Result<(), <E as Emitter<TokenTree>>::Error>
    where
        Slot: Unparse<TokenTree>,
        E: Emitter<TokenTree>,
    {
        sink.write_one(TokenTree::Punct(proc_macro2::Punct::new(
            '<',
            proc_macro2::Spacing::Alone,
        )))?;
        slot.unparse(sink)?;
        sink.write_one(TokenTree::Punct(proc_macro2::Punct::new(
            '>',
            proc_macro2::Spacing::Alone,
        )))
    }
}

#[syan::parse::recurse]
mod g {
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;

    // `inner` is a cycle member reached ONLY through the `angle` substruct.
    #[derive(Parse, Unparse)]
    pub enum Pat {
        Int(Integer),
        Angled {
            angle: super::Angle,
            #[group(self.angle)]
            inner: Box<Pat>,
        },
    }
}

/// A two-member SCC reached through the group substruct — the shape §0(b) is about, where the
/// substruct's field is `&'syan_substruct_ref Box<Body>` and `Body` is a *different* cycle member.
///
/// NOT A REGRESSION TEST for §0(b): this passes with or without the `peel_refs` half of the fix.
/// The port hit it on a 173-type grammar where the un-peeled spelling made `decycle` classify the
/// member as an outside leaf, so its premises were never hoisted and `SomeLeaf: Unparse<A>` was left
/// at a universally quantified atom. I could not shrink that to a minimal case — this module is the
/// closest shape that compiles, and it is kept as a smoke test of the two-member-through-a-group
/// path. `peel_refs` is retained on the strength of the port's evidence plus the fact that the two
/// spellings are equivalent as bounds (`&T: Unparse<A>` ⇐ `T: Unparse<A>`, `parse/unparse.rs:6`), so
/// it cannot weaken anything.
#[syan::parse::recurse]
mod m {
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;

    #[derive(Parse, Unparse)]
    pub enum PatBot {
        Int(Integer),
        Angled {
            angle: super::Angle,
            #[group(self.angle)]
            inner: Box<Body>,
        },
    }

    #[derive(Parse, Unparse)]
    pub struct Body {
        pub pat: Box<PatBot>,
    }
}

#[test]
fn a_second_cycle_member_behind_a_group_substruct_keeps_its_premises() {
    let src = quote! { < < 7 > > };
    let p: m::PatBot = Parse::parse(src.clone()).unwrap();
    let mut out = Vec::<TokenTree>::new();
    p.unparse(&mut (&mut out)).unwrap();
    assert_eq!(
        out.into_iter()
            .collect::<proc_macro2::TokenStream>()
            .to_string(),
        src.to_string(),
    );
}

#[test]
fn a_group_holder_needs_no_unparse_impl() {
    let src = quote! { < < 7 > > };
    let p: g::Pat = Parse::parse(src.clone()).unwrap();
    let mut out = Vec::<TokenTree>::new();
    p.unparse(&mut (&mut out)).unwrap();
    assert_eq!(
        out.into_iter()
            .collect::<proc_macro2::TokenStream>()
            .to_string(),
        src.to_string(),
    );
}

#[test]
fn deeply_nested_round_trips() {
    let mut src = quote! { 7 };
    for _ in 0..40 {
        src = quote! { < #src > };
    }
    let p: g::Pat = Parse::parse(src.clone()).unwrap();
    let mut out = Vec::<TokenTree>::new();
    p.unparse(&mut (&mut out)).unwrap();
    assert_eq!(
        out.into_iter()
            .collect::<proc_macro2::TokenStream>()
            .to_string(),
        src.to_string(),
    );
}
