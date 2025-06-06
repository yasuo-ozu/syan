use core::convert::Infallible;
use std::any::Any;

pub trait ParseError: Any {}

pub trait Merge<Rhs>: Sized {
    type Output: Sized;
    fn from_left(_: Self) -> Self::Output;
    fn from_right(_: Rhs) -> Self::Output;
}

impl Merge<Infallible> for Infallible {
    type Output = Infallible;

    fn from_left(_: Self) -> Self::Output {
        panic!()
    }

    fn from_right(_: Infallible) -> Self::Output {
        panic!()
    }
}

impl<L: ParseError> Merge<Infallible> for L {
    type Output = L;

    fn from_left(l: Self) -> Self::Output {
        l
    }

    fn from_right(_: Infallible) -> Self::Output {
        panic!()
    }
}

impl<R: ParseError> Merge<R> for Infallible {
    type Output = R;

    fn from_left(_: Self) -> Self::Output {
        panic!()
    }

    fn from_right(r: R) -> Self::Output {
        r
    }
}

impl<L: ParseError, R: ParseError> Merge<R> for L {
    type Output = Box<dyn ParseError>;
    fn from_left(l: Self) -> Self::Output {
        Box::new(l) as Box<dyn ParseError>
    }

    fn from_right(r: R) -> Self::Output {
        Box::new(r) as Box<dyn ParseError>
    }
}
