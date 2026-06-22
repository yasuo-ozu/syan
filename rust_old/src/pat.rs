use crate::{tokens::*, Path};
use syan::{
    nested::group::{GroupBrace, GroupBracket, GroupParen},
    parse::{Parse, Unparse},
    symbol::Token,
};
use type_macro_derive_tricks::macro_derive;

/// A Rust pattern
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum Pat<S, Tokens = std::convert::Infallible> {
    Ident(PatIdent<S>),
    Struct(PatStruct<S>),
    TupleStruct(PatTupleStruct<S>),
    Path(PatPath<S>),
    Tuple(PatTuple<S>),
    Box(PatBox<S>),
    Ref(PatRef<S>),
    // Lit(PatLit<S, Tokens>),
    Range(PatRange<S, Tokens>),
    Slice(PatSlice<S>),
    Rest(PatRest<S>),
    Paren(PatParen<S>),
    Wild(Token![S => _]),
    Macro(PatMacro<S>),
    Or(PatOr<S>),
    // Additional patterns from rustc_ast
    Never(PatNever<S>),
    Err(PatErr<S>),
}

/// Identifier pattern with optional binding mode and subpattern
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PatIdent<S> {
    pub by_ref: Option<Token![S => ref]>,
    pub mutability: Option<Token![S => mut]>,
    pub ident: Ident<S>,
    pub subpat: Option<(Token![S => @], Box<Pat<S>>)>,
}

/// Struct pattern
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PatStruct<S> {
    pub path: Path<S>,
    pub brace_token: GroupBrace<(), S>,
    pub fields: Vec<FieldPat<S>>,
    pub dot2_token: Option<Token![S => ..]>,
}

/// Field pattern in struct pattern
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct FieldPat<S> {
    pub member: PatMember<S>,
    pub colon_token: Option<Token![S => :]>,
    pub pat: Box<Pat<S>>,
}

/// Pattern struct field member (identifier or index)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum PatMember<S> {
    Named(Ident<S>),
    Unnamed(syan::span::WithSpan<syan::source::proc_macro2::literal::Integer, S>),
}


/// Tuple struct pattern (Foo(a, b, c))
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PatTupleStruct<S> {
    pub path: crate::Path<S>,
    pub paren_token: GroupParen<(), S>,
    pub elems: Vec<crate::Pat<S>>,
}

/// Path pattern
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PatPath<S> {
    pub qself: Option<crate::expr::QSelf<S>>,
    pub path: crate::Path<S>,
}

/// Tuple pattern (a, b, c)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PatTuple<S> {
    pub paren_token: GroupParen<(), S>,
    pub elems: Vec<crate::Pat<S>>,
}

/// Box pattern (box pat)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PatBox<S> {
    pub box_token: Token![S => box],
    pub pat: Box<crate::Pat<S>>,
}

/// Reference pattern (&pat or &mut pat)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PatRef<S> {
    pub and_token: Token![S => &],
    pub mutability: Option<Token![S => mut]>,
    pub pat: Box<crate::Pat<S>>,
}

/// Literal pattern (42, "hello", true)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PatLit<S, Tokens = std::convert::Infallible> {
    pub expr: Box<crate::Expr<S, Tokens>>,
}
/// Range pattern (1..=10, ..=10, 1..)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PatRange<S, Tokens = std::convert::Infallible> {
    pub lo: Option<Box<crate::Expr<S, Tokens>>>,
    pub limits: crate::expr::RangeLimits<S>,
    pub hi: Option<Box<crate::Expr<S, Tokens>>>,
}

/// Slice pattern ([a, b, ..rest])
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PatSlice<S> {
    pub bracket_token: GroupBracket<(), S>,
    pub elems: Vec<crate::Pat<S>>,
}

/// Rest pattern (..)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PatRest<S> {
    pub dot2_token: Token![S => ..],
}

/// Parenthesized pattern
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PatParen<S> {
    pub paren_token: GroupParen<(), S>,
    pub pat: Box<crate::Pat<S>>,
}

/// Macro pattern
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PatMacro<S> {
    pub mac: crate::expr::Macro<S>,
}

/// Or pattern (A | B | C)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PatOr<S> {
    pub cases: Vec<crate::Pat<S>>,
}

/// Never pattern (!)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PatNever<S> {
    pub bang_token: Token![S => !],
}

/// Error pattern (for error recovery)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PatErr<S> {
    pub span: S,
}

/// Binding annotation (ref/mut combinations)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct BindingAnnotation<S> {
    pub by_ref: ByRef<S>,
    pub mutbl: PatMutability<S>,
}

/// Reference binding
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum ByRef<S> {
    Yes(Token![S => ref]),
    No,
}

/// Pattern mutability
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum PatMutability<S> {
    Mut(Token![S => mut]),
    Not,
}

/// Range end syntax
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum RangeEnd<S> {
    Included(Token![S => ..=]),
    Excluded(Token![S => ..]),
}

/// Range syntax  
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum RangeSyntax<S> {
    DotDotDot(Token![S => ...]),
    DotDotEq(Token![S => ..=]),
}
