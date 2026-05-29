use syan::parse::recurse;

#[recurse]
mod rec {
    use crate::syntax::term::Ident;
    use syan::nested::group::{GroupAngle, GroupBrace, GroupParen};
    use syan::nested::punctuated::Punctuated;
    use syan::parse::{Parse, Unparse};
    use syan::symbol::Token;
    use type_macro_derive_tricks::macro_derive;

    #[macro_derive(Clone, Debug, Hash, Parse)]
    pub enum Item<S> {
        ExternCrate {
            heading: (Token![S => extern], Token![S => crate]),
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
            unsafety: Option<Token![S => unsafe]>,
            static_token: Token![S => static],
            mutability: Option<Token![S => mut]>,
            ident: Ident<S>,
            colon_token: Token![S => :],
            ty: Box<Type<S>>,
            eq_token: Token![S => =],
            expr: Box<Expr<S>>,
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
        Fn {
            defaultness: Option<Token![S => default]>,
            // TODO: sig
            brace_token: GroupBrace<(), S>,
            #[group(self.brace_token)]
            stmts: Vec<Stmt<S>>,
        },
        Mod {
            unsafety: Option<Token![S => unsafe]>,
            mod_token: Token![S => mod],
            ident: Ident<S>,
            content: ModContent<S>,
        },
        ForeignMod {
            unsafety: Option<Token![S => unsafe]>,
            extern_token: Token![S => extern],
            explicit_abi: Option<LitString<S>>,
            brace_token: GroupBrace<(), S>,
            #[group(self.brace_token)]
            items: Vec<ForeignItem<S>>,
        },
        GlobalAsm {
            global_asm_token: Token![S => global_asm],
            bang_token: Token![S => !],
            paren_token: GroupParen<(), S>,
            #[group(self.paren_token)]
            template: LitString<S>,
            options: Vec<GlobalAsmOptions<S>>,
        },
        TyAlias {
            defaultness: Option<Token![S => default]>,
            type_token: Token![S => type],
            ident: Ident<S>,
            generics: Option<Generics<S>>,
            where_clause: Option<(
                Token![S => where],
                Punctuated<WherePredicate<S>, Token![S => ,]>,
                Option<Token![S => ,]>,
            )>,
            bounds: Vec<TypeParamBound<S>>,
            ty: Option<(Token![S => =], Box<Type<S>>)>,
            semi_token: Token![S => ;],
        },
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

    /// Global assembly options
    /// Examples: att_syntax, intel_syntax, options("raw")
    #[macro_derive(Clone, Debug, Hash, Parse)]
    pub enum GlobalAsmOptions<S> {
        Att,
        Intel,
        Options(crate::Lit<S>),
    }

    /// Foreign item
    /// Examples: extern functions, types, statics in extern blocks
    #[macro_derive(Clone, Debug, Hash, Parse)]
    pub enum ForeignItem<S> {
        Static {
            static_token: Token![S => static],
            mutability: Option<Token![S => mut]>,
            ident: Ident<S>,
            colon_token: Token![S => :],
            ty: Box<Type<S>>,
            semi_token: Token![S => ;],
        },
        Fn {
            // sig
            semi_token: Token![S => ;],
        },
        TyAlias {
            type_token: Token![S => type],
            ident: Ident<S>,
            generics: Option<Generics<S>>,
            where_clause: Option<(Token![S => where], Vec<WherePredicate<S>>)>,
            semi_token: Token![S => ;],
        },
        MacCall(),
    }

    #[macro_derive(Clone, Debug, Hash, Parse)]
    pub enum ModContent<S> {
        Content {
            angle_token: GroupAngle<(), S>,
            #[group(self.angle_token)]
            items: Vec<Item<S>>,
        },
        NoContent {
            semi_token: Token![S => ;],
        },
    }

    /// Use tree structure
    /// Examples: std::vec::Vec, std::*, std::{Vec, HashMap}, std::vec::Vec as Vector
    #[macro_derive(Clone, Debug, Hash, Parse)]
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
            contents: Punctuated<UseTree<S>, Token![S => ,]>,
        },
        Prefixed {
            prefix: crate::Path<S>,
            semi_token: Token![S => ::],
            tree: Box<UseTree<S>>,
        },
    }

