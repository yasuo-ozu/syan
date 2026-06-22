use crate::{tokens::*, Expr, Type, GenericParam};
use syan::{
    nested::{
        group::{GroupAngle, GroupBrace, GroupParen},
        punctuated::Punctuated,
    },
    parse::{Parse, Unparse},
    span::Spanned,
    symbol::Token,
};
use type_macro_derive_tricks::macro_derive;

/// A Rust item with common attributes
/// Example: #[derive(Debug)] pub fn hello() {}
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct Item<S, Tokens = std::convert::Infallible> {
    // TODO! make `kind` parametric
    pub attrs: Vec<crate::Attribute<S>>,
    pub vis: crate::Visibility<S>,
    pub kind: ItemKind<S, Tokens>,
}

/// Item kinds
/// Examples: fn, struct, enum, impl, etc.
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum ItemKind<S, Tokens = std::convert::Infallible> {
    ExternCrate {
        ident_extern: Token![S => extern],
        crate_extern: Token![S => crate],
        name: Ident<S>,
        rename: Option<(Token![S => as], Ident<S>)>,
        semi_token: Token![S => ;],
    },
    Use {
        use_token: Token![S => use],
        tree: UseTree<S>,
        semi_token: Token![S => ;],
    },
    Static {
        safety: crate::Safety<S>,
        static_token: Token![S => static],
        mutability: Option<Token![S => mut]>,
        ident: Ident<S>,
        colon_token: Token![S => :],
        ty: Box<Type<S>>,
        eq_token: Token![S => =],
        expr: Box<Expr<S, Tokens>>,
        semi_token: Token![S => ;],
    },
    Const {
        const_token: Token![S => static],
        ident: Ident<S>,
        // generic_const_items
        generics: Option<
            GroupAngle<
                (
                    Punctuated<GenericParam<S>, Token![S => ,]>,
                    Option<Token![S => ,]>,
                ),
                S,
            >,
        >,
        colon_token: Token![S => :],
        ty: Box<Type<S>>,
        eq_token: Token![S => =],
        expr: Box<Expr<S, Tokens>>,
        // generic_const_items
        where_clause: Option<(
            Token![S => where],
            Punctuated<WherePredicate<S>, Token![S => ,]>,
            Option<Token![S => ,]>,
        )>,
        semi_token: Token![S => ;],
    },
    Fn(ItemFn<S>),
    Mod(ItemMod<S, Tokens>),
    ForeignMod(ItemForeignMod<S>),
    GlobalAsm(ItemGlobalAsm<S>),
    TyAlias(ItemTyAlias<S>),
    Enum(ItemEnum<S, Tokens>),
    Struct(ItemStruct<S>),
    Union(ItemUnion<S>),
    Trait(ItemTrait<S>),
    TraitAlias(ItemTraitAlias<S>),
    Impl(ItemImpl<S>),
    MacroCall(ItemMacroCall<S>),
    MacroDef(ItemMacroDef<S, Tokens>),
    Delegation(ItemDelegation<S>),
    DelegationMac(ItemDelegationMac<S>),
}

/// Function item
/// Example: fn hello(name: &str) -> String { format!("Hello, {}!", name) }
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ItemFn<S, Tokens = core::convert::Infallible> {
    pub defaultness: Defaultness<S>,
    pub sig: FnSig<S>,
    pub brace_token: GroupBrace<(), S>,
    #[group(self.brace_token)]
    pub stmts: Vec<crate::Stmt<S, Tokens>>,
}

/// Struct item
/// Example: struct Point { x: i32, y: i32 }
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ItemStruct<S> {
    pub struct_token: Token![S => struct],
    pub ident: Ident<S>,
    pub generics: Option<Generics<S>>,
    pub fields: StructFields<S>,
    pub semi_token: Option<Token![S => ;]>,
}

/// Struct fields
/// Examples: { x: i32, y: i32 } or (i32, i32) or unit
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum StructFields<S> {
    Named {
        brace_token: GroupBrace<(), S>,
        fields: Vec<Field<S>>,
    },
    Unnamed {
        paren_token: GroupParen<(), S>,
        fields: Vec<Field<S>>,
    },
    Unit,
}

/// Field in struct or enum variant
/// Example: name: String or String (tuple field)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct Field<S> {
    pub ident: Option<Ident<S>>,
    pub colon_token: Option<Token![S => :]>,
    pub ty: Type<S>,
}

