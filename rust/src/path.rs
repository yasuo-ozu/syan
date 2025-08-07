use syan::span::{Span, Spanned};
use crate::{Type, token::*};

/// A path like `std::collections::HashMap`
#[derive(Debug, Clone)]
pub struct Path<S: Span> {
    pub leading_colon: Option<ColonColonToken<S>>,
    pub segments: Vec<PathSegment<S>>,
}

impl<S: Span> Spanned for Path<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        let start = self.leading_colon.as_ref()
            .map(|c| c.span())
            .unwrap_or_else(|| {
                self.segments.first()
                    .map(|s| s.span())
                    .unwrap_or_default()
            });
        
        let end = self.segments.last()
            .map(|s| s.span())
            .unwrap_or_else(|| start.clone());
        
        start.migrate(end)
    }
}

/// A segment of a path like `HashMap` in `std::collections::HashMap`
#[derive(Debug, Clone)]
pub struct PathSegment<S: Span> {
    pub ident: Ident<S>,
    pub arguments: Option<PathArguments<S>>,
}

impl<S: Span> Spanned for PathSegment<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        let end = self.arguments.as_ref()
            .map(|a| a.span())
            .unwrap_or_else(|| self.ident.span());
        self.ident.span().migrate(end)
    }
}

/// Path arguments like `<T, U>` in `HashMap<T, U>`
#[derive(Debug, Clone)]
pub enum PathArguments<S: Span> {
    None,
    AngleBracketed(AngleBracketedGenericArguments<S>),
    Parenthesized(ParenthesizedGenericArguments<S>),
}

impl<S: Span> Spanned for PathArguments<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        match self {
            PathArguments::None => S::default(),
            PathArguments::AngleBracketed(args) => args.span(),
            PathArguments::Parenthesized(args) => args.span(),
        }
    }
}

/// Angle-bracketed generic arguments `<T, U>`
#[derive(Debug, Clone)]
pub struct AngleBracketedGenericArguments<S: Span> {
    pub lt_token: LtToken<S>,
    pub args: Vec<GenericArgument<S>>,
    pub gt_token: GtToken<S>,
}

impl<S: Span> Spanned for AngleBracketedGenericArguments<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.lt_token.span().migrate(self.gt_token.span())
    }
}

/// Parenthesized generic arguments `(A, B) -> C`
#[derive(Debug, Clone)]
pub struct ParenthesizedGenericArguments<S: Span> {
    pub paren_token: ParenToken<S>,
    pub inputs: Vec<Type<S>>,
    pub output: Option<(RArrowToken<S>, Type<S>)>,
}

impl<S: Span> Spanned for ParenthesizedGenericArguments<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        let end = self.output.as_ref()
            .map(|(_, ty)| ty.span())
            .unwrap_or_else(|| self.paren_token.span());
        self.paren_token.span().migrate(end)
    }
}

/// A generic argument
#[derive(Debug, Clone)]
pub enum GenericArgument<S: Span> {
    Type(Type<S>),
    Const(crate::Expr<S>),
    Lifetime(Lifetime<S>),
    Binding(Binding<S>),
    Constraint(Constraint<S>),
}

impl<S: Span> Spanned for GenericArgument<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        match self {
            GenericArgument::Type(ty) => ty.span(),
            GenericArgument::Const(expr) => expr.span(),
            GenericArgument::Lifetime(lt) => lt.span(),
            GenericArgument::Binding(binding) => binding.span(),
            GenericArgument::Constraint(constraint) => constraint.span(),
        }
    }
}

/// Lifetime like `'a`
#[derive(Debug, Clone)]
pub struct Lifetime<S: Span> {
    pub apostrophe: ApostropheToken<S>,
    pub ident: Ident<S>,
}

impl<S: Span> Spanned for Lifetime<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.apostrophe.span().migrate(self.ident.span())
    }
}

// Placeholder implementations
macro_rules! define_path_stub {
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

define_path_stub!(Binding);
define_path_stub!(Constraint);