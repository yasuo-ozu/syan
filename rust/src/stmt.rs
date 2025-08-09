use crate::{Expr, Pat, Type};
use syan::{
    parse::{Parse, Unparse},
    symbol::Token,
};
use type_macro_derive_tricks::macro_derive;

/// A Rust statement
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum Stmt<S, Tokens = std::convert::Infallible> {
    Local(Local<S, Tokens>),
    Item(crate::Item<S, Tokens>),
    // Expr(Expr<S, Tokens>),
    // Semi(Expr<S, Tokens>, Token![S => ;]),
    // MacCall(StmtMacCall<S>),
    Empty,
}

/// Local variable declaration (let statement)  
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct Local<S, Tokens = std::convert::Infallible> {
    pub attrs: Vec<crate::Attribute<S>>,
    pub let_token: Token![S => let],
    pub pat: Pat<S>,
    pub colon_token: Option<Token![S => :]>,
    pub ty: Option<Type<S>>,
    pub kind: LocalKind<S, Tokens>,
    pub semi_token: Token![S => ;],
}

/// Local variable kind
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum LocalKind<S, Tokens = std::convert::Infallible> {
    Init(Token![S => =], Expr<S, Tokens>),
    InitElse(
        Token![S => =],
        // Expr<S, Tokens>,
        // Token![S => else],
        // crate::expr::Block<S>,
    ),
    Decl,
}

/// Statement-level macro call
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct StmtMacCall<S> {
    pub mac: crate::expr::Macro<S>,
    pub style: MacStmtStyle<S>,
}

/// Macro statement style
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum MacStmtStyle<S> {
    Semicolon(Token![S => ;]),
    Braces,
    NoBraces,
}
