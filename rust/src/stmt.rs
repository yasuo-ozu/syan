use syan::span::{Span, Spanned};
use crate::{Expr, Pat, Type, token::*};

/// A Rust statement
#[derive(Debug, Clone)]
pub enum Stmt<S: Span> {
    Local(Local<S>),
    Item(crate::Item<S>),
    Expr(Expr<S>),
    Semi(Expr<S>, SemiToken<S>),
}

impl<S: Span> Spanned for Stmt<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        match self {
            Stmt::Local(local) => local.span(),
            Stmt::Item(item) => item.span(),
            Stmt::Expr(expr) => expr.span(),
            Stmt::Semi(expr, semi) => expr.span().migrate(semi.span()),
        }
    }
}

/// Local variable declaration (let statement)
#[derive(Debug, Clone)]
pub struct Local<S: Span> {
    pub let_token: LetToken<S>,
    pub pat: Pat<S>,
    pub ty: Option<(ColonToken<S>, Type<S>)>,
    pub init: Option<(EqToken<S>, Expr<S>)>,
    pub semi_token: SemiToken<S>,
}

impl<S: Span> Spanned for Local<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.let_token.span().migrate(self.semi_token.span())
    }
}