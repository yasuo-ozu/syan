use syan::{
    nested::group::{GroupBracket, GroupParen},
    parse::{Parse, Unparse},
    symbol::Token,
};
use type_macro_derive_tricks::macro_derive;
use crate::{Path, item::TypeParamBound};

/// A Rust type
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum Type<S, Tokens = std::convert::Infallible> {
    Path(TypePath<S>),
    Array(TypeArray<S, Tokens>),
    Slice(TypeSlice<S>),
    Ptr(TypePtr<S>),
    Reference(TypeReference<S>),
    Tuple(TypeTuple<S>),
    Never(Token![S => !]),
    ImplTrait(TypeImplTrait<S>),
    TraitObject(TypeTraitObject<S>),
    Paren(TypeParen<S>),
    Infer(Token![S => _]),
    Macro(TypeMacro<S>),
    // Additional types from rustc_ast
    BareFn(TypeBareFn<S>),
    AnonStruct(TypeAnonStruct<S>),
    AnonUnion(TypeAnonUnion<S>),
    Typeof(TypeTypeof<S, Tokens>),
    ImplicitSelf,
    CVarArgs(TypeCVarArgs<S>),
    Err(TypeErr<S>),
}


/// Path type
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct TypePath<S> {
    pub path: Path<S>,
}


/// Array type [T; N]
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct TypeArray<S, Tokens = std::convert::Infallible> {
    pub bracket_token: GroupBracket<(), S>,
    pub elem: Box<Type<S>>,
    pub semi_token: Token![S => ;],
    pub len: crate::Expr<S, Tokens>,
}


/// Slice type [T]
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct TypeSlice<S> {
    pub bracket_token: GroupBracket<(), S>,
    pub elem: Box<Type<S>>,
}



/// Pointer type (*const T or *mut T)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct TypePtr<S> {
    pub star_token: Token![S => *],
    pub const_token: Option<Token![S => const]>,
    pub mutability: Option<Token![S => mut]>,
    pub elem: Box<Type<S>>,
}

/// Reference type (&T or &mut T)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct TypeReference<S> {
    pub and_token: Token![S => &],
    pub lifetime: Option<crate::Lifetime<S>>,
    pub mutability: Option<Token![S => mut]>,
    pub elem: Box<Type<S>>,
}

/// Tuple type (T1, T2, T3)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct TypeTuple<S> {
    pub paren_token: GroupParen<(), S>,
    pub elems: Vec<Type<S>>,
}

/// Impl trait type (impl Trait)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct TypeImplTrait<S> {
    pub impl_token: Token![S => impl],
    pub bounds: Vec<TypeParamBound<S>>,
}

/// Trait object type (dyn Trait)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct TypeTraitObject<S> {
    pub dyn_token: Option<Token![S => dyn]>,
    pub bounds: Vec<TypeParamBound<S>>,
}

/// Parenthesized type ((T))
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct TypeParen<S> {
    pub paren_token: GroupParen<(), S>,
    pub elem: Box<Type<S>>,
}

/// Macro-generated type
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct TypeMacro<S> {
    pub mac: crate::expr::Macro<S>,
}

// Additional types from rustc_ast

/// Bare function type (fn(usize) -> bool)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct TypeBareFn<S> {
    pub lifetimes: Option<crate::item::BoundLifetimes<S>>,
    pub unsafety: Option<crate::item::Unsafe<S>>,
    pub ext: crate::item::Extern<S>,
    pub fn_token: Token![S => fn],
    pub paren_token: GroupParen<(), S>,
    pub inputs: Vec<BareFnArg<S>>,
    pub c_variadic: Option<CVarArgs<S>>,
    pub output: Option<crate::item::ReturnType<S>>,
}

/// Bare function argument
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct BareFnArg<S> {
    pub attrs: Vec<crate::Attribute<S>>,
    pub name: Option<(crate::Ident<S>, Token![S => :])>,
    pub ty: Type<S>,
}

/// C-style variadic arguments (...)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct CVarArgs<S> {
    pub dot3_token: Token![S => ...],
}

/// Anonymous struct type
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct TypeAnonStruct<S> {
    pub struct_token: Token![S => struct],
    pub fields: crate::item::FieldsNamed<S>,
}

/// Anonymous union type
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct TypeAnonUnion<S> {
    pub union_token: Token![S => union],
    pub fields: crate::item::FieldsNamed<S>,
}

/// Typeof type (typeof(expr))
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct TypeTypeof<S, Tokens = std::convert::Infallible> {
    pub typeof_token: Token![S => typeof],
    pub paren_token: GroupParen<(), S>,
    pub expr: crate::Expr<S, Tokens>,
}

/// C-style variadic arguments type
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct TypeCVarArgs<S> {
    pub dot3_token: Token![S => ...],
}

/// Error type (for error recovery)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct TypeErr<S> {
    pub span: S,
}

/// Mutable type wrapper  
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct MutTy<S> {
    pub ty: Box<Type<S>>,
    pub mutbl: TypeMutability<S>,
}

/// Type mutability
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum TypeMutability<S> {
    Mut(Token![S => mut]),
    Not,
}

/// Trait object syntax  
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum TraitObjectSyntax<S> {
    Dyn(Token![S => dyn]),
    DynStar(Token![S => dyn], Token![S => *]),
    None,
}

/// Integer types
#[derive(Clone, Debug)]
pub enum IntTy {
    Isize,
    I8,
    I16,
    I32,
    I64,
    I128,
}

/// Unsigned integer types
#[derive(Clone, Debug)]
pub enum UintTy {
    Usize,
    U8,
    U16,
    U32,
    U64,
    U128,
}

/// Float types
#[derive(Clone, Debug)]
pub enum FloatTy {
    F16,
    F32,
    F64,
    F128,
}