    #[macro_derive(Clone, Debug, Hash, Parse)]
    pub enum Expr<S> {
        Binary {
            left: Box<Expr<S>>,
            op: BinOp<S>,
            right: Box<Expr<S>>,
        },
        Unary {
            op: UnOp<S>,
            expr: Box<Expr<S>>,
        },
        Call {
            func: Box<Expr<S>>,
            paren_token: GroupParen<(), S>,
            #[group(self.paren_token)]
            args: Punctuated<Expr<S>, Token![S => ,]>,
            #[group(self.paren_token)]
            trailing_comma: Option<Token![S => ,]>,
        },
        MethodCall {
            receiver: Box<Expr<S>>,
            dot_token: Token![S => .],
            method: Ident<S>,
            // TODO: Generics?
            paren_token: GroupParen<(), S>,
            #[group(self.paren_token)]
            args: Punctuated<Expr<S>, Token![S => ,]>,
            #[group(self.paren_token)]
            trailing_comma: Option<Token![S => ,]>,
        },
        Path(Path<S>),
        Lit(Lit<S>),
        Block {
            label: Option<Label<S>>,
            brace_token: GroupBrace<(), S>,
            stmts: Vec<Stmt<S>>,
        },
        If(ExprIf<S, Tokens>),
        Match(ExprMatch<S, Tokens>),
        Loop(ExprLoop<S>),
        While(ExprWhile<S, Tokens>),
        For(ExprFor<S, Tokens>),
        Return(ExprReturn<S, Tokens>),
        Break(ExprBreak<S, Tokens>),
        Continue(ExprContinue<S>),
        Paren(ExprParen<S, Tokens>),
        Index(ExprIndex<S, Tokens>),
        Field(ExprField<S, Tokens>),
        Reference(ExprReference<S, Tokens>),
        Array(ExprArray<S, Tokens>),
        Tuple(ExprTuple<S, Tokens>),
        Struct(ExprStruct<S, Tokens>),
        Closure(ExprClosure<S, Tokens>),
        Async(ExprAsync<S>),
        Await(ExprAwait<S, Tokens>),
        Try(ExprTry<S, Tokens>),
        Assign(ExprAssign<S, Tokens>),
        AssignOp(ExprAssignOp<S, Tokens>),
        Range(ExprRange<S, Tokens>),
        Cast(ExprCast<S, Tokens>),
        Type(ExprType<S, Tokens>),
        Let(ExprLet<S, Tokens>),
        Macro(ExprMacro<S, Tokens>),
        Unsafe(ExprUnsafe<S>),
        // Additional expressions from rustc_ast
        Repeat(ExprRepeat<S, Tokens>),
        Gen(ExprGen<S>),
        TryBlock(ExprTryBlock<S>),
        Yield(ExprYield<S, Tokens>),
        Yeet(ExprYeet<S, Tokens>),
        Become(ExprBecome<S, Tokens>),
        IncludedBytes(ExprIncludedBytes),
        FormatArgs(ExprFormatArgs<S>),
        OffsetOf(ExprOffsetOf<S>),
        ConstBlock(ExprConstBlock<S>),
        InlineAsm(ExprInlineAsm<S>),
        Underscore(ExprUnderscore<S>),
        Err(ExprErr<S>),
    }

    #[macro_derive(Clone, Debug, Hash, Parse)]
    pub enum BinOp<S> {
        Add(Token![S => +]),
        Sub(Token![S => -]),
        Mul(Token![S => *]),
        Div(Token![S => /]),
        Rem(Token![S => %]),
        And(Token![S => &&]),
        Or(Token![S => ||]),
        BitXor(Token![S => ^]),
        BitAnd(Token![S => &]),
        BitOr(Token![S => |]),
        Shl(Token![S => <<]),
        Shr(Token![S => >>]),
        Eq(Token![S => ==]),
        Lt(Token![S => <]),
        Le(Token![S => <=]),
        Ne(Token![S => !=]),
        Ge(Token![S => >=]),
        Gt(Token![S => >]),
    }

    #[macro_derive(Clone, Debug, Hash, Parse)]
    pub enum UnOp<S> {
        Deref(Token![S => *]),
        Not(Token![S => !]),
        Neg(Token![S => -]),
    }

    #[macro_derive(Clone, Debug, Hash, Parse)]
    pub enum Stmt<S> {
        Local {
            let_token: Token![S => let],
            pat: Pat<S>,
            ty: Option<(Token![S => :], Type<S>)>,
            init: Option<(
                Token![S => =],
                Expr<S>,
                Option<(Token![S => else], GroupBrace<Vec<Stmt<S>>, S>)>,
            )>,
            semi_token: Token![S => ;],
        },
        Item(Item<S>),
        Expr(Expr<S>),
    }
}

pub use rec::*;
