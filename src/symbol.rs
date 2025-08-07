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
        '_ Space@' '
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

mod imp {
    #[doc(hidden)]
    pub enum _Symbol<T> {
        /// The symbol instance variant.
        ///
        /// This is the only variant that can be constructed and represents
        /// a runtime instance of the type-level symbol `T`.
        Symbol,

        /// Unreachable phantom variant for type parameter storage.
        ///
        /// This variant cannot be constructed due to the [`Infallible`] field
        /// and exists only to maintain the type parameter `T` in the enum definition.
        ///
        /// [`Infallible`]: core::convert::Infallible
        _Phantom(core::marker::PhantomData<T>, core::convert::Infallible),
    }

    /// Convenience re-export of the `Symbol` variant.
    ///
    /// This allows using `Symbol` directly instead of `_Symbol::Symbol`
    /// when importing from the `imp` module.
    pub use _Symbol::Symbol;

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
}

/// A wrapper enum for type-level symbols that provides runtime behavior.
///
/// `_Symbol<T>` is a zero-sized wrapper around type-level symbol representations
/// (typically [`Joint`] types containing character encodings). It enables runtime
/// instantiation and formatting of compile-time symbol types.
///
/// [`Joint`]: crate::nested::Joint
///
/// # Variants
///
/// - `Symbol` - The only meaningful variant, representing a symbol instance
/// - `_Phantom` - Unreachable variant used for type parameter storage
///
/// # Type Parameter
///
/// - `T` - The underlying type-level symbol representation, usually a `Joint<Tuple>`
///   containing character types from the [`chars`] module
///
/// [`chars`]: crate::symbol::chars
///
/// # Usage
///
/// This type is typically not used directly. Instead, use the [`Symbol!`] macro
/// which generates `_Symbol<Joint<...>>` types automatically.
///
/// [`Symbol!`]: crate::symbol::Symbol
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
///
/// # Traits
///
/// - [`Default`] - Always returns the `Symbol` variant
/// - [`Display`] - Delegates to `T::default().fmt()` when `T: Default + Display`  
/// - [`Debug`] - Delegates to `T::default().fmt()` when `T: Default + Debug`
///
/// # Implementation Details
///
/// The `_Phantom` variant is unreachable and exists only to store the type parameter.
/// All instances are created through `Default::default()` which returns the `Symbol` variant.
/// The formatting traits delegate to the underlying type's default instance, enabling
/// runtime inspection of compile-time symbol representations.
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

/// Create a type-level symbol from an identifier.
///
/// This macro converts Rust identifiers into compile-time type representations
/// using the `Joint<Tuple>` structure. Each character of the identifier is
/// encoded as a corresponding type from the [`chars`] module.
///
/// # Syntax
///
/// ```text
/// Symbol!(identifier)
/// ```
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
///
/// # Type Structure
///
/// The resulting type is always of the form `syan::nested::Joint<Tuple>` where
/// `Tuple` contains the character type representations. For long identifiers,
/// nested `Joint` structures are used automatically.
///
/// # Traits
///
/// The generated symbol types implement:
/// - [`Default`] - Create instances with `Default::default()`
/// - [`Debug`] - Debug formatting shows character representations
/// - [`Clone`], [`Copy`] - Standard derivable traits
///
/// # Implementation Details
///
/// - Uses recursive chunking for identifiers longer than 14 characters
/// - Leverages the [`newer_type`] crate for trait implementations
/// - Character mapping is handled by the `chars` module
/// - Proc-macro implementation in `syan_macro::symbol`
#[doc(inline)]
pub use crate::_Symbol as Symbol;
