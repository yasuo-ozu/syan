use super::{IntoParseStream, Parse};
use crate::error::ParseError;
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
            Parse::parse(&mut $stream),
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
            __SyanError: crate::error::Error + Into<ParseError<<__SyanMacroAtom as Spanned>::Span>>,
        {
            type Error = __SyanError;
            fn parse(
                __syan_stream: impl IntoParseStream<Atom = __SyanMacroAtom>,
            ) -> ::core::result::Result<Self, Self::Error> {
                let mut __syan_stream = __syan_stream.into_parse_stream();
                __syan_parse_bindings!(@inner __syan_stream []; [ $($E),+ ]; [ $($a),+ ]);
                ::core::result::Result::Ok( ( $($a),+ ) )
            }

            fn convert_error(error: Self::Error) -> ParseError<<__SyanMacroAtom as Spanned>::Span>
            where
                __SyanMacroAtom: Spanned,
            {
                error.into()
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

impl<Atom> Parse<Atom> for () {
    type Error = core::convert::Infallible;
    fn parse(_: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        Ok(())
    }

    fn convert_error(error: Self::Error) -> ParseError<<Atom as Spanned>::Span>
    where
        Atom: Spanned,
    {
        match error {}
    }
}

impl<T, Atom> Parse<Atom> for (T,)
where
    T: Parse<Atom>,
{
    type Error = T::Error;
    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        Ok((T::parse(stream)?,))
    }

    fn convert_error(error: Self::Error) -> ParseError<<Atom as Spanned>::Span>
    where
        Atom: Spanned,
    {
        T::convert_error(error)
    }
}
