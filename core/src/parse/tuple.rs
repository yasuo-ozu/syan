use super::Parse;
use crate::error::ParseError;
use crate::parse::unparse::{Emitter, Unparse};
use crate::span::Spanned;

#[doc(hidden)]
macro_rules! __syan_wrap_err {
        ($e:expr; []; $ecur:ident) => {
            < $ecur as crate::error::UnionWith<_> >::use_left($e)
        };
        ($e:expr; [$head:ident $(, $tail:ident)*]; $ecur:ident) => {
            < $head as crate::error::UnionWith<_> >::use_right(
                __syan_wrap_err!($e; [ $($tail),* ]; $ecur)
            )
        };
    }

#[doc(hidden)]
macro_rules! __syan_parse_bindings {
    (@inner $stream:ident [$($prev:ident),*]; [$Ehead:ident $(, $Etail:ident)*]; [$ahead:ident $(, $atail:ident)*]) => {
        let $ahead = ::core::result::Result::map_err(
            Parse::parse_stream(&mut *$stream),
            |e| __syan_wrap_err!(e; [ $($prev),* ]; $Ehead),
        )?;
        __syan_parse_bindings!(@inner $stream [ $($prev,)* $Ehead ]; [ $($Etail),* ]; [ $($atail),* ]);
    };
    (@inner $stream:ident [$($prev:ident),*]; []; []) => {};
}

// ===== 1 arity 実装器（where節にマクロ呼び出しを置かない） ====================

#[doc(hidden)]
macro_rules! __syan_tuple_parse_impl_one {
    // (A...), (E...), (M...), M_last, (O...), (a...)
    (($($A:ident),+),
     ($($E:ident),+),
     ($($M:ident),+),
     $MLast:ident,
     ($($O:ident),+),
     ($($a:ident),+)) => {
        impl<
            $($A,)+
            __SyanMacroAtom: Spanned,
            __SyanError,
            $($E,)+
            $($M,)+
        > crate::parse::parse::Parse<__SyanMacroAtom> for ( $($A),+ )
        where
            // A_i: Parse<Atom, Error = E_i>
            $( $A: crate::parse::parse::Parse<__SyanMacroAtom, Error = $E>, )+
            // E_i: UnionWith<M_i, Output = O_i>
            $( $E: crate::error::UnionWith<$M, Output = $O>, )+
            // Infallible: UnionWith<Infallible, Output = last M>
            ::core::convert::Infallible:
                crate::error::UnionWith<::core::convert::Infallible, Output = $MLast>,
            __SyanError: crate::error::Error
                + Into<ParseError<crate::span::SpanOf<__SyanMacroAtom>>>,
        {
            type Error = __SyanError;
            fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = __SyanMacroAtom>>(__syan_stream: &mut __S) -> ::core::result::Result<Self, Self::Error> {
                __syan_parse_bindings!(@inner __syan_stream []; [ $($E),+ ]; [ $($a),+ ]);
                ::core::result::Result::Ok( ( $($a),+ ) )
            }

        }
        impl<
            $($A,)+
            __SyanMacroAtom,
        > crate::parse::unparse::Unparse<__SyanMacroAtom> for ( $($A),+ )
        where
            // A_i: Parse<Atom, Error = E_i>
            $( $A: crate::parse::unparse::Unparse<__SyanMacroAtom>, )+
        {
            fn unparse<__SyanMacroS: crate::parse::unparse::Emitter<__SyanMacroAtom>>(&self, sink: &mut __SyanMacroS) -> ::core::result::Result<(), __SyanMacroS::Error> {
                let ($($a),+) = self;
                $(
                    $A::unparse($a, sink)?;
                )*
                Ok(())
            }
        }
    };
}
macro_rules! impl_for_tup {
    (@imp [$($a:ident)*][$($A:ident)*][$($e:ident)*] $mf:ident [$($m:ident)*] $m0:ident $m1:ident $($ms:ident)*) => {
        impl_for_tup!(@imp [$($a)*] [$($A)*][$($e)*] $mf [$($m)* $m0] $m1 $($ms)*);
    };
    (@imp [$($a:ident)*][$($A:ident)*][$($e:ident)*] $mf:ident [$($m:ident)*] $ml:ident) => {
        __syan_tuple_parse_impl_one! {
            ($($A),*), ($($e),*), ($mf $(,$m)*, $ml), $ml, (__SyanError, $mf $(,$m)*), ($($a),*)
        }
    };
    ([$a:ident][$A:ident][$e:ident][$m:ident]) => {};
    ([$a:ident $($as:ident)*] [$A:ident $($As:ident)*] [$e:ident $($es:ident)*] [$m:ident $($ms:ident)*]) => {
        impl_for_tup!([$($as)*] [$($As)*] [$($es)*] [$($ms)*]);
        impl_for_tup!(@imp [$a $($as)*][$A$($As)*][$e$($es)*]$m[]$($ms)*);
    };
}
impl_for_tup!(
    [a0 a1 a2 a3 a4 a5 a6 a7 a8 a9 a10 a11 a12 a13]
    [A0 A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11 A12 A13]
    [__SyanError0 __SyanError1 __SyanError2 __SyanError3 __SyanError4 __SyanError5 __SyanError6 __SyanError7 __SyanError8 __SyanError9 __SyanError10 __SyanError11 __SyanError12 __SyanError13]
    [__SyanErrorMerged0 __SyanErrorMerged1 __SyanErrorMerged2 __SyanErrorMerged3 __SyanErrorMerged4 __SyanErrorMerged5 __SyanErrorMerged6 __SyanErrorMerged7 __SyanErrorMerged8 __SyanErrorMerged9 __SyanErrorMerged10 __SyanErrorMerged11 __SyanErrorMerged12 __SyanErrorMerged13]
);

impl<Atom: crate::span::Spanned> Parse<Atom> for () {
    type Error = core::convert::Infallible;
    fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = Atom>>(
        _: &mut __S,
    ) -> Result<Self, Self::Error> {
        Ok(())
    }
}

impl<Atom> Unparse<Atom> for () {
    fn unparse<S: Emitter<Atom>>(&self, _sink: &mut S) -> Result<(), S::Error> {
        Ok(())
    }
}

impl<T, Atom: crate::span::Spanned> Parse<Atom> for (T,)
where
    T: Parse<Atom>,
{
    type Error = T::Error;
    fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = Atom>>(
        stream: &mut __S,
    ) -> Result<Self, Self::Error> {
        Ok((T::parse_stream(&mut *stream)?,))
    }
}

impl<T, Atom> Unparse<Atom> for (T,)
where
    T: Unparse<Atom>,
{
    fn unparse<S: Emitter<Atom>>(&self, sink: &mut S) -> Result<(), S::Error> {
        self.0.unparse(sink)
    }
}
