use crate::ast::*;
use proc_macro2::{TokenStream, TokenTree};
use syan::span::{Span, WithSpan};

#[derive(Clone, Debug)]
pub struct HaskellSpan {
    pub start: usize,
    pub end: usize,
}

impl Default for HaskellSpan {
    fn default() -> Self {
        HaskellSpan { start: 0, end: 0 }
    }
}

impl Span for HaskellSpan {
    fn migrate(self, other: Self) -> Self {
        HaskellSpan {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

pub trait HaskellParse {
    type Error;
    fn haskell_parse(tokens: TokenStream) -> Result<Self, Self::Error>
    where
        Self: Sized;
}

impl HaskellParse for WithSpan<String, HaskellSpan> {
    type Error = ();
    
    fn haskell_parse(tokens: TokenStream) -> Result<Self, Self::Error> {
        let mut tokens_iter = tokens.into_iter();
        if let Some(token) = tokens_iter.next() {
            match token {
                TokenTree::Ident(ident) => {
                    Ok(WithSpan {
                        slot: ident.to_string(),
                        span: HaskellSpan::default(),
                    })
                }
                _ => Err(()),
            }
        } else {
            Err(())
        }
    }
}

impl HaskellParse for WithSpan<ModuleName<HaskellSpan>, HaskellSpan> {
    type Error = ();
    
    fn haskell_parse(tokens: TokenStream) -> Result<Self, Self::Error> {
        let name = WithSpan::<String, HaskellSpan>::haskell_parse(tokens)?;
        Ok(WithSpan {
            slot: ModuleName { name },
            span: HaskellSpan::default(),
        })
    }
}

impl HaskellParse for WithSpan<Literal, HaskellSpan> {
    type Error = ();
    
    fn haskell_parse(tokens: TokenStream) -> Result<Self, Self::Error> {
        let mut tokens_iter = tokens.into_iter();
        if let Some(token) = tokens_iter.next() {
            match token {
                TokenTree::Literal(lit) => {
                    let lit_str = lit.to_string();
                    let literal = if lit_str.starts_with('"') && lit_str.ends_with('"') {
                        Literal::String(lit_str[1..lit_str.len()-1].to_string())
                    } else if lit_str.starts_with('\'') && lit_str.ends_with('\'') {
                        let chars: Vec<char> = lit_str[1..lit_str.len()-1].chars().collect();
                        if chars.len() == 1 {
                            Literal::Char(chars[0])
                        } else {
                            return Err(());
                        }
                    } else if lit_str.contains('.') {
                        if let Ok(f) = lit_str.parse::<f64>() {
                            Literal::Float(f)
                        } else {
                            return Err(());
                        }
                    } else {
                        if let Ok(i) = lit_str.parse::<i64>() {
                            Literal::Integer(i)
                        } else {
                            return Err(());
                        }
                    };
                    
                    Ok(WithSpan {
                        slot: literal,
                        span: HaskellSpan::default(),
                    })
                }
                _ => Err(()),
            }
        } else {
            Err(())
        }
    }
}

impl HaskellParse for WithSpan<Expression<HaskellSpan>, HaskellSpan> {
    type Error = ();
    
    fn haskell_parse(tokens: TokenStream) -> Result<Self, Self::Error> {
        let mut tokens_iter = tokens.clone().into_iter().peekable();
        if let Some(token) = tokens_iter.peek() {
            match token {
                TokenTree::Ident(_) => {
                    let name = WithSpan::<String, HaskellSpan>::haskell_parse(tokens)?;
                    Ok(WithSpan {
                        slot: Expression::Var(name),
                        span: HaskellSpan::default(),
                    })
                }
                TokenTree::Literal(_) => {
                    let lit = WithSpan::<Literal, HaskellSpan>::haskell_parse(tokens)?;
                    Ok(WithSpan {
                        slot: Expression::Lit(lit),
                        span: HaskellSpan::default(),
                    })
                }
                TokenTree::Group(_) => {
                    Ok(WithSpan {
                        slot: Expression::Tuple(Vec::new()),
                        span: HaskellSpan::default(),
                    })
                }
                _ => Err(()),
            }
        } else {
            Err(())
        }
    }
}

impl HaskellParse for WithSpan<Pattern<HaskellSpan>, HaskellSpan> {
    type Error = ();
    
    fn haskell_parse(tokens: TokenStream) -> Result<Self, Self::Error> {
        let mut tokens_iter = tokens.clone().into_iter().peekable();
        if let Some(token) = tokens_iter.peek() {
            match token {
                TokenTree::Ident(ident) if ident.to_string() == "_" => {
                    Ok(WithSpan {
                        slot: Pattern::Wildcard,
                        span: HaskellSpan::default(),
                    })
                }
                TokenTree::Ident(_) => {
                    let name = WithSpan::<String, HaskellSpan>::haskell_parse(tokens)?;
                    Ok(WithSpan {
                        slot: Pattern::Var(name),
                        span: HaskellSpan::default(),
                    })
                }
                TokenTree::Literal(_) => {
                    let lit = WithSpan::<Literal, HaskellSpan>::haskell_parse(tokens)?;
                    Ok(WithSpan {
                        slot: Pattern::Lit(lit),
                        span: HaskellSpan::default(),
                    })
                }
                TokenTree::Punct(p) if p.as_char() == '_' => {
                    Ok(WithSpan {
                        slot: Pattern::Wildcard,
                        span: HaskellSpan::default(),
                    })
                }
                _ => Err(()),
            }
        } else {
            Err(())
        }
    }
}