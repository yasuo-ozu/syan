use crate::ast::*;
use crate::parse::HaskellSpan;
use proc_macro2::{Delimiter, Group, Ident, Punct, Spacing, Span, TokenStream, TokenTree};
use syan::span::WithSpan;

pub struct TokenStreamEmitter {
    pub tokens: Vec<TokenTree>,
}

impl TokenStreamEmitter {
    pub fn new() -> Self {
        Self { tokens: Vec::new() }
    }
    
    pub fn into_token_stream(self) -> TokenStream {
        self.tokens.into_iter().collect()
    }
}

pub trait HaskellUnparse {
    fn unparse_to(&self, emitter: &mut TokenStreamEmitter);
}

impl HaskellUnparse for WithSpan<String, HaskellSpan> {
    fn unparse_to(&self, emitter: &mut TokenStreamEmitter) {
        let ident = Ident::new(&self.slot, Span::call_site());
        emitter.tokens.push(TokenTree::Ident(ident));
    }
}

impl HaskellUnparse for WithSpan<Literal, HaskellSpan> {
    fn unparse_to(&self, emitter: &mut TokenStreamEmitter) {
        let literal = match &self.slot {
            Literal::Integer(i) => proc_macro2::Literal::i64_unsuffixed(*i),
            Literal::Float(f) => proc_macro2::Literal::f64_unsuffixed(*f),
            Literal::Char(c) => proc_macro2::Literal::character(*c),
            Literal::String(s) => proc_macro2::Literal::string(s),
        };
        emitter.tokens.push(TokenTree::Literal(literal));
    }
}

impl HaskellUnparse for WithSpan<ModuleName<HaskellSpan>, HaskellSpan> {
    fn unparse_to(&self, emitter: &mut TokenStreamEmitter) {
        self.slot.name.unparse_to(emitter);
    }
}

impl HaskellUnparse for WithSpan<Expression<HaskellSpan>, HaskellSpan> {
    fn unparse_to(&self, emitter: &mut TokenStreamEmitter) {
        match &self.slot {
            Expression::Var(name) => name.unparse_to(emitter),
            Expression::Con(name) => name.unparse_to(emitter),
            Expression::Lit(lit) => lit.unparse_to(emitter),
            Expression::App(func, arg) => {
                func.unparse_to(emitter);
                arg.unparse_to(emitter);
            }
            Expression::InfixApp(left, op, right) => {
                left.unparse_to(emitter);
                op.unparse_to(emitter);
                right.unparse_to(emitter);
            }
            Expression::Lambda(pats, body) => {
                // Use a placeholder identifier instead of backslash
                let lambda = Ident::new("lambda", Span::call_site());
                emitter.tokens.push(TokenTree::Ident(lambda));
                
                for pat in pats {
                    pat.unparse_to(emitter);
                }
                
                let arrow = Punct::new('-', Spacing::Joint);
                emitter.tokens.push(TokenTree::Punct(arrow));
                let arrow2 = Punct::new('>', Spacing::Alone);
                emitter.tokens.push(TokenTree::Punct(arrow2));
                
                body.unparse_to(emitter);
            }
            Expression::Tuple(exprs) => {
                let mut inner_tokens = Vec::new();
                for (i, expr) in exprs.iter().enumerate() {
                    let mut inner_emitter = TokenStreamEmitter::new();
                    expr.unparse_to(&mut inner_emitter);
                    inner_tokens.extend(inner_emitter.tokens);
                    
                    if i < exprs.len() - 1 {
                        inner_tokens.push(TokenTree::Punct(Punct::new(',', Spacing::Alone)));
                    }
                }
                    
                let tokens: TokenStream = inner_tokens.into_iter().collect();
                let group = Group::new(Delimiter::Parenthesis, tokens);
                emitter.tokens.push(TokenTree::Group(group));
            }
            Expression::List(exprs) => {
                let mut inner_tokens = Vec::new();
                for (i, expr) in exprs.iter().enumerate() {
                    let mut inner_emitter = TokenStreamEmitter::new();
                    expr.unparse_to(&mut inner_emitter);
                    inner_tokens.extend(inner_emitter.tokens);
                    
                    if i < exprs.len() - 1 {
                        inner_tokens.push(TokenTree::Punct(Punct::new(',', Spacing::Alone)));
                    }
                }
                    
                let tokens: TokenStream = inner_tokens.into_iter().collect();
                let group = Group::new(Delimiter::Bracket, tokens);
                emitter.tokens.push(TokenTree::Group(group));
            }
            Expression::Paren(expr) => {
                let mut inner_emitter = TokenStreamEmitter::new();
                expr.unparse_to(&mut inner_emitter);
                let tokens: TokenStream = inner_emitter.tokens.into_iter().collect();
                let group = Group::new(Delimiter::Parenthesis, tokens);
                emitter.tokens.push(TokenTree::Group(group));
            }
            _ => {
                let placeholder = Ident::new("todo", Span::call_site());
                emitter.tokens.push(TokenTree::Ident(placeholder));
            }
        }
    }
}

