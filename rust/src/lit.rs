use syan::span::{Span, Spanned};

/// A Rust literal
#[derive(Debug, Clone)]
pub enum Lit<S: Span> {
    Str(LitStr<S>),
    ByteStr(LitByteStr<S>),
    Byte(LitByte<S>),
    Char(LitChar<S>),
    Int(LitInt<S>),
    Float(LitFloat<S>),
    Bool(LitBool<S>),
}

impl<S: Span> Spanned for Lit<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        match self {
            Lit::Str(lit) => lit.span(),
            Lit::ByteStr(lit) => lit.span(),
            Lit::Byte(lit) => lit.span(),
            Lit::Char(lit) => lit.span(),
            Lit::Int(lit) => lit.span(),
            Lit::Float(lit) => lit.span(),
            Lit::Bool(lit) => lit.span(),
        }
    }
}

/// String literal
#[derive(Debug, Clone)]
pub struct LitStr<S: Span> {
    pub value: String,
    pub span: S,
}

impl<S: Span> Spanned for LitStr<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.span.clone()
    }
}

/// Byte string literal
#[derive(Debug, Clone)]
pub struct LitByteStr<S: Span> {
    pub value: Vec<u8>,
    pub span: S,
}

impl<S: Span> Spanned for LitByteStr<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.span.clone()
    }
}

/// Byte literal
#[derive(Debug, Clone)]
pub struct LitByte<S: Span> {
    pub value: u8,
    pub span: S,
}

impl<S: Span> Spanned for LitByte<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.span.clone()
    }
}

/// Character literal
#[derive(Debug, Clone)]
pub struct LitChar<S: Span> {
    pub value: char,
    pub span: S,
}

impl<S: Span> Spanned for LitChar<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.span.clone()
    }
}

/// Integer literal
#[derive(Debug, Clone)]
pub struct LitInt<S: Span> {
    pub value: u64,
    pub suffix: Option<String>,
    pub span: S,
}

impl<S: Span> Spanned for LitInt<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.span.clone()
    }
}

/// Float literal
#[derive(Debug, Clone)]
pub struct LitFloat<S: Span> {
    pub value: f64,
    pub suffix: Option<String>,
    pub span: S,
}

impl<S: Span> Spanned for LitFloat<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.span.clone()
    }
}

/// Boolean literal
#[derive(Debug, Clone)]
pub struct LitBool<S: Span> {
    pub value: bool,
    pub span: S,
}

impl<S: Span> Spanned for LitBool<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.span.clone()
    }
}