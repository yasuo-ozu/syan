pub mod chars {
    macro_rules! impl_char {
        ((@add_doc $name:ident $(($token:tt))?  $char:literal)) => {
            #[doc(hidden)]
            #[allow(non_camel_case_types)]
            #[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
            pub struct $name;
        };
        ((@add_doc $_:lifetime $name:ident $(($token:tt))?  $char:literal)) => {
            #[doc = concat!("Represents ", stringify!($char), "")]
            #[doc = ""]
            #[doc = "```"]
            #[doc = "# use syan::symbol::chars::*;"]
            #[doc = concat!("assert_eq!(&format!(\"{}\", ", stringify!($char), "), &format!(\"{}\", ", stringify!($name), "));")]
            #[doc = concat!("assert_eq!(&format!(\"{}\",", stringify!($char), "), &format!(\"{}\", Char!(", stringify!($char), ")));")]
            $(
                #[doc = stringify!("assert_eq!(\"", $char, "\", &format!(\"{}\", Char!(", $token, ")));")]
            )?
            #[doc = "```"]
            #[allow(non_camel_case_types)]
            #[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
            pub struct $name;
        };
        ($dollar:tt $($($lt:lifetime)? $name:ident $(($token:tt))?@$char:tt)*) => {
            $(
                impl_char!((@add_doc $($lt)? $name $char));

                impl core::default::Default for $name {
                    fn default() -> Self {
                        $name
                    }
                }

                impl core::fmt::Display for $name {
                    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        f.write_str(&format!("{}", $char))
                    }
                }
            )*

            #[doc(hidden)]
            #[macro_export]
            macro_rules! __Char {
                $(
                    $(
                        ($token) => { $dollar crate::symbol::chars::$name };
                    )?
                    ($char) => { $dollar crate::symbol::chars::$name };
                )*
                ($dollar) => { $dollar crate::symbol::chars::Dollar };
            }
        };
    }

    impl_char!(
        $
        _a(a)@'a' _b(b)@'b' _c(c)@'c' _d(d)@'d' _e(e)@'e' _f(f)@'f' _g(g)@'g' _h(h)@'h' _i(i)@'i'
        _j(j)@'j' _k(k)@'k' _l(l)@'l' _m(m)@'m' _n(n)@'n' _o(o)@'o' _p(p)@'p' _q(q)@'q' _r(r)@'r'
        _s(s)@'s' _t(t)@'t' _u(u)@'u' _v(v)@'v' _w(w)@'w' _x(x)@'x' _y(y)@'y' _z(z)@'z'
        _A(A)@'A' _B(B)@'B' _C(C)@'C' _D(D)@'D' _E(E)@'E' _F(F)@'F' _G(G)@'G' _H(H)@'H' _I(I)@'I'
        _J(J)@'J' _K(K)@'K' _L(L)@'L' _M(M)@'M' _N(N)@'N' _O(O)@'O' _P(P)@'P' _Q(Q)@'Q' _R(R)@'R'
        _S(S)@'S' _T(T)@'T' _U(U)@'U' _V(V)@'V' _W(W)@'W' _X(X)@'X' _Y(Y)@'Y' _Z(Z)@'Z'
        _0(0)@'0' _1(1)@'1' _2(2)@'2' _3(3)@'3' _4(4)@'4' _5(5)@'5' _6(6)@'6' _7(7)@'7' _8(8)@'8'
        _9(9)@'9' __(_)@'_'
        '_ Not(!)@'!'
        '_ Quot@'"'
        '_ Pound(#)@'#'
        '_ Dollar@'$'
        '_ Percnt(%)@'%'
        '_ And(&)@'&'
        '_ Apos@'\''
        '_ Star(*)@'*'
        '_ Plus(+)@'+'
        '_ Comma(,)@','
        '_ Minus(-)@'-'
        '_ Dot(.)@'.'
        '_ Slash(/)@'/'
        '_ Colon(:)@':'
        '_ Semi(;)@';'
        '_ Lt(<)@'<'
        '_ Eq(=)@'='
        '_ Gt(>)@'>'
        '_ Question(?)@'?'
        '_ Commat(@)@'@'
        '_ Backslash@'\\'
        '_ Caret(^)@'^'
        '_ Underscore(_)@'_'
        '_ Grave@'`'
        '_ Or(|)@'|'
        '_ Tilde(~)@'~'
        '_ OpenParen@'('
        '_ CloseParen@')'
        '_ OpenBrace@'{'
        '_ CloseBrace@'}'
        '_ OpenBracket@'['
        '_ CloseBracket@']'
    );

    /// Represents '<'
    pub type OpenAngle = Lt;
    #[allow(non_upper_case_globals)]
    /// Represents '<'
    pub const OpenAngle: OpenAngle = Lt;
    /// Represents '>'
    pub type CloseAngle = Gt;
    #[allow(non_upper_case_globals)]
    /// Represents '>'
    pub const CloseAngle: CloseAngle = Gt;

    /// Emit a type-level char from given token or char literal
    ///
    /// # Example
    ///
    /// ```
    /// # use syan::symbol::*;
    /// # use syan::symbol::chars::{Star, OpenParen, Dollar, Char};
    /// assert_eq!(Char!('*'), Star);
    /// assert_eq!(Char!(*), Star);
    /// assert_eq!(Char!('('), OpenParen);
    ///
    /// fn take_dollar(dollar: Char!['$']) {
    ///     assert_eq!(dollar, Char!($));
    /// }
    /// take_dollar(Dollar);
    /// ```
    #[doc(inline)]
    pub use __Char as Char;
}

#[doc(hidden)]
#[macro_export]
macro_rules! _Symbol {
    ($($t:tt)*) => {
        $crate::_imp::syan_macro::symbol!($crate, $($t)*)
    };
}

pub use _Symbol as Symbol;
