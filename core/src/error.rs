use crate::span::Span;
use core::convert::Infallible;

pub trait Error: Sized {
    fn from_cause(cause: Vec<Self>) -> Self;
    fn into_parse_error(self) -> ParseError;
}

impl Error for ParseError {
    fn from_cause(cause: Vec<Self>) -> Self {
        let mut ret = Self::new((), "cannot parse");
        for c in cause {
            ret.add_sub_error(c);
        }
        ret
    }

    fn into_parse_error(self) -> ParseError {
        self
    }
}

impl Error for Infallible {
    fn from_cause(_cause: Vec<Self>) -> Self {
        unreachable!()
    }

    fn into_parse_error(self) -> ParseError {
        match self {}
    }
}

#[derive(Debug, Clone)]
pub struct ParseError {
    /// The `Debug` rendering of the span the error was reported at, or `None` for an unspanned one.
    ///
    /// Why a rendering rather than the span itself: `ParseError` is a single concrete type used as
    /// the error of every `Parse` impl, so it cannot be generic over the span without infecting the
    /// whole trait; and it cannot hold `Box<dyn Span>` because [`Span::migrate`] takes `self` by
    /// value and names `Self`, so `Span` is not object-safe. Erasing to `Box<dyn Any>` instead would
    /// force `'static`, which rules out a borrowed span like `Sp<'a>` in
    /// `tests/recurse_borrowed_stream.rs` — a case the crate deliberately supports.
    ///
    /// A previous attempt left `// span: Box<dyn Span>` commented out in this struct for exactly
    /// that reason. If typed recovery is wanted later it needs `ParseError<'a, S>` or a lifetime,
    /// which is a much larger change; this keeps the position reportable in the meantime.
    span: Option<String>,
    message: String,
    sub_errors: Vec<Self>,
}

impl ParseError {
    pub fn new<S: Span>(span: S, message: impl core::fmt::Display) -> Self {
        Self {
            // `()` is the span of an unspanned atom; rendering it would prefix every message with
            // a useless "()".
            span: (core::any::type_name::<S>() != "()").then(|| format!("{span:?}")),
            message: format!("{message}"),
            sub_errors: Vec::new(),
        }
    }

    pub fn add_sub_error(&mut self, error: Self) -> &mut Self {
        self.sub_errors.push(error);
        self
    }

    /// The `Debug` rendering of the span this error was reported at, if it had one.
    pub fn span_debug(&self) -> Option<&str> {
        self.span.as_deref()
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    /// The alternatives that failed, for an error built by [`Error::from_cause`]. An aggregate
    /// carries no span of its own — the positions live on these.
    pub fn sub_errors(&self) -> &[Self] {
        &self.sub_errors
    }
}

impl std::error::Error for ParseError {}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.span {
            Some(span) => write!(f, "{span}: {}", self.message)?,
            None => f.write_str(&self.message)?,
        }
        // An aggregate from `from_cause` is unspanned, so without this the position of every
        // alternative that actually failed would be invisible.
        for sub in &self.sub_errors {
            for line in format!("{sub}").lines() {
                write!(f, "\n  {line}")?;
            }
        }
        Ok(())
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

impl UnionWith<ParseError> for Infallible {
    type Output = ParseError;
    fn use_left(self) -> Self::Output {
        match self {}
    }
    fn use_right(rhs: ParseError) -> Self::Output {
        rhs
    }
}

impl UnionWith<Infallible> for ParseError {
    type Output = ParseError;
    fn use_left(self) -> Self::Output {
        self
    }
    fn use_right(rhs: Infallible) -> Self::Output {
        match rhs {}
    }
}

impl UnionWith<ParseError> for ParseError {
    type Output = ParseError;
    fn use_left(self) -> Self::Output {
        self
    }
    fn use_right(rhs: ParseError) -> Self::Output {
        rhs
    }
}

impl From<Infallible> for ParseError {
    fn from(infallible: Infallible) -> Self {
        match infallible {}
    }
}
