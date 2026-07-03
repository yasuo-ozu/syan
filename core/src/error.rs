use crate::span::Span;
use core::any::Any;
use core::convert::Infallible;
use core::fmt::Debug;

pub trait Error: Sized {
    fn from_cause(cause: Vec<Self>) -> Self;
    fn into_parse_error(self) -> ParseError;
}

impl Error for ParseError {
    fn from_cause(cause: Vec<Self>) -> Self {
        let mut span = None;
        let mut sub_errors = Vec::with_capacity(cause.len());
        for c in cause {
            span = merge_span(span, c.span.as_deref());
            sub_errors.push(c);
        }
        ParseError {
            message: "cannot parse".to_string(),
            span,
            sub_errors,
        }
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

/// A span stored inside a [`ParseError`] with its concrete type erased, so `ParseError` stays
/// non-generic while still carrying position information for any [`Span`]. Blanket-implemented for
/// every `S: Span + 'static`; recover the concrete span with [`ParseError::span_of`].
pub trait ErasedSpan: Debug + 'static {
    fn as_any(&self) -> &dyn Any;
    fn clone_box(&self) -> Box<dyn ErasedSpan>;
    /// Merge two erased spans through the concrete [`Span::migrate`] when they share a type (the
    /// homogeneous case inside a single parse); a type mismatch keeps `self`.
    fn merge(&self, other: &dyn ErasedSpan) -> Box<dyn ErasedSpan>;
}

impl<S: Span + 'static> ErasedSpan for S {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn ErasedSpan> {
        Box::new(self.clone())
    }

    fn merge(&self, other: &dyn ErasedSpan) -> Box<dyn ErasedSpan> {
        match other.as_any().downcast_ref::<S>() {
            Some(o) => Box::new(self.clone().migrate(o.clone())),
            None => Box::new(self.clone()),
        }
    }
}

impl Clone for Box<dyn ErasedSpan> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Left-fold a child span into an accumulator through [`ErasedSpan::merge`]; for the homogeneous
/// spans of one parse this is the concrete [`Span::migrate`], which for positional spans yields the
/// furthest-progress span.
fn merge_span(
    acc: Option<Box<dyn ErasedSpan>>,
    next: Option<&dyn ErasedSpan>,
) -> Option<Box<dyn ErasedSpan>> {
    match (acc, next) {
        (acc, None) => acc,
        (None, Some(next)) => Some(next.clone_box()),
        (Some(acc), Some(next)) => Some(acc.merge(next)),
    }
}

#[derive(Debug, Clone)]
pub struct ParseError {
    message: String,
    span: Option<Box<dyn ErasedSpan>>,
    sub_errors: Vec<Self>,
}

impl ParseError {
    pub fn new(span: impl Span + 'static, message: impl core::fmt::Display) -> Self {
        Self {
            message: format!("{message}"),
            span: Some(Box::new(span)),
            sub_errors: Vec::new(),
        }
    }

    /// Recover the carried span as the concrete type `S`, if the stored span is an `S` (they always
    /// are within one homogeneous parse). Returns `None` when no span was carried or the types differ.
    pub fn span_of<S: Span + 'static>(&self) -> Option<S> {
        self.span.as_ref()?.as_any().downcast_ref::<S>().cloned()
    }

    pub fn add_sub_error(&mut self, error: Self) -> &mut Self {
        self.span = merge_span(self.span.take(), error.span.as_deref());
        self.sub_errors.push(error);
        self
    }
}

impl std::error::Error for ParseError {}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
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
