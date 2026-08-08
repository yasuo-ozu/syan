//! The **`#[decycle]`-annotated definitions** of the three traits a `#[recurse]` cycle needs its
//! obligations broken for: [`Parse`], [`Unparse`] and [`Spanned`].
//!
//! # Why they live here and not next to their derives
//!
//! `#[recurse]` hands its module to `decycle`'s `process_module`, which reaches a trait defined in
//! another crate through a `#[decycle] use <path>;` item. That item works by invoking an *alter
//! macro* — a `macro_rules!` carrying the trait's own definition — that must be reachable under a
//! name the module can bind. If that macro were named `Parse` (decycle's default) it would land in
//! the same module as `pub use syan_macro::Parse` — the **derive** — and the two would collide in the
//! macro namespace (`E0252: the name Parse is defined multiple times`).
//!
//! So each trait is defined *here*, with `alter_macro_name` giving its alter macro a distinct,
//! `#[macro_export]`ed name (`__syan_decycle_Parse`, …, reachable as `::syan::__syan_decycle_Parse`),
//! and this module re-exports **only the trait** (type namespace). `parse::parse`, `parse::unparse`
//! and `span` then re-export the trait from here *and* the derive from `syan_macro` with no
//! collision, so `syan::parse::Parse` stays both a trait and a derive exactly as before. The
//! generated `#[recurse]` module binds the two namespaces separately:
//!
//! ```ignore
//! use ::syan::decycle_traits::Parse;                    // the trait   (type namespace)
//! #[decycle] use ::syan::__syan_decycle_Parse as Parse; // alter macro (macro namespace)
//! ```
//!
//! Every path inside these definitions must be **absolute** (`::syan::…`, `::core::…`): the
//! definition tokens are replayed inside the *consumer* crate, where `crate::`/`super::` would point
//! somewhere else entirely. `allowed_paths` tells decycle's type-leaker that those roots are already
//! globally reachable, so nothing needs interning (and no `marker` type is required).

/// See the module docs for why this definition lives here. Re-exported as `syan::parse::Parse`.
#[decycle::decycle(
    decycle = ::syan::__decycle,
    allowed_paths = [::syan, ::core, ::std],
    alter_macro_name = __syan_decycle_Parse
)]
pub trait Parse<Atom>: ::core::marker::Sized {
    type Error: ::syan::error::Error;

    fn parse(
        stream: impl ::syan::parse::into_parse_stream::IntoParseStream<Atom = Atom>,
    ) -> ::core::result::Result<Self, Self::Error>;

    /// Wrap this value in [`Attempt`](::syan::nested::Attempt), the **atomic-parse** marker: parsing
    /// an `Attempt<Self>` parses `Self` but rewinds the stream on failure (it requires
    /// `Atom: Clone`). This is the value constructor; `value.attempt()` is sugar for
    /// `Attempt(value)`.
    fn attempt(self) -> ::syan::nested::attempt::Attempt<Self> {
        ::syan::nested::attempt::Attempt(self)
    }
}

/// See the module docs for why this definition lives here. Re-exported as `syan::parse::Unparse`.
#[decycle::decycle(
    decycle = ::syan::__decycle,
    allowed_paths = [::syan, ::core, ::std],
    alter_macro_name = __syan_decycle_Unparse
)]
pub trait Unparse<Atom> {
    // NOTE: the associated type is spelled in FULL (`<S as Emitter<Atom>>::Error`, not the `S::Error`
    // shorthand). decycle's ranked engine reflects this signature onto a generated twin trait whose
    // method generics are renamed; a bare shorthand projection does not survive that rewrite (E0220).
    fn unparse<S: ::syan::parse::unparse::Emitter<Atom>>(
        &self,
        sink: &mut S,
    ) -> ::core::result::Result<(), <S as ::syan::parse::unparse::Emitter<Atom>>::Error>;
}

/// See the module docs for why this definition lives here. Re-exported as `syan::span::Spanned`.
#[decycle::decycle(
    decycle = ::syan::__decycle,
    allowed_paths = [::syan, ::core, ::std],
    alter_macro_name = __syan_decycle_Spanned
)]
pub trait Spanned {
    type Span: ::syan::span::Span;

    fn span(&self) -> Self::Span;
}
