use crate::{tokens::*, Type};
use syan::{
    nested::group::GroupParen,
    parse::{Parse, Unparse},
    symbol::Token,
};
use type_macro_derive_tricks::macro_derive;

/// A path like `std::collections::HashMap`
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct Path<S, Tokens = std::convert::Infallible> {
    pub leading_colon: Option<Token![S => ::]>,
    pub segments: Vec<PathSegment<S, Tokens>>,
}

/// A segment of a path like `HashMap` in `std::collections::HashMap`
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PathSegment<S, Tokens = std::convert::Infallible> {
    pub ident: Ident<S>,
    pub arguments: Option<PathArguments<S, Tokens>>,
}

/// Path arguments like `<T, U>` in `HashMap<T, U>`
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum PathArguments<S, Tokens = std::convert::Infallible> {
    None,
    AngleBracketed(AngleBracketedGenericArguments<S, Tokens>),
    Parenthesized(ParenthesizedGenericArguments<S>),
}

/// Angle-bracketed generic arguments `<T, U>`
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct AngleBracketedGenericArguments<S, Tokens = std::convert::Infallible> {
    pub lt_token: Token![S => <],
    pub args: Vec<crate::GenericArgument<S, Tokens>>,
    pub gt_token: Token![S => >],
}

/// Parenthesized generic arguments `(A, B) -> C`
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ParenthesizedGenericArguments<S> {
    pub paren_token: GroupParen<(), S>,
    pub inputs: Vec<Type<S>>,
    pub output: Option<(Token![S => ->], Type<S>)>,
}

/// Lifetime like `'a`
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct Lifetime<S> {
    pub apostrophe: Token![S => '\''],
    pub ident: Ident<S>,
}

// Placeholder implementations
macro_rules! define_path_stub {
    ($name:ident) => {
        #[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
        pub struct $name<S> {
            pub span: S,
        }
    };
}

define_path_stub!(Binding);
define_path_stub!(Constraint);
