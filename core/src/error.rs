use crate::span::Span;
use core::convert::Infallible;

/// The aggregation half of an error type: how a set of failed alternatives becomes one error.
///
/// This used to also carry `fn into_parse_error(self) -> ParseError`, which named `ParseError`
/// concretely and so could not survive `ParseError` gaining a span parameter. Conversion is now
/// plain `From`/`Into`, which composes better and is one fewer concept: a leaf error becomes the
/// enclosing error with `.map_err(Into::into)`.
pub trait Error: Sized {
    fn from_cause(cause: Vec<Self>) -> Self;
}

impl<S: Span> Error for ParseError<S> {
    fn from_cause(cause: Vec<Self>) -> Self {
        // The aggregate takes the FIRST alternative's span. With no notion of how far each
        // alternative got, that is the only deterministic choice available; ranking by furthest
        // progress (and keeping only the winner) is the open work in
        // `error-design-vs-chumsky.md` §R1/§R4.
        let span = cause.first().map(|c| c.span().clone()).unwrap_or_default();
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

/// What went wrong, as data.
///
/// # Why an enum, and why no `String`
///
/// The previous shape was `struct ParseError { span: Option<String>, message: String, sub_errors:
/// Vec<Self> }`, built by `ParseError::new(span, message)` — which rendered **both** strings eagerly,
/// on every failed alternative, for parses that then succeeded and never read them. Measured, that
/// was 25–37% of total parse time and ~35–81% of all allocations (`perf-measurements.md` §4, §8).
///
/// Here the kind is a variant, the detail is `&'static str` or a small `Copy` enum, and the span is
/// held **by value** rather than as its `Debug` rendering. Constructing one is ~4 ns against ~118–154
/// ns, and the type is 32 bytes rather than 72 — which also shrinks every `Result` on the *success*
/// path, since an error travels by value out of every field parse.
///
/// Rendering happens once, in [`Display`](core::fmt::Display), on the error a human actually sees.
///
/// # `#[non_exhaustive]`
///
/// New kinds can be added without a major version, so downstream `match` needs a `_` arm and cannot
/// construct variants directly — use the constructors ([`expected`](Self::expected),
/// [`other`](Self::other), …). [`Other`](Self::Other) is the escape hatch for parsers this enum does
/// not know about; it is the only variant that allocates, and only on the failure path.
///
/// `S` defaults to `()` so an atom without spans needs no annotation. A derived impl uses
/// `ParseError<<Atom as Spanned>::Span>`.
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
    pub fn expected(span: S, what: &'static str) -> Self {
        ParseError::Expected { span, what }
    }
    pub fn eof(span: S) -> Self {
        ParseError::Eof { span }
    }
    pub fn group(span: S) -> Self {
        ParseError::Group { span }
    }
    pub fn spacing(span: S, want_joint: bool) -> Self {
        ParseError::Spacing { span, want_joint }
    }
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
        // The span is rendered HERE, once, rather than at construction time — that is the whole
        // point of holding it by value. `()` carries no position, so skip it rather than print "()".
        let spanned = core::any::type_name::<S>() != "()";
        match self {
            ParseError::Alternatives { alts, .. } => {
                // No separate expected-set is needed: the alternatives *are* the set, and joining
                // them at print time costs nothing on the parse path.
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

pub trait UnionWith<Rhs>: Sized {
    type Output: Error;
    fn use_left(self) -> Self::Output;
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