/// Enum item
/// Example: enum Color { Red, Green, Blue(u8, u8, u8) }
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ItemEnum<S, Tokens = std::convert::Infallible> {
    pub enum_token: Token![S => enum],
    pub ident: Ident<S>,
    pub generics: Option<Generics<S>>,
    pub brace_token: GroupBrace<(), S>,
    pub variants: Vec<Variant<S, Tokens>>,
}

/// Enum variant
/// Example: Red or Blue(u8, u8, u8) or Point { x: i32, y: i32 }
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct Variant<S, Tokens = std::convert::Infallible> {
    pub ident: Ident<S>,
    pub fields: StructFields<S>,
    pub discriminant: Option<(Token![S => =], Expr<S, Tokens>)>,
}


/// Implementation block (impl Trait for Type { ... })
/// Example: impl Display for Point { fn fmt(&self, f: &mut Formatter) -> Result<(), Error> { ... } }
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ItemImpl<S> {
    pub defaultness: Option<Defaultness<S>>,
    pub unsafety: Option<Unsafe<S>>,
    pub impl_token: Token![S => impl],
    pub generics: Option<Generics<S>>,
    pub polarity: Option<ImplPolarity<S>>,
    pub trait_: Option<TraitRef<S>>,
    pub for_token: Option<Token![S => for]>,
    pub self_ty: Box<Type<S>>,
    pub where_clause: Option<WhereClause<S>>,
    pub brace_token: GroupBrace<(), S>,
    pub items: Vec<AssocItem<S>>,
}

/// Trait definition (trait Name { ... })
/// Example: trait Draw { fn draw(&self); fn area(&self) -> f64; }
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ItemTrait<S> {
    pub unsafety: Option<Unsafe<S>>,
    pub auto_token: Option<Token![S => auto]>,
    pub trait_token: Token![S => trait],
    pub ident: Ident<S>,
    pub generics: Option<Generics<S>>,
    pub colon_token: Option<Token![S => :]>,
    pub supertraits: Vec<TypeParamBound<S>>,
    pub where_clause: Option<WhereClause<S>>,
    pub brace_token: GroupBrace<(), S>,
    pub items: Vec<AssocItem<S>>,
}

/// Trait alias (trait Name = TraitBound + AnotherBound;)
/// Example: trait MyIterator<T> = Iterator<Item = T> + Send;
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ItemTraitAlias<S> {
    pub trait_token: Token![S => trait],
    pub ident: Ident<S>,
    pub generics: Option<Generics<S>>,
    pub eq_token: Token![S => =],
    pub bounds: Vec<TypeParamBound<S>>,
    pub where_clause: Option<WhereClause<S>>,
    pub semi_token: Token![S => ;],
}

/// Module declaration (mod name { ... })
/// Example: mod utils { pub fn helper() {} }
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ItemMod<S, Tokens = std::convert::Infallible> {
    pub unsafety: Option<Unsafe<S>>,
    pub mod_token: Token![S => mod],
    pub ident: Ident<S>,
    pub content: Option<ModContent<S, Tokens>>,
    pub semi_token: Option<Token![S => ;]>,
}

/// Type alias (type Name = Type;)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ItemType<S> {
    pub type_token: Token![S => type],
    pub ident: Ident<S>,
    pub generics: Option<Generics<S>>,
    pub where_clause: Option<WhereClause<S>>,
    pub eq_token: Token![S => =],
    pub ty: Box<Type<S>>,
    pub semi_token: Token![S => ;],
}

/// Union item (union Name { ... })
/// Example: union Value { int: i32, float: f32 }
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ItemUnion<S> {
    pub union_token: Token![S => union],
    pub ident: Ident<S>,
    pub generics: Option<Generics<S>>,
    pub where_clause: Option<WhereClause<S>>,
    pub fields: FieldsNamed<S>,
}

/// Type alias (type Name = Type;) - modern form
/// Example: type Result<T> = std::result::Result<T, MyError>;
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ItemTyAlias<S> {
    pub defaultness: Option<Defaultness<S>>,
    pub type_token: Token![S => type],
    pub ident: Ident<S>,
    pub generics: Option<Generics<S>>,
    pub where_clause: Option<WhereClause<S>>,
    pub bounds: Vec<TypeParamBound<S>>,
    pub eq_token: Option<Token![S => =]>,
    pub ty: Option<Box<Type<S>>>,
    pub semi_token: Token![S => ;],
}

