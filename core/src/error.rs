use crate::span::Span;
use core::convert::Infallible;

/// The aggregation half of an error type: how a set of failed alternatives becomes one error.
///
/// Conversion of a leaf error into the enclosing one is plain `Into`, not part of this trait.
pub trait Error: Sized {
    /// Fold the failures of every alternative into a single error.
    fn from_cause(cause: Vec<Self>) -> Self;

    /// Fold the failures of an **ordered choice** — alternatives all attempted from the same
    /// position, as a derived `enum` attempts its variants. That common start is what makes the
    /// failures comparable, so this is the only fold that may rank them;
    /// [`from_cause`](Self::from_cause) takes causes from anywhere and cannot.
    fn from_alternatives(cause: Vec<Self>) -> Self {
        Self::from_cause(cause)
    }
}

impl<S: Span> Error for ParseError<S> {
    fn from_cause(cause: Vec<Self>) -> Self {
        // Unranked causes: the aggregate takes the FIRST one's span, since spans from unrelated
        // attempts (a `#[group]`'s content parses on a stream of its own) are not comparable.
        let span = cause.first().map(|c| c.span().clone()).unwrap_or_default();
        ParseError::Alternatives {
            span,
            alts: cause.into_boxed_slice(),
        }
    }

    fn from_alternatives(cause: Vec<Self>) -> Self {
        // Farthest failure wins: the alternative that got closest to matching is where the input
        // actually goes wrong. `migrate` is the source's own "which position reaches further" rule
        // and keeps `self` on a tie, so an all-tied aggregate still reports the first alternative.
        // Reducing rather than folding from `S::default()` keeps that tie off a synthetic span.
        let span = cause
            .iter()
            .map(|c| c.span().clone())
            .reduce(|acc, span| acc.migrate(span))
            .unwrap_or_default();
        ParseError::Alternatives {
            span,
            alts: cause.into_boxed_slice(),
        }
    }
}

impl Error for Infallible {
    fn from_cause(_cause: Vec<Self>) -> Self {
        unreachable!()
    }
}

/// Which kind of literal a [`ParseError::Literal`] is about.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LitKind {
    Bool,
    Char,
    ByteChar,
    Str,
    ByteStr,
    CStr,
    Int,
    Float,
}

impl LitKind {
    /// The name of this kind as it appears in an error message, such as `"a string literal"`.
    pub fn as_str(self) -> &'static str {
        match self {
            LitKind::Bool => "a boolean literal",
            LitKind::Char => "a character literal",
            LitKind::ByteChar => "a byte-character literal",
            LitKind::Str => "a string literal",
            LitKind::ByteStr => "a byte-string literal",
            LitKind::CStr => "a C-string literal",
            LitKind::Int => "an integer literal",
            LitKind::Float => "a float literal",
        }
    }
}

/// What went wrong, as data: the kind is a variant, the detail is `&'static str` or a small `Copy`
/// enum, and the span is held by value and rendered only by [`Display`](core::fmt::Display).
///
/// The enum is `#[non_exhaustive]`, so a downstream `match` needs a `_` arm and variants are built
/// through the constructors ([`expected`](Self::expected), [`other`](Self::other), …). `S` defaults
/// to `()`, so an atom that carries no spans needs no annotation.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError<S = ()> {
    /// A specific thing was expected here and something else was found.
    Expected { span: S, what: &'static str },
    /// Input ended where something was still expected.
    Eof { span: S },
    /// A delimited group was expected.
    Group { span: S },
    /// Spacing was wrong: `want_joint` records which of `#[joint]` / `#[alone]` was asked for.
    Spacing { span: S, want_joint: bool },
    /// A literal of this kind was expected, or was malformed.
    Literal { span: S, kind: LitKind },
    /// Every alternative of an enum failed. `span` is the aggregate's position; `alts` are the
    /// individual failures, which is where the useful detail lives.
    Alternatives { span: S, alts: Box<[Self]> },
    /// Escape hatch for a hand-written parser. The only allocating variant.
    Other(Box<str>, S),
}

