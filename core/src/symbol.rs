//! Type-level symbols: keywords, identifiers and punctuation encoded as zero-sized types.
//!
//! Use [`Symbol!`](macro@crate::symbol::Symbol) to name one, [`Token!`](macro@crate::symbol::Token)
//! to pair it with a span, and [`chars::Char!`](macro@crate::symbol::chars::Char) for a single
//! character.

/// One zero-sized type per source character: `_a`, `_Z`, `_0`, `Star`, `Semi`, and so on.
///
/// These are the building blocks a [`Symbol!`](macro@crate::symbol::Symbol) expands to; name one
/// directly with the [`Char!`](macro@crate::symbol::chars::Char) macro.
pub mod chars {
    /// Marker that every single-character symbol parses from the atom type `Atom`.
    ///
    /// # Safety
    ///
    /// This trait is `unsafe` because downstream code relies on the marker holding for *all* generated
    /// `char` symbol types; it is implemented only by the blanket impl in this crate (gated on each
    /// `char` type being `Parse<Atom>`), never by hand. Implementing it manually could assert the
    /// invariant for an `Atom` that cannot in fact parse every symbol.
    pub unsafe trait AtomParsedToAllChars {}

    macro_rules! impl_char {
        ((@add_doc $name:ident $(($token:tt))?  $char:literal)) => {
            #[doc(hidden)]
            #[allow(non_camel_case_types)]
            #[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
            #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
            #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

            unsafe impl<Atom: crate::span::Spanned> AtomParsedToAllChars for Atom
            where
                $($name: crate::parse::Parse<Atom>,)*
            {}

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
        '_ Space@' '
    );

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

mod imp {
    #[doc(hidden)]
    #[derive(Copy, Clone, PartialEq, Eq, Hash)]
    pub enum _Symbol<T> {
        Symbol,

        /// Holds `T`; uninhabited via the `Infallible` field, so it can never be constructed.
        _Phantom(core::marker::PhantomData<T>, core::convert::Infallible),
    }

    pub use _Symbol::Symbol;

    // Deliberately hand-written, not derived: `#[derive(Default)]` would add a `T: Default` bound, but
    // `_Symbol<T>` defaults to its fieldless `Symbol` variant for *any* `T`.
    #[allow(clippy::derivable_impls)]
    impl<T> Default for _Symbol<T> {
        fn default() -> Self {
            _Symbol::Symbol
        }
    }

    impl<T: Default + core::fmt::Display> core::fmt::Display for _Symbol<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            T::default().fmt(f)
        }
    }

    impl<T: Default + core::fmt::Debug> core::fmt::Debug for _Symbol<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            T::default().fmt(f)
        }
    }

    /// The symbol is a ZST: `T` spells its characters at the type level and carries no data, so it
    /// is written as unit and read back as itself. Deliberately no `T: Serialize` bound -- requiring
    /// one would force every `chars::*` marker to be serializable for no gain.
    #[cfg(feature = "serde")]
    impl<T> serde::Serialize for _Symbol<T> {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_unit()
        }
    }

    #[cfg(feature = "serde")]
    impl<'de, T> serde::Deserialize<'de> for _Symbol<T> {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            <() as serde::Deserialize>::deserialize(deserializer)?;
            Ok(_Symbol::Symbol)
        }
    }

    impl<Atom, T> crate::parse::Parse<Atom> for _Symbol<T>
    where
        Atom: crate::span::Spanned + super::chars::AtomParsedToAllChars,
        T: crate::parse::Parse<Atom>,
    {
        type Error = T::Error;

        fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = Atom>>(
            stream: &mut __S,
        ) -> Result<Self, Self::Error> {
            T::parse_stream(&mut *stream)?;
            Ok(Self::Symbol)
        }
    }

    impl<Atom, T> crate::parse::Unparse<Atom> for _Symbol<T>
    where
        T: Default + core::fmt::Display,
        Atom: From<String> + super::chars::AtomParsedToAllChars,
    {
        fn unparse<S: crate::parse::unparse::Emitter<Atom>>(
            &self,
            sink: &mut S,
        ) -> Result<(), S::Error> {
            let symbol_str = T::default().to_string();
            let atom = Atom::from(symbol_str);
            sink.write_one(atom)
        }
    }
}