/// Foreign module (extern "C" { ... })
/// Example: extern "C" { fn malloc(size: usize) -> *mut c_void; }
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ItemForeignMod<S> {
    pub unsafety: Option<Unsafe<S>>,
    pub abi: Extern<S>,
    pub brace_token: GroupBrace<(), S>,
    pub items: Vec<ForeignItem<S>>,
}

/// Global assembly (global_asm!(...))
/// Example: global_asm!("nop");
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ItemGlobalAsm<S> {
    pub global_asm: GlobalAsm<S>,
}

/// Macro call item (macro!(...);)
/// Example: println!("Hello, world!");
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ItemMacroCall<S> {
    pub mac: crate::expr::Macro<S>,
    // TODO: combine it with MacroDelim, to disallow `macro!{ .. };`
    pub semi_token: Option<Token![S => ;]>,
}

/// Named fields (for structs/unions)
/// Example: { x: i32, y: i32, name: String }
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct FieldsNamed<S> {
    pub brace_token: GroupBrace<(), S>,
    pub fields: Vec<Field<S>>,
}

/// Foreign item
/// Examples: extern functions, types, statics in extern blocks
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum ForeignItem<S> {
    Static(ForeignStatic<S>),
    Fn(ForeignFn<S>),
    TyAlias(ForeignTyAlias<S>),
    MacCall(ForeignMacCall<S>),
}

/// Foreign static item
/// Example: static errno: c_int;
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ForeignStatic<S> {
    pub static_token: Token![S => static],
    pub mutability: Option<Token![S => mut]>,
    pub ident: Ident<S>,
    pub colon_token: Token![S => :],
    pub ty: Box<Type<S>>,
    pub semi_token: Token![S => ;],
}

/// Foreign function item
/// Example: fn malloc(size: usize) -> *mut c_void;
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ForeignFn<S> {
    pub sig: FnSig<S>,
    pub semi_token: Token![S => ;],
}

/// Foreign type alias
/// Example: type FILE;
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ForeignTyAlias<S> {
    pub type_token: Token![S => type],
    pub ident: Ident<S>,
    pub generics: Option<Generics<S>>,
    pub where_clause: Option<WhereClause<S>>,
    pub semi_token: Token![S => ;],
}

/// Foreign macro call
/// Example: some_macro!() in extern block;
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ForeignMacCall<S> {
    pub mac: crate::expr::Macro<S>,
    pub semi_token: Option<Token![S => ;]>,
}

/// Global assembly
/// Example: global_asm!("nop", options(att_syntax));
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct GlobalAsm<S> {
    pub global_asm_token: Token![S => global_asm],
    pub bang_token: Token![S => !],
    pub paren_token: GroupParen<(), S>,
    pub template: crate::Lit<S>,
    pub options: Vec<GlobalAsmOptions<S>>,
}

/// Global assembly options
/// Examples: att_syntax, intel_syntax, options("raw")
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum GlobalAsmOptions<S> {
    Att,
    Intel,
    Options(crate::Lit<S>),
}

// Missing helper types for items

/// Defaultness of associated items
/// Example: default fn method() or just fn method()
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum Defaultness<S> {
    Default(Token![S => default]),
    Final,
}

/// Unsafety of functions and blocks
/// Example: unsafe fn dangerous() {}
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum Unsafe<S> {
    Yes(Token![S => unsafe]),
}

/// Implementation polarity (positive/negative impl)
/// Example: impl !Send for MyType {}
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum ImplPolarity<S> {
    Positive,
    Negative(Token![S => !]),
}

/// Trait reference in impl
/// Example: Display in impl Display for MyType
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct TraitRef<S> {
    pub path: crate::Path<S>,
}

/// Associated item in trait or impl
/// Examples: const, fn, type, macro in trait/impl blocks
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum AssocItem<S> {
    Const(AssocConst<S>),
    Fn(AssocFn<S>),
    Type(AssocType<S>),
    Macro(AssocMacro<S>),
}

/// Associated const
/// Example: const NAME: usize = 42;
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct AssocConst<S, Tokens = std::convert::Infallible> {
    pub defaultness: Option<Defaultness<S>>,
    pub const_token: Token![S => const],
    pub ident: Ident<S>,
    pub colon_token: Token![S => :],
    pub ty: Type<S>,
    pub eq_token: Option<Token![S => =]>,
    pub value: Option<Expr<S, Tokens>>,
    pub semi_token: Token![S => ;],
}

