use crate::error::ParseError;
use crate::parse::{IntoParseStream, Parse, ParseStream, Unparse};
use crate::span::Spanned;
use core::{marker::PhantomData, mem::transmute};
use std::any::Any;

#[doc(hidden)]
#[macro_export]
macro_rules! _Choice {
    (@impl) => {()};
    (@impl $t0:ty $(,$t:ty)*) => {
        ($t0, $crate::nested::choice::Choice!($($t),*))
    };
    ($($t:ty),*$(,)?) => {
        $crate::nested::choice::Choice<$crate::nested::choice::Choice!(@impl $($t:ty),*)>
    };
}

#[doc(inline)]
pub use _Choice as Choice;

pub struct Choice<HList>(Box<dyn Any>, PhantomData<HList>);

impl<Atom: Clone, T: 'static + Parse<Atom>> Parse<Atom> for Choice<(T, ())> {
    type Error = T::Error;
    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        let stream = stream.into_parse_stream();
        Ok(Self(
            Box::new(T::parse(stream)?) as Box<dyn Any>,
            PhantomData,
        ))
    }

    fn convert_error(error: Self::Error) -> ParseError<<Atom as Spanned>::Span>
    where
        Atom: Spanned,
    {
        T::convert_error(error)
    }
}

impl<Atom: Clone + Spanned, T: 'static + Parse<Atom>, U, HList> Parse<Atom>
    for Choice<(T, (U, HList))>
where
    Choice<(U, HList)>: Parse<Atom>,
    T::Error: crate::error::UnionWith<<Choice<(U, HList)> as Parse<Atom>>::Error>,
    <T::Error as crate::error::UnionWith<<Choice<(U, HList)> as Parse<Atom>>::Error>>::Output:
        Into<ParseError<<Atom as Spanned>::Span>>,
{
    type Error =
        <T::Error as crate::error::UnionWith<<Choice<(U, HList)> as Parse<Atom>>::Error>>::Output;
    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        match stream.dup(|stream| T::parse(stream)) {
            Ok(result) => Ok(Self(Box::new(result) as Box<dyn Any>, PhantomData)),
            Err(_) => {
                let result = <Choice<(U, HList)>>::parse(stream)
                    .map_err(<T::Error as crate::error::UnionWith<_>>::use_right)?;
                Ok(unsafe {
                    core::mem::transmute::<Choice<(U, HList)>, Choice<(T, (U, HList))>>(result)
                })
            }
        }
    }

    fn convert_error(error: Self::Error) -> ParseError<<Atom as Spanned>::Span>
    where
        Atom: Spanned,
    {
        error.into()
    }
}
impl<Atom: Clone> Unparse<Atom> for Choice<()> {
    fn unparse<S: crate::parse::unparse::Emitter<Atom>>(
        &self,
        _sink: &mut S,
    ) -> Result<(), S::Error> {
        unreachable!()
    }
}

impl<Atom: Clone, T: 'static + Unparse<Atom>, HList> Unparse<Atom> for Choice<(T, HList)>
where
    Choice<HList>: Unparse<Atom>,
{
    fn unparse<S: crate::parse::unparse::Emitter<Atom>>(
        &self,
        sink: &mut S,
    ) -> Result<(), S::Error> {
        if let Some(r) = self.0.downcast_ref::<T>() {
            r.unparse::<S>(sink)
        } else {
            let this = unsafe { transmute::<&Choice<(T, HList)>, &Choice<HList>>(self) };
            this.unparse(sink)
        }
    }
}

impl Clone for Choice<()> {
    fn clone(&self) -> Self {
        Self(Box::new(()) as _, PhantomData)
    }
}

impl<T: 'static + Clone, HList> Clone for Choice<(T, HList)>
where
    Choice<HList>: Clone,
{
    fn clone(&self) -> Self {
        if let Some(r) = self.0.downcast_ref::<T>() {
            Self(Box::new(r.clone()) as _, PhantomData)
        } else {
            let this = unsafe { transmute::<&Choice<(T, HList)>, &Choice<HList>>(self) };
            Self(this.clone().0, PhantomData)
        }
    }
}

impl core::fmt::Display for Choice<()> {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unreachable!()
    }
}

impl<T: 'static + core::fmt::Display, HList> core::fmt::Display for Choice<(T, HList)>
where
    Choice<HList>: core::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(rf) = self.0.downcast_ref::<T>() {
            rf.fmt(f)
        } else {
            unsafe { core::mem::transmute::<&Choice<(T, HList)>, &Choice<HList>>(self).fmt(f) }
        }
    }
}

impl core::fmt::Debug for Choice<()> {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unreachable!()
    }
}

impl<T: 'static + core::fmt::Debug, HList> core::fmt::Debug for Choice<(T, HList)>
where
    Choice<HList>: core::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(rf) = self.0.downcast_ref::<T>() {
            rf.fmt(f)
        } else {
            unsafe { core::mem::transmute::<&Choice<(T, HList)>, &Choice<HList>>(self).fmt(f) }
        }
    }
}
