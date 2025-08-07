use syan::span::{Span, Spanned};
use crate::{Path, token::*};

/// A Rust type
#[derive(Debug, Clone)]
pub enum Type<S: Span> {
    Path(TypePath<S>),
    Array(TypeArray<S>),
    Slice(TypeSlice<S>),
    Ptr(TypePtr<S>),
    Reference(TypeReference<S>),
    Tuple(TypeTuple<S>),
    Never(NeverToken<S>),
    ImplTrait(TypeImplTrait<S>),
    TraitObject(TypeTraitObject<S>),
    Paren(TypeParen<S>),
    Infer(UnderscoreToken<S>),
    Macro(TypeMacro<S>),
}

impl<S: Span> Spanned for Type<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        match self {
            Type::Path(t) => t.span(),
            Type::Array(t) => t.span(),
            Type::Slice(t) => t.span(),
            Type::Ptr(t) => t.span(),
            Type::Reference(t) => t.span(),
            Type::Tuple(t) => t.span(),
            Type::Never(t) => t.span(),
            Type::ImplTrait(t) => t.span(),
            Type::TraitObject(t) => t.span(),
            Type::Paren(t) => t.span(),
            Type::Infer(t) => t.span(),
            Type::Macro(t) => t.span(),
        }
    }
}

/// Path type
#[derive(Debug, Clone)]
pub struct TypePath<S: Span> {
    pub path: Path<S>,
}

impl<S: Span> Spanned for TypePath<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.path.span()
    }
}

/// Array type [T; N]
#[derive(Debug, Clone)]
pub struct TypeArray<S: Span> {
    pub bracket_token: BracketToken<S>,
    pub elem: Box<Type<S>>,
    pub semi_token: SemiToken<S>,
    pub len: crate::Expr<S>,
}

impl<S: Span> Spanned for TypeArray<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.bracket_token.span()
    }
}

/// Slice type [T]
#[derive(Debug, Clone)]
pub struct TypeSlice<S: Span> {
    pub bracket_token: BracketToken<S>,
    pub elem: Box<Type<S>>,
}

impl<S: Span> Spanned for TypeSlice<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.bracket_token.span()
    }
}

// Placeholder implementations for other type variants
macro_rules! define_type_stub {
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

define_type_stub!(TypePtr);
define_type_stub!(TypeReference);
define_type_stub!(TypeTuple);
define_type_stub!(TypeImplTrait);
define_type_stub!(TypeTraitObject);
define_type_stub!(TypeParen);
define_type_stub!(TypeMacro);