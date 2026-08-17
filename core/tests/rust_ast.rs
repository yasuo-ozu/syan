// Parses `proc_macro2` tokens throughout; the whole suite is skipped without the optional
// dependency that provides them.
#![cfg(feature = "proc_macro2")]

use syan::nested::group::{GroupBrace, GroupParen};
use syan::parse::{Parse, Unparse};
use syan::source::proc_macro2::literal::Integer;
use syan::symbol::Token;

type DefaultExpr<S> = Expr<S, Expr<S, Expr<S, ExpressionTerm>>>;

#[derive(Parse, Unparse)]
pub enum Expr<S, Expr0 = DefaultExpr<S>> {
    Term(Term<S, Expr0>),
    Binary {
        left: Box<Term<S, Expr0>>,
        op: Token![S => +],
        right: Box<Expr0>,
    },
}

#[derive(Parse, Unparse)]
pub enum Term<S, Expr0 = DefaultExpr<S>> {
    Literal(Integer),
    Ident(Ident),
    Block(Block<S, Expr0>),
}

#[derive(Parse, Unparse)]
pub struct Ident {
    pub name: Integer,
}

#[derive(Parse, Unparse)]
pub enum Item<S, Expr0 = DefaultExpr<S>> {
    Fn(ItemFn<S, Expr0>),
    Mod(ItemMod<S>),
    Trait(ItemTrait<S>),
    Impl(ItemImpl<S>),
}

#[derive(Parse, Unparse)]
pub struct ItemFn<S, Expr0 = DefaultExpr<S>> {
    pub fn_token: Token![S => fn],
    pub name: Ident,
    pub paren_group: GroupParen<(), S>,
    pub body: Block<S, Expr0>,
}

#[derive(Parse, Unparse)]
pub struct ItemMod<S> {
    pub mod_token: Token![S => mod],
    pub name: Ident,
    pub brace_group: GroupBrace<(), S>,
    #[group(self.brace_group)]
    pub content: Integer,
}

#[derive(Parse, Unparse)]
pub struct ItemTrait<S> {
    pub trait_token: Token![S => trait],
    pub name: Ident,
    pub brace_group: GroupBrace<(), S>,
    #[group(self.brace_group)]
    pub content: Integer,
}

#[derive(Parse, Unparse)]
pub struct ItemImpl<S> {
    pub impl_token: Token![S => impl],
    pub name: Ident,
    pub for_token: Token![S => for],
    pub target: Ident,
    pub brace_group: GroupBrace<(), S>,
    #[group(self.brace_group)]
    pub content: Integer,
}

#[derive(Parse, Unparse)]
pub enum Stmt<S, Expr0 = DefaultExpr<S>> {
    Let {
        let_token: Token![S => let],
        name: Ident,
        eq_token: Token![S => =],
        value: Box<Expr0>,
        semicolon: Token![S => ;],
    },
    Expr {
        expr: Box<Expr0>,
        semicolon: Option<Token![S => ;]>,
    },
}

#[derive(Parse, Unparse)]
pub struct Block<S, Expr0 = DefaultExpr<S>> {
    pub brace_group: GroupBrace<(), S>,
    #[group(self.brace_group)]
    pub stmts: Vec<Stmt<S, Expr0>>,
}

pub struct ExpressionTerm;
impl<Atom: syan::span::Spanned> Parse<Atom> for ExpressionTerm
where
    Atom: syan::span::Spanned,
{
    type Error = syan::error::ParseError<syan::span::SpanOf<Atom>>;
    fn parse_stream<__S: syan::parse::ParseStream<Atom = Atom>>(
        _stream: &mut __S,
    ) -> Result<Self, Self::Error> {
        Err(syan::error::ParseError::other(Default::default(), "expression recursion limit"))
    }
}

impl<Atom> Unparse<Atom> for ExpressionTerm {
    fn unparse<Emitter: syan::parse::unparse::Emitter<Atom>>(
        &self,
        _sink: &mut Emitter,
    ) -> Result<(), Emitter::Error> {
        panic!()
    }
}

use template_quote::quote;

#[test]
fn test_simple_module() {
    let tokens = quote! {
        mod 100 {
            200
        }
    };
    let module: ItemMod<_> = Parse::parse(tokens).unwrap();
    assert_eq!(module.name.name.value, "100");
    assert_eq!(module.content.value, "200");
}

#[test]
fn test_trait_item() {
    let tokens = quote! {
        trait 123 {
            456
        }
    };
    let trait_item: ItemTrait<_> = Parse::parse(tokens).unwrap();
    assert_eq!(trait_item.name.name.value, "123");
    assert_eq!(trait_item.content.value, "456");
}

#[test]
fn test_impl_item() {
    let tokens = quote! {
        impl 111 for 222 {
            333
        }
    };
    let impl_item: ItemImpl<_> = Parse::parse(tokens).unwrap();
    assert_eq!(impl_item.name.name.value, "111");
    assert_eq!(impl_item.target.name.value, "222");
    assert_eq!(impl_item.content.value, "333");
}

#[test]
fn test_item_function() {
    let tokens = quote! {
        fn 42() {
            1;
        }
    };
    let item: Item<_> = Parse::parse(tokens).unwrap();
    match item {
        Item::Fn(func) => assert_eq!(func.name.name.value, "42"),
        _ => panic!("Expected function item"),
    }
}

#[test]
fn test_expression_literal() {
    let tokens = quote! { 123 };
    let expr: Expr<_> = Parse::parse(tokens).unwrap();
    match expr {
        Expr::Term(Term::Literal(lit)) => assert_eq!(lit.value, "123"),
        _ => panic!("Expected literal expression"),
    }
}

#[test]
fn test_expression_block() {
    let tokens = quote! {
        {
            let 1 = 2;
        }
    };
    let expr: Expr<_> = Parse::parse(tokens).unwrap();
    match expr {
        Expr::Term(Term::Block(block)) => assert_eq!(block.stmts.len(), 1),
        _ => panic!("Expected block expression"),
    }
}
