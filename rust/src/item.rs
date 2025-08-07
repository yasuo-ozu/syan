use syan::{
    parse::{Parse, ParseStream, Unparse, IntoParseStream},
    span::{Span, Spanned},
    nested::Choice,
};
use crate::{Expr, Stmt, Type, Pat, Path, token::*};

/// A Rust item
#[derive(Debug, Clone)]
pub enum Item<S: Span> {
    Fn(ItemFn<S>),
    Struct(ItemStruct<S>),
    Enum(ItemEnum<S>),
    Impl(ItemImpl<S>),
    Trait(ItemTrait<S>),
    Use(ItemUse<S>),
    Mod(ItemMod<S>),
    Static(ItemStatic<S>),
    Const(ItemConst<S>),
    Type(ItemType<S>),
}

impl<S: Span> Spanned for Item<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        match self {
            Item::Fn(item) => item.span(),
            Item::Struct(item) => item.span(),
            Item::Enum(item) => item.span(),
            Item::Impl(item) => item.span(),
            Item::Trait(item) => item.span(),
            Item::Use(item) => item.span(),
            Item::Mod(item) => item.span(),
            Item::Static(item) => item.span(),
            Item::Const(item) => item.span(),
            Item::Type(item) => item.span(),
        }
    }
}

/// Function item
#[derive(Debug, Clone)]
pub struct ItemFn<S: Span> {
    pub fn_token: FnToken<S>,
    pub ident: Ident<S>,
    pub generics: Option<Generics<S>>,
    pub paren_token: ParenToken<S>,
    pub inputs: Vec<FnArg<S>>,
    pub output: Option<ReturnType<S>>,
    pub block: Block<S>,
}

impl<S: Span> Spanned for ItemFn<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.fn_token.span().migrate(self.block.span())
    }
}

/// Struct item
#[derive(Debug, Clone)]
pub struct ItemStruct<S: Span> {
    pub struct_token: StructToken<S>,
    pub ident: Ident<S>,
    pub generics: Option<Generics<S>>,
    pub fields: StructFields<S>,
    pub semi_token: Option<SemiToken<S>>,
}

impl<S: Span> Spanned for ItemStruct<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        let end_span = self.semi_token.as_ref()
            .map(|s| s.span())
            .unwrap_or_else(|| self.fields.span());
        self.struct_token.span().migrate(end_span)
    }
}

/// Struct fields
#[derive(Debug, Clone)]
pub enum StructFields<S: Span> {
    Named {
        brace_token: BraceToken<S>,
        fields: Vec<Field<S>>,
    },
    Unnamed {
        paren_token: ParenToken<S>,
        fields: Vec<Field<S>>,
    },
    Unit,
}

impl<S: Span> Spanned for StructFields<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        match self {
            StructFields::Named { brace_token, .. } => brace_token.span(),
            StructFields::Unnamed { paren_token, .. } => paren_token.span(),
            StructFields::Unit => S::default(),
        }
    }
}

/// Field in struct or enum variant
#[derive(Debug, Clone)]
pub struct Field<S: Span> {
    pub ident: Option<Ident<S>>,
    pub colon_token: Option<ColonToken<S>>,
    pub ty: Type<S>,
}

impl<S: Span> Spanned for Field<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        let start = self.ident.as_ref()
            .map(|i| i.span())
            .unwrap_or_else(|| self.ty.span());
        start.migrate(self.ty.span())
    }
}

/// Enum item
#[derive(Debug, Clone)]
pub struct ItemEnum<S: Span> {
    pub enum_token: EnumToken<S>,
    pub ident: Ident<S>,
    pub generics: Option<Generics<S>>,
    pub brace_token: BraceToken<S>,
    pub variants: Vec<Variant<S>>,
}

impl<S: Span> Spanned for ItemEnum<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.enum_token.span().migrate(self.brace_token.span())
    }
}

/// Enum variant
#[derive(Debug, Clone)]
pub struct Variant<S: Span> {
    pub ident: Ident<S>,
    pub fields: StructFields<S>,
    pub discriminant: Option<(EqToken<S>, Expr<S>)>,
}

impl<S: Span> Spanned for Variant<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        let end = self.discriminant.as_ref()
            .map(|(_, expr)| expr.span())
            .unwrap_or_else(|| self.fields.span());
        self.ident.span().migrate(end)
    }
}

// Placeholder implementations for other item types
macro_rules! define_item_stub {
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

define_item_stub!(ItemImpl);
define_item_stub!(ItemTrait);
define_item_stub!(ItemUse);
define_item_stub!(ItemMod);
define_item_stub!(ItemStatic);
define_item_stub!(ItemConst);
define_item_stub!(ItemType);

// Additional helper types
define_item_stub!(Generics);
define_item_stub!(FnArg);
define_item_stub!(ReturnType);
define_item_stub!(Block);