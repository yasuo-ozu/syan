use syan::{
    nested::group::GroupParen,
    parse::{Parse, Unparse},
    symbol::Token,
};
use type_macro_derive_tricks::macro_derive;

/// Visibility modifier
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum Visibility<S> {
    Public(VisPublic<S>),
    Restricted(VisRestricted<S>),
    Inherited,
}

/// Public visibility
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct VisPublic<S> {
    pub pub_token: Token![S => pub],
}

/// Restricted visibility like `pub(crate)` or `pub(in path)`
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct VisRestricted<S> {
    pub pub_token: Token![S => pub],
    pub paren_token: GroupParen<(), S>,
    pub restriction: VisRestriction<S>,
}

/// Restriction in visibility
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum VisRestriction<S> {
    Crate(Token![S => crate]),
    Super(Token![S => super]),
    SelfValue(Token![S => self]),
    In(Token![S => in], Box<crate::Path<S>>),
}