/// Associated function
/// Example: fn method(&self) -> String { ... }
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct AssocFn<S> {
    pub defaultness: Option<Defaultness<S>>,
    pub sig: FnSig<S>,
    pub body: Option<Block<S>>,
    pub semi_token: Option<Token![S => ;]>,
}

/// Associated type
/// Example: type Item = String;
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct AssocType<S> {
    pub defaultness: Option<Defaultness<S>>,
    pub type_token: Token![S => type],
    pub ident: Ident<S>,
    pub generics: Option<Generics<S>>,
    pub where_clause: Option<WhereClause<S>>,
    pub bounds: Vec<TypeParamBound<S>>,
    pub eq_token: Option<Token![S => =]>,
    pub ty: Option<Type<S>>,
    pub semi_token: Token![S => ;],
}

/// Associated macro
/// Example: my_macro!() in trait/impl block;
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct AssocMacro<S> {
    pub mac: crate::expr::Macro<S>,
    pub semi_token: Option<Token![S => ;]>,
}

/// Function signature
/// Example: fn hello(name: &str) -> String
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct FnSig<S> {
    pub constness: Option<Token![S => const]>,
    pub asyncness: Option<Token![S => async]>,
    pub coroutine_kind: Option<Token![S => gen]>,
    pub safety: Option<crate::Safety<S>>,
    pub ext: Option<Extern<S>>,
    pub fn_token: Token![S => fn],
    pub ident: Ident<S>,
    pub generics: Option<
        GroupAngle<
            (
                Punctuated<GenericParam<S>, Token![S => ,]>,
                Option<Token![S => ,]>,
            ),
            S,
        >,
    >,
    pub paren_token: GroupParen<(), S>,
    pub inputs: Vec<FnArg<S>>,
    pub output: Option<ReturnType<S>>,
    pub where_clause: Option<WhereClause<S>>,
}

/// External ABI
/// Examples: extern "C", extern, or no extern
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum Extern<S> {
    Implicit(Token![S => extern]),
    Explicit(Token![S => extern], crate::Lit<S>),
}

/// Use tree structure
/// Examples: std::vec::Vec, std::*, std::{Vec, HashMap}, std::vec::Vec as Vector

#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum UseTree<S> {
    Simple {
        name: Ident<S>,
        rename: Option<(Token![S => as], Ident<S>)>,
    },
    Glob {
        star_token: Token![S => *],
    },
    Group {
        brace_token: GroupBrace<(), S>,
        #[group(self.brace_token)]
        contents: Punctuated<UseTree<S>, Token![S => ,]>,
    },
    Prefixed {
        prefix: crate::Path<S>,
        semi_token: Token![S => ::],
        tree: Box<UseTree<S>>,
    },
}

/// Module content
/// Example: { fn helper() {} const VALUE: i32 = 42; }
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ModContent<S, Tokens = std::convert::Infallible> {
    pub brace_token: GroupBrace<(), S>,
    pub items: Vec<Item<S, Tokens>>,
}

// Additional helper types

/// Generic parameters <T, U>
/// Example: <T: Clone, U: Display, const N: usize>
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct Generics<S> {
    pub lt_token: Option<Token![S => <]>,
    pub params: Vec<GenericParam<S>>,
    pub gt_token: Option<Token![S => >]>,
    pub where_clause: Option<WhereClause<S>>,
}

/// Const parameter
/// Example: const N: usize = 10
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ConstParam<S, Tokens = std::convert::Infallible> {
    pub const_token: Token![S => const],
    pub ident: Ident<S>,
    pub colon_token: Token![S => :],
    pub ty: Type<S>,
    pub eq_token: Option<Token![S => =]>,
    pub default: Option<Expr<S, Tokens>>,
}

/// Type parameter bound
/// Examples: Clone, Display, 'static
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum TypeParamBound<S> {
    Trait(TraitBound<S>),
    Lifetime(crate::Lifetime<S>),
}

/// Trait bound
/// Example: Clone + Send + 'static
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct TraitBound<S> {
    pub paren_token: Option<GroupParen<(), S>>,
    pub modifier: TraitBoundModifier<S>,
    pub lifetimes: Option<BoundLifetimes<S>>,
    pub path: crate::Path<S>,
}

/// Trait bound modifier
/// Examples: ?Sized, Send (no modifier)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum TraitBoundModifier<S> {
    None,
    Maybe(Token![S => ?]),
}