impl<S: Span> ParseError<S> {
    /// `what` was expected at `span` and something else was found.
    pub fn expected(span: S, what: &'static str) -> Self {
        ParseError::Expected { span, what }
    }
    /// Input ended at `span` where something was still expected.
    pub fn eof(span: S) -> Self {
        ParseError::Eof { span }
    }
    /// A delimited group was expected at `span`.
    pub fn group(span: S) -> Self {
        ParseError::Group { span }
    }
    /// Spacing was wrong at `span`; `want_joint` says whether a separator was forbidden or required.
    pub fn spacing(span: S, want_joint: bool) -> Self {
        ParseError::Spacing { span, want_joint }
    }
    /// A literal of `kind` was expected at `span`, or was malformed.
    pub fn literal(span: S, kind: LitKind) -> Self {
        ParseError::Literal { span, kind }
    }
    /// The allocating escape hatch. Prefer a structured variant where one fits.
    pub fn other(span: S, message: impl Into<Box<str>>) -> Self {
        ParseError::Other(message.into(), span)
    }

    /// Where the failure was reported. Total: every variant carries one.
    pub fn span(&self) -> &S {
        match self {
            ParseError::Expected { span, .. }
            | ParseError::Eof { span }
            | ParseError::Group { span }
            | ParseError::Spacing { span, .. }
            | ParseError::Literal { span, .. }
            | ParseError::Alternatives { span, .. }
            | ParseError::Other(_, span) => span,
        }
    }

    /// The alternatives of an [`Alternatives`](Self::Alternatives) aggregate, else empty.
    pub fn alternatives(&self) -> &[Self] {
        match self {
            ParseError::Alternatives { alts, .. } => alts,
            _ => &[],
        }
    }
}

impl<S: Span> std::error::Error for ParseError<S> {}

impl<S: Span> core::fmt::Display for ParseError<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `()` carries no position, so skip it rather than print "()".
        let spanned = core::any::type_name::<S>() != "()";
        match self {
            ParseError::Alternatives { alts, .. } => {
                f.write_str("expected ")?;
                for (i, a) in alts.iter().enumerate() {
                    if i > 0 {
                        f.write_str(if i + 1 == alts.len() { ", or " } else { ", " })?;
                    }
                    write!(f, "{a}")?;
                }
                if alts.is_empty() {
                    f.write_str("something else")?;
                }
                return Ok(());
            }
            ParseError::Expected { what, .. } => write!(f, "expected {what}")?,
            ParseError::Eof { .. } => f.write_str("unexpected end of input")?,
            ParseError::Group { .. } => f.write_str("expected a delimited group")?,
            ParseError::Spacing { want_joint, .. } => f.write_str(if *want_joint {
                "expected no space here"
            } else {
                "expected a space here"
            })?,
            ParseError::Literal { kind, .. } => write!(f, "expected {}", kind.as_str())?,
            ParseError::Other(msg, _) => f.write_str(msg)?,
        }
        if spanned {
            write!(f, " at {:?}", self.span())?;
        }
        Ok(())
    }
}

impl<S: Span> From<Infallible> for ParseError<S> {
    fn from(infallible: Infallible) -> Self {
        match infallible {}
    }
}

/// Merges two error types into the one that can carry both.
///
/// Reach for it where a parse has two independent failure sources — the item and the separator of a
/// [`Punctuated`](crate::nested::punctuated::Punctuated), say — and either may be [`Infallible`].
pub trait UnionWith<Rhs>: Sized {
    /// The error type both sides are lifted into.
    type Output: Error;
    /// Lift an error produced by the left-hand side.
    fn use_left(self) -> Self::Output;
    /// Lift an error produced by the right-hand side.
    fn use_right(rhs: Rhs) -> Self::Output;
}

impl UnionWith<Infallible> for Infallible {
    type Output = Infallible;
    fn use_left(self) -> Self::Output {
        match self {}
    }
    fn use_right(rhs: Infallible) -> Self::Output {
        match rhs {}
    }
}

impl<S: Span> UnionWith<ParseError<S>> for Infallible {
    type Output = ParseError<S>;
    fn use_left(self) -> Self::Output {
        match self {}
    }
    fn use_right(rhs: ParseError<S>) -> Self::Output {
        rhs
    }
}

impl<S: Span> UnionWith<Infallible> for ParseError<S> {
    type Output = ParseError<S>;
    fn use_left(self) -> Self::Output {
        self
    }
    fn use_right(rhs: Infallible) -> Self::Output {
        match rhs {}
    }
}

impl<S: Span> UnionWith<ParseError<S>> for ParseError<S> {
    type Output = ParseError<S>;
    fn use_left(self) -> Self::Output {
        self
    }
    fn use_right(rhs: ParseError<S>) -> Self::Output {
        rhs
    }
}
