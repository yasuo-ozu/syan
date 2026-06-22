use syan::{
    parse::{Parse, Unparse},
    span::WithSpan,
    symbol::Token,
};
use type_macro_derive_tricks::macro_derive;

/// Identifier
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct Ident<S> {
    pub name: WithSpan<String, S>,
    pub span: S,
}

#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum Safety<S> {
    Safe(Token![S => safe]),
    Unsafe(Token![S => unsafe]),
    Default,
}
