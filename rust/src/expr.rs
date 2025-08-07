use syan::{
    parse::{Parse, ParseStream, Unparse, IntoParseStream},
    span::{Span, Spanned},
};
use crate::{Type, Pat, Path, token::*};

/// A Rust expression
#[derive(Debug, Clone)]
pub enum Expr<S: Span> {
    Binary(ExprBinary<S>),
    Unary(ExprUnary<S>),
    Call(ExprCall<S>),
    MethodCall(ExprMethodCall<S>),
    Path(ExprPath<S>),
    Lit(ExprLit<S>),
    Block(ExprBlock<S>),
    If(ExprIf<S>),
    Match(ExprMatch<S>),
    Loop(ExprLoop<S>),
    While(ExprWhile<S>),
    For(ExprFor<S>),
    Return(ExprReturn<S>),
    Break(ExprBreak<S>),
    Continue(ExprContinue<S>),
    Paren(ExprParen<S>),
    Index(ExprIndex<S>),
    Field(ExprField<S>),
    Reference(ExprReference<S>),
    Array(ExprArray<S>),
    Tuple(ExprTuple<S>),
    Struct(ExprStruct<S>),
    Closure(ExprClosure<S>),
    Async(ExprAsync<S>),
    Await(ExprAwait<S>),
    Try(ExprTry<S>),
    Assign(ExprAssign<S>),
    AssignOp(ExprAssignOp<S>),
    Range(ExprRange<S>),
    Cast(ExprCast<S>),
    Type(ExprType<S>),
    Let(ExprLet<S>),
    Macro(ExprMacro<S>),
    Unsafe(ExprUnsafe<S>),
}

impl<S: Span> Spanned for Expr<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        match self {
            Expr::Binary(e) => e.span(),
            Expr::Unary(e) => e.span(),
            Expr::Call(e) => e.span(),
            Expr::MethodCall(e) => e.span(),
            Expr::Path(e) => e.span(),
            Expr::Lit(e) => e.span(),
            Expr::Block(e) => e.span(),
            Expr::If(e) => e.span(),
            Expr::Match(e) => e.span(),
            Expr::Loop(e) => e.span(),
            Expr::While(e) => e.span(),
            Expr::For(e) => e.span(),
            Expr::Return(e) => e.span(),
            Expr::Break(e) => e.span(),
            Expr::Continue(e) => e.span(),
            Expr::Paren(e) => e.span(),
            Expr::Index(e) => e.span(),
            Expr::Field(e) => e.span(),
            Expr::Reference(e) => e.span(),
            Expr::Array(e) => e.span(),
            Expr::Tuple(e) => e.span(),
            Expr::Struct(e) => e.span(),
            Expr::Closure(e) => e.span(),
            Expr::Async(e) => e.span(),
            Expr::Await(e) => e.span(),
            Expr::Try(e) => e.span(),
            Expr::Assign(e) => e.span(),
            Expr::AssignOp(e) => e.span(),
            Expr::Range(e) => e.span(),
            Expr::Cast(e) => e.span(),
            Expr::Type(e) => e.span(),
            Expr::Let(e) => e.span(),
            Expr::Macro(e) => e.span(),
            Expr::Unsafe(e) => e.span(),
        }
    }
}

/// Binary expression
#[derive(Debug, Clone)]
pub struct ExprBinary<S: Span> {
    pub left: Box<Expr<S>>,
    pub op: BinOp<S>,
    pub right: Box<Expr<S>>,
}

impl<S: Span> Spanned for ExprBinary<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.left.span().migrate(self.right.span())
    }
}

/// Binary operator
#[derive(Debug, Clone)]
pub enum BinOp<S: Span> {
    Add(PlusToken<S>),
    Sub(MinusToken<S>),
    Mul(StarToken<S>),
    Div(SlashToken<S>),
    Rem(PercentToken<S>),
    And(AndAndToken<S>),
    Or(OrOrToken<S>),
    BitXor(CaretToken<S>),
    BitAnd(AndToken<S>),
    BitOr(OrToken<S>),
    Shl(LtLtToken<S>),
    Shr(GtGtToken<S>),
    Eq(EqEqToken<S>),
    Lt(LtToken<S>),
    Le(LeToken<S>),
    Ne(NeToken<S>),
    Ge(GeToken<S>),
    Gt(GtToken<S>),
}

impl<S: Span> Spanned for BinOp<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        match self {
            BinOp::Add(t) => t.span(),
            BinOp::Sub(t) => t.span(),
            BinOp::Mul(t) => t.span(),
            BinOp::Div(t) => t.span(),
            BinOp::Rem(t) => t.span(),
            BinOp::And(t) => t.span(),
            BinOp::Or(t) => t.span(),
            BinOp::BitXor(t) => t.span(),
            BinOp::BitAnd(t) => t.span(),
            BinOp::BitOr(t) => t.span(),
            BinOp::Shl(t) => t.span(),
            BinOp::Shr(t) => t.span(),
            BinOp::Eq(t) => t.span(),
            BinOp::Lt(t) => t.span(),
            BinOp::Le(t) => t.span(),
            BinOp::Ne(t) => t.span(),
            BinOp::Ge(t) => t.span(),
            BinOp::Gt(t) => t.span(),
        }
    }
}

// Placeholder implementations for other expression types
macro_rules! define_expr_stub {
    ($name:ident) => {
        #[derive(Debug, Clone)]
        pub struct $name<S: Span> {
            pub span: S,
        }
        
        impl<S: Span> Spanned for $name<S> {
            type Span = S;
            
            fn span(&self) -> Self::Span {
                self.span.clone()
            }
        }
    };
}

define_expr_stub!(ExprUnary);
define_expr_stub!(ExprCall);
define_expr_stub!(ExprMethodCall);
define_expr_stub!(ExprPath);
define_expr_stub!(ExprLit);
define_expr_stub!(ExprBlock);
define_expr_stub!(ExprIf);
define_expr_stub!(ExprMatch);
define_expr_stub!(ExprLoop);
define_expr_stub!(ExprWhile);
define_expr_stub!(ExprFor);
define_expr_stub!(ExprReturn);
define_expr_stub!(ExprBreak);
define_expr_stub!(ExprContinue);
define_expr_stub!(ExprParen);
define_expr_stub!(ExprIndex);
define_expr_stub!(ExprField);
define_expr_stub!(ExprReference);
define_expr_stub!(ExprArray);
define_expr_stub!(ExprTuple);
define_expr_stub!(ExprStruct);
define_expr_stub!(ExprClosure);
define_expr_stub!(ExprAsync);
define_expr_stub!(ExprAwait);
define_expr_stub!(ExprTry);
define_expr_stub!(ExprAssign);
define_expr_stub!(ExprAssignOp);
define_expr_stub!(ExprRange);
define_expr_stub!(ExprCast);
define_expr_stub!(ExprType);
define_expr_stub!(ExprLet);
define_expr_stub!(ExprMacro);
define_expr_stub!(ExprUnsafe);