/// A wrapper enum for type-level symbols that provides runtime behavior.
///
/// `_Symbol<T>` is a zero-sized wrapper around type-level symbol representations
/// (typically [`Joint`] types containing character encodings). It enables runtime
/// instantiation and formatting of compile-time symbol types.
///
/// [`Joint`]: struct@crate::nested::Joint
///
/// # Usage
///
/// This type is typically not used directly. Instead, use the [`Symbol!`] macro
/// which generates `_Symbol<Joint<...>>` types automatically.
///
/// [`Symbol!`]: macro@crate::symbol::Symbol
///
/// # Examples
///
/// ```
/// # use syan::symbol::Symbol;
/// // Create symbol instances with Default
/// let hello: Symbol!(hello) = Default::default();
/// let world: Symbol!(world) = Default::default();
///
/// // Debug formatting shows the underlying character encoding
/// println!("{:?}", hello); // Output: Symbol
/// ```
#[doc(inline)]
pub use imp::_Symbol as Symbol;
pub use imp::*;

#[doc(hidden)]
#[macro_export]
macro_rules! _Symbol {
    ($($t:tt)*) => {
        $crate::_imp::syan_macro::symbol!($crate, $($t)*)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! _Token {
    ($s:ty => $($t:tt)*) => {
        $crate::span::WithSpan<$crate::_imp::syan_macro::symbol!($crate, $($t)*), $s>
    };
}

/// Create a type-level symbol from an identifier.
///
/// This macro converts Rust identifiers into compile-time type representations
/// using the `Joint<Tuple>` structure. Each character of the identifier is
/// encoded as a corresponding type from the [`chars`] module.
///
/// # Examples
///
/// ## Basic Usage
///
/// ```
/// # use syan::symbol::Symbol;
/// // Create symbol types
/// type Hello = Symbol!(hello);
/// type World = Symbol!(world);
///
/// // Create instances with Default
/// let hello_symbol: Hello = Default::default();
/// let world_symbol: World = Default::default();
///
/// // Debug formatting shows the encoded characters
/// println!("{:?}", hello_symbol); // Output: (_h, _e, _l, _l, _o)
/// ```
///
/// ## Long Identifiers
///
/// The macro automatically handles identifiers longer than 14 characters by
/// using recursive `Joint` nesting:
///
/// ```
/// # use syan::symbol::Symbol;
/// // Short identifiers use simple tuples
/// type Short = Symbol!(hello);  // Joint<(_h, _e, _l, _l, _o)>
///
/// // Long identifiers use recursive nesting
/// type Long = Symbol!(very_long_identifier_name);
/// // Joint<(Joint<(first_14_chars...)>, Joint<(remaining_chars...)>)>
///
/// let long_symbol: Long = Default::default();
/// ```
///
/// ## Character Encoding
///
/// Each character is mapped to a corresponding type:
/// - `a-z` → `_a`, `_b`, ..., `_z`
/// - `A-Z` → `_A`, `_B`, ..., `_Z`
/// - `0-9` → `_0`, `_1`, ..., `_9`
/// - `_` → `__`
///
/// ```
/// # use syan::symbol::Symbol;
/// type Example = Symbol!(test_123);
/// // Encodes as: Joint<(_t, _e, _s, _t, __, _1, _2, _3)>
/// ```
/// # Deriving on a node that uses it
///
/// A type macro in a field position blocks the *built-in* derives — rustc rejects `#[derive(Debug)]`
/// (or `Clone`, `PartialEq`, …) on an item containing one, with "`derive` cannot be used on items
/// with type macros". Proc-macro derives such as [`Parse`](macro@crate::parse::Parse) are unaffected.
///
/// With the `serde` feature, `Serialize`/`Deserialize` also derive fine, but serde cannot see
/// through the macro to infer bounds — see [`Token!`](macro@crate::symbol::Token) for the
/// `#[serde(bound(..))]` this needs.
#[doc(inline)]
pub use crate::_Symbol as Symbol;

/// A [`Symbol!`] that remembers where it was matched.
///
/// `Token![S => x]` is [`WithSpan`](crate::span::WithSpan)`<Symbol!(x), S>`, so it parses exactly
/// what `Symbol!(x)` parses and additionally stores a span of type `S`. Use it when a node needs to
/// report a position; use [`Symbol!`] when it does not.
///
/// The span type comes first, before `=>`. Leaving it a type parameter keeps the node reusable
/// across sources, since each source brings its own span type.
///
/// ```
/// # use syan::parse::Parse;
/// # use syan::symbol::Token;
/// type Span = syan::source::string::Span;
///
/// #[derive(Parse)]
/// struct Arrow {
///     minus: Token![Span => -],
///     gt: Token![Span => >],
/// }
///
/// let a: Arrow = Parse::parse("->").unwrap();
/// assert_eq!((a.minus.span.line, a.minus.span.col), (1, 1));
/// assert_eq!((a.gt.span.line, a.gt.span.col), (1, 2));
/// ```
///
/// The token is spelled the same way as in [`Symbol!`]: an identifier, a punctuation character, a
/// number, or a character literal.
///
/// # Deriving on a node that uses it
///
/// Like [`Symbol!`], this is a type macro, so the *built-in* derives cannot be used on a struct that
/// names it in a field — rustc rejects `#[derive(Debug)]` on an item containing a type macro.
/// `#[derive(Parse)]` and other proc-macro derives are unaffected.
///
/// Two ways round it. Route the built-in derives through an attribute macro, which expands the type
/// macro first — [`type-macro-derive-tricks`](https://docs.rs/type-macro-derive-tricks) spells this
/// `#[macro_derive(..)]`:
///
/// ```
/// # use syan::parse::Parse;
/// # use syan::symbol::Token;
/// # use type_macro_derive_tricks::macro_derive;
/// # type Span = syan::source::string::Span;
/// #[macro_derive(Parse, Debug, Clone, PartialEq)]
/// struct Assign<S> {
///     eq: Token![S => =],
///     n: syan::literal::Integer,
/// }
///
/// let a: Assign<Span> = Parse::parse("= 1").unwrap();
/// assert_eq!(a, a.clone());
/// ```
///
/// Or write the type out — `WithSpan<chars::Eq, S>` is a plain type and derives normally, at the
/// cost of the readable form:
///
/// ```
/// # use syan::parse::Parse;
/// # use syan::span::WithSpan;
/// # use syan::symbol::chars;
/// # type Span = syan::source::string::Span;
/// #[derive(Parse, Debug, Clone, PartialEq)]
/// struct Assign<S> {
///     eq: WithSpan<chars::Eq, S>,
///     n: syan::literal::Integer,
/// }
/// ```
///
/// With the `serde` feature, serde's derive works but cannot infer bounds through the macro, so the
/// bound must be spelled out. An explicit `#[serde(bound(..))]` *replaces* serde's inference rather
/// than adding to it, so name **every** type parameter the node has — a second parameter left out
/// fails with `the trait bound `P: Serialize` is not satisfied`, whose note about a missing feature
/// flag sends you looking in the wrong crate:
///
/// ```
/// # #[cfg(feature = "serde")] {
/// # use serde::{Deserialize, Serialize};
/// # use syan::literal::Integer;
/// # use syan::parse::Parse;
/// # use syan::symbol::Token;
/// #[derive(Parse, Serialize, Deserialize)]
/// #[serde(bound(serialize = "S: Serialize", deserialize = "S: Deserialize<'de>"))]
/// struct Assign<S> {
///     name: Token![S => x],
///     eq: Token![S => =],
///     value: Integer,
/// }
/// # }
/// ```
///
/// [`Symbol!`]: macro@crate::symbol::Symbol
#[doc(inline)]
pub use crate::_Token as Token;
