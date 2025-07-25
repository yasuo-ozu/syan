use crate::span::Span;
use core::convert::Infallible;

pub trait Error: Sized {
    fn from_cause(cause: Vec<Self>) -> Self;
}

impl<S: Span> Error for ParseError<S> {
    fn from_cause(cause: Vec<Self>) -> Self {
        let mut ret = Self::new(S::default(), "cannot parse");
        for c in cause {
            ret.add_sub_error(c);
        }
        ret
    }
}

impl Error for Infallible {
    fn from_cause(_cause: Vec<Self>) -> Self {
        unreachable!()
    }
}

#[derive(Debug)]
pub struct ParseError<S> {
    span: S,
    message: String,
    sub_errors: Vec<Self>,
}

impl<S> ParseError<S> {
    pub fn new(span: S, message: impl core::fmt::Display) -> Self {
        Self {
            span,
            message: format!("{message}"),
            sub_errors: Vec::new(),
        }
    }

    pub fn add_sub_error(&mut self, error: Self) -> &mut Self {
        self.sub_errors.push(error);
        self
    }

    pub fn add_sub_errors(&mut self, errors: impl IntoIterator<Item = Self>) -> &mut Self {
        for error in errors.into_iter() {
            self.sub_errors.push(error);
        }
        self
    }

    pub fn map_span<T>(self, f: impl Fn(S) -> T + Copy) -> ParseError<T> {
        ParseError {
            span: f(self.span),
            message: self.message,
            sub_errors: self.sub_errors.into_iter().map(|e| e.map_span(f)).collect(),
        }
    }
    pub fn union_left<T>(self) -> ParseError<<S as UnionWith<T>>::Output>
    where
        S: UnionWith<T>,
    {
        ParseError {
            span: S::from_left(self.span),
            message: self.message,
            sub_errors: self
                .sub_errors
                .into_iter()
                .map(|e| e.union_left::<T>())
                .collect(),
        }
    }

    pub fn union_right<T>(self) -> ParseError<<T as UnionWith<S>>::Output>
    where
        T: UnionWith<S>,
    {
        ParseError {
            span: T::from_right(self.span),
            message: self.message,
            sub_errors: self
                .sub_errors
                .into_iter()
                .map(|e| e.union_right::<T>())
                .collect(),
        }
    }
}

impl<S: Clone> Clone for ParseError<S> {
    fn clone(&self) -> Self {
        Self {
            span: self.span.clone(),
            message: self.message.clone(),
            sub_errors: self.sub_errors.clone(),
        }
    }
}

impl<S: std::fmt::Debug> std::error::Error for ParseError<S> {}

impl<S> core::fmt::Display for ParseError<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

pub type Result<T, S> = core::result::Result<T, ParseError<S>>;

pub trait UnionWith<Rhs>: Sized {
    type Output;
    fn from_left(self) -> Self::Output;
    fn from_right(rhs: Rhs) -> Self::Output;
}

impl UnionWith<Infallible> for Infallible {
    type Output = Infallible;
    fn from_left(self) -> Self::Output {
        unreachable!()
    }
    fn from_right(rhs: Infallible) -> Self::Output {
        unreachable!()
    }
}

impl<S> UnionWith<ParseError<S>> for Infallible {
    type Output = ParseError<S>;
    fn from_left(self) -> Self::Output {
        unreachable!()
    }
    fn from_right(rhs: ParseError<S>) -> Self::Output {
        rhs
    }
}

impl<S> UnionWith<Infallible> for ParseError<S> {
    type Output = ParseError<S>;
    fn from_left(self) -> Self::Output {
        self
    }
    fn from_right(_: Infallible) -> Self::Output {
        unreachable!()
    }
}

impl<S> UnionWith<ParseError<S>> for ParseError<S> {
    type Output = ParseError<S>;
    fn from_left(self) -> Self::Output {
        self
    }
    fn from_right(rhs: ParseError<S>) -> Self::Output {
        rhs
    }
}