/// Higher-ranked trait bounds
/// Example: for<'a> Fn(&'a str) -> &'a str
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct BoundLifetimes<S> {
    pub for_token: Token![S => for],
    pub lt_token: Token![S => <],
    pub lifetimes: Vec<crate::Lifetime<S>>,
    pub gt_token: Token![S => >],
}

/// Where clause
/// Example: where T: Clone, U: Display, N: const
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct WhereClause<S> {
    pub where_token: Token![S => where],
    pub predicates: Vec<WherePredicate<S>>,
}

/// Where predicate
/// Examples: T: Clone, 'a: 'b, T = String
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum WherePredicate<S> {
    Type(PredicateType<S>),
    Lifetime(PredicateLifetime<S>),
    Eq(PredicateEq<S>),
}

/// Type predicate in where clause
/// Example: T: Clone + Send
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PredicateType<S> {
    pub lifetimes: Option<BoundLifetimes<S>>,
    pub bounded_ty: Type<S>,
    pub colon_token: Token![S => :],
    pub bounds: Vec<TypeParamBound<S>>,
}

/// Lifetime predicate in where clause
/// Example: 'a: 'b + 'c
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PredicateLifetime<S> {
    pub lifetime: crate::Lifetime<S>,
    pub colon_token: Token![S => :],
    pub bounds: Vec<crate::Lifetime<S>>,
}

/// Equality predicate in where clause
/// Example: T::Item = String
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PredicateEq<S> {
    pub lhs_ty: Type<S>,
    pub eq_token: Token![S => =],
    pub rhs_ty: Type<S>,
}

/// Function argument
/// Example: name: &str, value: Option<i32>
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct FnArg<S> {
    pub pat: crate::Pat<S>,
    pub colon_token: Token![S => :],
    pub ty: Type<S>,
}

/// Return type annotation
/// Example: -> Result<String, Error>
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ReturnType<S> {
    pub arrow_token: Token![S => ->],
    pub ty: Box<Type<S>>,
}

/// Macro definition (macro_rules! name { ... })
/// Example: macro_rules! vec { ($($x:expr),*) => { ... }; }
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ItemMacroDef<S, Tokens = std::convert::Infallible> {
    // TODO: support macro_rules! and macro
    pub macro_rules_token: Token![S => macro_rules],
    pub bang_token: Token![S => !],
    pub ident: Ident<S>,
    pub brace_token: GroupBrace<(), S>,
    pub rules: Vec<MacroRule<S, Tokens>>,
}

/// Delegation item (reuse impl)
/// Example: reuse SomeTrait { fn method as other_method; }
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ItemDelegation<S> {
    pub reuse_token: Token![S => reuse],
    pub path: crate::Path<S>,
    pub brace_token: Option<GroupBrace<(), S>>,
    pub items: Vec<DelegationItem<S>>,
}

/// Delegation macro call item (reuse macro!(...);)
/// Example: reuse some_macro!(args);
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ItemDelegationMac<S> {
    pub reuse_token: Token![S => reuse],
    pub mac: crate::expr::Macro<S>,
    pub semi_token: Option<Token![S => ;]>,
}

/// Macro rule in macro_rules!
/// Example: ($($x:expr),*) => { vec![$($x),*] };
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct MacroRule<S, Tokens = std::convert::Infallible> {
    pub matcher: MacroMatcher<S, Tokens>,
    pub fat_arrow_token: Token![S => =>],
    pub transcriber: MacroTranscriber<S, Tokens>,
    pub semi_token: Option<Token![S => ;]>,
}

/// Macro matcher (left side of =>)
/// Example: ($($x:expr),*)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct MacroMatcher<S, Tokens = std::convert::Infallible> {
    pub paren_token: GroupParen<(), S>,
    pub tokens: Tokens,
}

/// Macro transcriber (right side of =>)
/// Example: { vec![$($x),*] }
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct MacroTranscriber<S, Tokens = std::convert::Infallible> {
    pub brace_token: GroupBrace<(), S>,
    pub tokens: Tokens,
}

/// Item in delegation block
/// Example: fn method as other_method;
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct DelegationItem<S> {
    pub kind: DelegationItemKind<S>,
    pub rename: Option<(Token![S => as], Ident<S>)>,
    pub semi_token: Token![S => ;],
}

/// Kind of delegation item
/// Examples: fn method, type Item, const VALUE
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum DelegationItemKind<S> {
    Fn(Ident<S>),
    Type(Ident<S>),
    Const(Ident<S>),
}

/// Block of statements - this is now defined in expr.rs as crate::expr::Block
pub use crate::expr::Block;
