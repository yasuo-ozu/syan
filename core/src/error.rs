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
    message: String,
    sub_errors: Vec<Self>,
}

impl ParseError {
    pub fn new(_span: impl Span, message: impl core::fmt::Display) -> Self {
        Self {
            message: format!("{message}"),
            sub_errors: Vec::new(),
        }
    }

    pub fn add_sub_error(&mut self, error: Self) -> &mut Self {
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
