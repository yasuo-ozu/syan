use syan::span::{Span, Spanned};
use crate::{Path, Type, token::*};

/// A Rust pattern
#[derive(Debug, Clone)]
pub enum Pat<S: Span> {
    Ident(PatIdent<S>),
    Struct(PatStruct<S>),
    TupleStruct(PatTupleStruct<S>),
    Path(PatPath<S>),
    Tuple(PatTuple<S>),
    Box(PatBox<S>),
    Ref(PatRef<S>),
    Lit(PatLit<S>),
    Range(PatRange<S>),
    Slice(PatSlice<S>),
    Rest(PatRest<S>),
    Paren(PatParen<S>),
    Wild(UnderscoreToken<S>),
    Macro(PatMacro<S>),
    Or(PatOr<S>),
}

impl<S: Span> Spanned for Pat<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        match self {
            Pat::Ident(p) => p.span(),
            Pat::Struct(p) => p.span(),
            Pat::TupleStruct(p) => p.span(),
            Pat::Path(p) => p.span(),
            Pat::Tuple(p) => p.span(),
            Pat::Box(p) => p.span(),
            Pat::Ref(p) => p.span(),
            Pat::Lit(p) => p.span(),
            Pat::Range(p) => p.span(),
            Pat::Slice(p) => p.span(),
            Pat::Rest(p) => p.span(),
            Pat::Paren(p) => p.span(),
            Pat::Wild(p) => p.span(),
            Pat::Macro(p) => p.span(),
            Pat::Or(p) => p.span(),
        }
    }
}

/// Identifier pattern with optional binding mode and subpattern
#[derive(Debug, Clone)]
pub struct PatIdent<S: Span> {
    pub by_ref: Option<RefToken<S>>,
    pub mutability: Option<MutToken<S>>,
    pub ident: Ident<S>,
    pub subpat: Option<(AtToken<S>, Box<Pat<S>>)>,
}

impl<S: Span> Spanned for PatIdent<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        let start = self.by_ref.as_ref()
            .map(|r| r.span())
            .or_else(|| self.mutability.as_ref().map(|m| m.span()))
            .unwrap_or_else(|| self.ident.span());
        
        let end = self.subpat.as_ref()
            .map(|(_, pat)| pat.span())
            .unwrap_or_else(|| self.ident.span());
        
        start.migrate(end)
    }
}

/// Struct pattern
#[derive(Debug, Clone)]
pub struct PatStruct<S: Span> {
    pub path: Path<S>,
    pub brace_token: BraceToken<S>,
    pub fields: Vec<FieldPat<S>>,
    pub dot2_token: Option<Dot2Token<S>>,
}

impl<S: Span> Spanned for PatStruct<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.path.span().migrate(self.brace_token.span())
    }
}

/// Field pattern in struct pattern
#[derive(Debug, Clone)]
pub struct FieldPat<S: Span> {
    pub member: Member<S>,
    pub colon_token: Option<ColonToken<S>>,
    pub pat: Box<Pat<S>>,
}

impl<S: Span> Spanned for FieldPat<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.member.span().migrate(self.pat.span())
    }
}

/// Struct field member (identifier or index)
#[derive(Debug, Clone)]
pub enum Member<S: Span> {
    Named(Ident<S>),
    Unnamed(crate::LitInt<S>),
}

impl<S: Span> Spanned for Member<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        match self {
            Member::Named(ident) => ident.span(),
            Member::Unnamed(index) => index.span(),
        }
    }
}

// Placeholder implementations for other pattern types
macro_rules! define_pat_stub {
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

define_pat_stub!(PatTupleStruct);
define_pat_stub!(PatPath);
define_pat_stub!(PatTuple);
define_pat_stub!(PatBox);
define_pat_stub!(PatRef);
define_pat_stub!(PatLit);
define_pat_stub!(PatRange);
define_pat_stub!(PatSlice);
define_pat_stub!(PatRest);
define_pat_stub!(PatParen);
define_pat_stub!(PatMacro);
define_pat_stub!(PatOr);