impl HaskellUnparse for WithSpan<Pattern<HaskellSpan>, HaskellSpan> {
    fn unparse_to(&self, emitter: &mut TokenStreamEmitter) {
        match &self.slot {
            Pattern::Var(name) => name.unparse_to(emitter),
            Pattern::Con(name) => name.unparse_to(emitter),
            Pattern::Lit(lit) => lit.unparse_to(emitter),
            Pattern::Wildcard => {
                let wildcard = Ident::new("_", Span::call_site());
                emitter.tokens.push(TokenTree::Ident(wildcard));
            }
            Pattern::As(name, pat) => {
                name.unparse_to(emitter);
                let at = Punct::new('@', Spacing::Alone);
                emitter.tokens.push(TokenTree::Punct(at));
                pat.unparse_to(emitter);
            }
            Pattern::App(con, pats) => {
                con.unparse_to(emitter);
                for pat in pats {
                    pat.unparse_to(emitter);
                }
            }
            Pattern::Tuple(pats) => {
                let mut inner_tokens = Vec::new();
                for (i, pat) in pats.iter().enumerate() {
                    let mut inner_emitter = TokenStreamEmitter::new();
                    pat.unparse_to(&mut inner_emitter);
                    inner_tokens.extend(inner_emitter.tokens);
                    
                    if i < pats.len() - 1 {
                        inner_tokens.push(TokenTree::Punct(Punct::new(',', Spacing::Alone)));
                    }
                }
                    
                let tokens: TokenStream = inner_tokens.into_iter().collect();
                let group = Group::new(Delimiter::Parenthesis, tokens);
                emitter.tokens.push(TokenTree::Group(group));
            }
            Pattern::List(pats) => {
                let mut inner_tokens = Vec::new();
                for (i, pat) in pats.iter().enumerate() {
                    let mut inner_emitter = TokenStreamEmitter::new();
                    pat.unparse_to(&mut inner_emitter);
                    inner_tokens.extend(inner_emitter.tokens);
                    
                    if i < pats.len() - 1 {
                        inner_tokens.push(TokenTree::Punct(Punct::new(',', Spacing::Alone)));
                    }
                }
                    
                let tokens: TokenStream = inner_tokens.into_iter().collect();
                let group = Group::new(Delimiter::Bracket, tokens);
                emitter.tokens.push(TokenTree::Group(group));
            }
            Pattern::Paren(pat) => {
                let mut inner_emitter = TokenStreamEmitter::new();
                pat.unparse_to(&mut inner_emitter);
                let tokens: TokenStream = inner_emitter.tokens.into_iter().collect();
                let group = Group::new(Delimiter::Parenthesis, tokens);
                emitter.tokens.push(TokenTree::Group(group));
            }
        }
    }
}