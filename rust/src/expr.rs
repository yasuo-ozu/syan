use syan::{
    nested::group::{GroupBrace, GroupBracket, GroupParen},
    parse::{Parse, Unparse},
    span::WithSpan,
    symbol::Token,
};
use type_macro_derive_tricks::macro_derive;

/// A Rust expression  
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum Expr<S, Tokens> {
    Binary(ExprBinary<S, Tokens>),
    Unary(ExprUnary<S, Tokens>),
    Call(ExprCall<S, Tokens>),
    MethodCall(ExprMethodCall<S, Tokens>),
    Path(ExprPath<S>),
    Lit(ExprLit<S>),
    Block(ExprBlock<S>),
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

/// Binary expression
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprBinary<S, Tokens = std::convert::Infallible> {
    pub left: Box<Expr<S, Tokens>>,
    pub op: BinOp<S>,
    pub right: Box<Expr<S, Tokens>>,
}

/// Binary operator
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
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


/// Unary expression
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprUnary<S, Tokens = std::convert::Infallible> {
    pub op: UnOp<S>,
    pub expr: Box<Expr<S, Tokens>>,
}

/// Unary operator
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum UnOp<S> {
    Deref(Token![S => *]),
    Not(Token![S => !]),
    Neg(Token![S => -]),
}

/// Function call expression
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprCall<S, Tokens = std::convert::Infallible> {
    pub func: Box<Expr<S, Tokens>>,
    pub paren_token: GroupParen<(), S>,
    pub args: Vec<Expr<S, Tokens>>,
}

/// Method call expression
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprMethodCall<S, Tokens = std::convert::Infallible> {
    pub receiver: Box<Expr<S, Tokens>>,
    pub dot_token: Token![S => .],
    pub method: crate::Ident<S>,
    pub turbofish: Option<crate::AngleBracketedGenericArguments<S>>,
    pub paren_token: GroupParen<(), S>,
    pub args: Vec<Expr<S, Tokens>>,
}

/// Path expression (variable/function reference)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprPath<S> {
    pub qself: Option<QSelf<S>>,
    pub path: crate::Path<S>,
}

/// Qualified self type (for <T as Trait>::Item syntax)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct QSelf<S> {
    pub lt_token: Token![S => <],
    pub ty: Box<crate::Type<S>>,
    pub position: usize,
    pub as_token: Option<Token![S => as]>,
    pub gt_token: Token![S => >],
}

// TODO: Implement Spanned manually when derive macros are fixed

/// Literal expression
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprLit<S> {
    pub lit: crate::Lit<S>,
}

/// Block expression
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprBlock<S> {
    pub label: Option<Label<S>>,
    pub block: Block<S>,
}

/// Block label
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct Label<S> {
    pub name: crate::Lifetime<S>,
    pub colon_token: Token![S => :],
}

/// Code block
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct Block<S, Tokens = std::convert::Infallible> {
    pub brace_token: GroupBrace<(), S>,
    pub stmts: Vec<crate::Stmt<S, Tokens>>,
}
/// If expression
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprIf<S, Tokens = std::convert::Infallible> {
    pub if_token: Token![S => if],
    pub cond: Box<Expr<S, Tokens>>,
    pub then_branch: Block<S>,
    pub else_branch: Option<(Token![S => else], Box<Expr<S, Tokens>>)>,
}

/// Match expression
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprMatch<S, Tokens = std::convert::Infallible> {
    pub match_token: Token![S => match],
    pub expr: Box<Expr<S, Tokens>>,
    pub brace_token: GroupBrace<(), S>,
    pub arms: Vec<Arm<S>>,
}

/// Match arm
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct Arm<S, Tokens = std::convert::Infallible> {
    pub pat: crate::Pat<S>,
    pub guard: Option<(Token![S => if], Box<Expr<S, Tokens>>)>,
    pub fat_arrow_token: Token![S => =>],
    pub body: Box<Expr<S, Tokens>>,
    pub comma: Option<Token![S => ,]>,
}

/// Loop expression
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprLoop<S> {
    pub label: Option<Label<S>>,
    pub loop_token: Token![S => loop],
    pub block: Block<S>,
}

/// While expression
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprWhile<S, Tokens = std::convert::Infallible> {
    pub label: Option<Label<S>>,
    pub while_token: Token![S => while],
    pub cond: Box<Expr<S, Tokens>>,
    pub block: Block<S>,
}

/// For expression
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprFor<S, Tokens = std::convert::Infallible> {
    pub label: Option<Label<S>>,
    pub for_token: Token![S => for],
    pub pat: Box<crate::Pat<S>>,
    pub in_token: Token![S => in],
    pub iter: Box<Expr<S, Tokens>>,
    pub block: Block<S>,
}

/// Return expression
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprReturn<S, Tokens = std::convert::Infallible> {
    pub return_token: Token![S => return],
    pub expr: Option<Box<Expr<S, Tokens>>>,
}

/// Break expression
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprBreak<S, Tokens = std::convert::Infallible> {
    pub break_token: Token![S => break],
    pub label: Option<crate::Lifetime<S>>,
    pub expr: Option<Box<Expr<S, Tokens>>>,
}

/// Continue expression
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprContinue<S> {
    pub continue_token: Token![S => continue],
    pub label: Option<crate::Lifetime<S>>,
}

/// Parenthesized expression
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprParen<S, Tokens = std::convert::Infallible> {
    pub paren_token: GroupParen<(), S>,
    pub expr: Box<Expr<S, Tokens>>,
}

/// Index expression (array[index])
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprIndex<S, Tokens = std::convert::Infallible> {
    pub expr: Box<Expr<S, Tokens>>,
    pub bracket_token: GroupBracket<(), S>,
    pub index: Box<Expr<S, Tokens>>,
}

/// Field access expression (obj.field)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprField<S, Tokens = std::convert::Infallible> {
    pub base: Box<Expr<S, Tokens>>,
    pub dot_token: Token![S => .],
    pub member: ExprMember<S>,
}

/// Expression member of a struct (field name or tuple index)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum ExprMember<S> {
    Named(crate::Ident<S>),
    Unnamed(WithSpan<syan::source::proc_macro2::literal::Integer, S>),
}
/// Reference expression (&expr or &mut expr)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprReference<S, Tokens = std::convert::Infallible> {
    pub and_token: Token![S => &],
    pub mutability: Option<Token![S => mut]>,
    pub expr: Box<Expr<S, Tokens>>,
}

/// Array literal expression [1, 2, 3] or [1; N]
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprArray<S, Tokens = std::convert::Infallible> {
    pub bracket_token: GroupBracket<(), S>,
    pub elems: Vec<Expr<S, Tokens>>,
}

/// Tuple expression (1, 2, 3)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprTuple<S, Tokens = std::convert::Infallible> {
    pub paren_token: GroupParen<(), S>,
    pub elems: Vec<Expr<S, Tokens>>,
}

/// Struct literal expression Point { x: 1, y: 2 }
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprStruct<S, Tokens = std::convert::Infallible> {
    pub path: crate::Path<S>,
    pub brace_token: GroupBrace<(), S>,
    pub fields: Vec<FieldValue<S>>,
    pub dot2_token: Option<Token![S => ..]>,
    pub rest: Option<Box<Expr<S, Tokens>>>,
}

/// Field value in struct literal
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct FieldValue<S, Tokens = std::convert::Infallible> {
    pub member: ExprMember<S>,
    pub colon_token: Option<Token![S => :]>,
    pub expr: Expr<S, Tokens>,
}

/// Closure expression |x| x + 1
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprClosure<S, Tokens = std::convert::Infallible> {
    pub asyncness: Option<Token![S => async]>,
    pub movability: Option<Token![S => move]>,
    pub or1_token: Token![S => |],
    pub inputs: Vec<crate::Pat<S>>,
    pub or2_token: Token![S => |],
    pub output: Option<(Token![S => ->], Box<crate::Type<S>>)>,
    pub body: Box<Expr<S, Tokens>>,
}

/// Async block expression
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprAsync<S> {
    pub async_token: Token![S => async],
    pub capture: Option<Token![S => move]>,
    pub block: Block<S>,
}

/// Await expression (expr.await)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprAwait<S, Tokens = std::convert::Infallible> {
    pub base: Box<Expr<S, Tokens>>,
    pub dot_token: Token![S => .],
    pub await_token: Token![S => await],
}

/// Try expression (expr?)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprTry<S, Tokens = std::convert::Infallible> {
    pub expr: Box<Expr<S, Tokens>>,
    pub question_token: Token![S => ?],
}

/// Assignment expression (x = y)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprAssign<S, Tokens = std::convert::Infallible> {
    pub left: Box<Expr<S, Tokens>>,
    pub eq_token: Token![S => =],
    pub right: Box<Expr<S, Tokens>>,
}

/// Assignment with operator expression (x += y)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprAssignOp<S, Tokens = std::convert::Infallible> {
    pub left: Box<Expr<S, Tokens>>,
    pub op: BinOp<S>,
    pub eq_token: Token![S => =],
    pub right: Box<Expr<S, Tokens>>,
}
/// Range expression (1..10, 1..=10, .., ..10)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprRange<S, Tokens = std::convert::Infallible> {
    pub from: Option<Box<Expr<S, Tokens>>>,
    pub limits: RangeLimits<S>,
    pub to: Option<Box<Expr<S, Tokens>>>,
}

/// Range limits
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum RangeLimits<S> {
    HalfOpen(Token![S => ..]),
    Closed(Token![S => ..=]),
}

/// Cast expression (expr as Type)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprCast<S, Tokens = std::convert::Infallible> {
    pub expr: Box<Expr<S, Tokens>>,
    pub as_token: Token![S => as],
    pub ty: Box<crate::Type<S>>,
}

/// Type ascription expression (expr: Type)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprType<S, Tokens = std::convert::Infallible> {
    pub expr: Box<Expr<S, Tokens>>,
    pub colon_token: Token![S => :],
    pub ty: Box<crate::Type<S>>,
}

/// Let expression (let pat = expr)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprLet<S, Tokens = std::convert::Infallible> {
    pub let_token: Token![S => let],
    pub pat: crate::Pat<S>,
    pub eq_token: Token![S => =],
    pub expr: Box<Expr<S, Tokens>>,
}

/// Macro call expression
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprMacro<S, Tokens = std::convert::Infallible> {
    pub mac: Macro<S, Tokens>,
}

/// Macro call
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct Macro<S, Tokens = std::convert::Infallible> {
    pub path: crate::Path<S>,
    pub bang_token: Token![S => !],
    pub delimiter: ExprMacroDelimiter<S>,
    pub tokens: Tokens,
}

/// Expression macro delimiter
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum ExprMacroDelimiter<S> {
    Paren(syan::nested::group::GroupParen<(), S>),
    Brace(syan::nested::group::GroupBrace<(), S>),
    Bracket(syan::nested::group::GroupBracket<(), S>),
}

/// Unsafe block expression
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprUnsafe<S> {
    pub unsafe_token: Token![S => unsafe],
    pub block: Block<S>,
}

// Additional expressions from rustc_ast

/// Repeat expression ([value; count])
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprRepeat<S, Tokens = std::convert::Infallible> {
    pub bracket_token: GroupBracket<(), S>,
    pub expr: Box<Expr<S, Tokens>>,
    pub semi_token: Token![S => ;],
    pub len: Box<Expr<S, Tokens>>,
}

/// Generator expression (gen { ... } or async gen { ... })
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprGen<S> {
    pub capture: Option<Token![S => move]>,
    pub kind: GeneratorKind<S>,
    pub block: Block<S>,
}

/// Generator kind
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum GeneratorKind<S> {
    Gen(Token![S => gen]),
    Async(Token![S => async]),
    AsyncGen(Token![S => async], Token![S => gen]),
}

/// Try block expression (try { ... })
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprTryBlock<S> {
    pub try_token: Token![S => try],
    pub block: Block<S>,
}

/// Yield expression (yield value)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprYield<S, Tokens = std::convert::Infallible> {
    pub yield_token: Token![S => yield],
    pub expr: Option<Box<Expr<S, Tokens>>>,
}

/// Yeet expression (do yeet value)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprYeet<S, Tokens = std::convert::Infallible> {
    pub do_token: Token![S => do],
    pub yeet_token: Token![S => yeet],
    pub expr: Option<Box<Expr<S, Tokens>>>,
}

/// Become expression (become expr)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprBecome<S, Tokens = std::convert::Infallible> {
    pub become_token: Token![S => become],
    pub expr: Box<Expr<S, Tokens>>,
}

/// Included bytes expression
#[derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprIncludedBytes {
    pub bytes: Vec<u8>,
}

/// Format arguments expression
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprFormatArgs<S> {
    pub template: Vec<FormatArgsPiece<S>>,
    pub arguments: FormatArguments<S>,
}

/// Format arguments piece
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum FormatArgsPiece<S> {
    Literal(crate::Lit<S>),
    //Placeholder(FormatPlaceholder<S>),
}

/// Format placeholder
// #[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
// pub struct FormatPlaceholder<S> {
//     pub argument: FormatArgument<S>,
//     pub format_trait: FormatTrait,
//     pub format_options: FormatOptions<S>,
// }

/// Format argument
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum FormatArgument<S, Tokens = std::convert::Infallible> {
    Normal(Expr<S, Tokens>),
    Named(crate::Ident<S>, Expr<S, Tokens>),
}

/// Format trait
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FormatTrait {
    Display,
    Debug,
    LowerExp,
    UpperExp,
    Octal,
    Pointer,
    Binary,
    LowerHex,
    UpperHex,
}

/// Format options
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct FormatOptions<S> {
    pub width: Option<FormatCount<S>>,
    pub precision: Option<FormatCount<S>>,
    pub alignment: Option<FormatAlignment>,
    pub flags: FormatFlags,
}

/// Format count
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum FormatCount<S> {
    Literal(usize),
    Argument(crate::Ident<S>),
    Star,
}

/// Format alignment
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FormatAlignment {
    Left,
    Right,
    Center,
}

/// Format flags
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FormatFlags {
    pub alternate: bool,
    pub zero_pad: bool,
    pub plus: bool,
    pub minus: bool,
}

/// Format arguments
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct FormatArguments<S> {
    pub arguments: Vec<FormatArgument<S>>,
}

/// OffsetOf expression (offset_of!(Type, field))
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprOffsetOf<S> {
    pub offset_of_token: Token![S => offset_of],
    pub bang_token: Token![S => !],
    pub paren_token: GroupParen<(), S>,
    pub container: Box<crate::Type<S>>,
    pub comma_token: Token![S => ,],
    pub fields: Vec<OffsetOfField<S>>,
}

/// Field in offset_of
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum OffsetOfField<S> {
    Named(crate::Ident<S>),
    Index(WithSpan<syan::source::proc_macro2::literal::Integer, S>),
}

/// Const block expression (const { ... })
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprConstBlock<S> {
    pub const_token: Token![S => const],
    pub block: Block<S>,
}

/// Inline assembly expression
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprInlineAsm<S> {
    pub asm_token: Token![S => asm],
    pub bang_token: Token![S => !],
    pub paren_token: GroupParen<(), S>,
    pub template: Vec<InlineAsmTemplatePiece<S>>,
    pub operands: Vec<InlineAsmOperand<S>>,
    pub options: InlineAsmOptions,
}

/// Inline assembly template piece
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum InlineAsmTemplatePiece<S> {
    String(crate::Lit<S>),
    Placeholder(InlineAsmPlaceholder<S>),
}

/// Inline assembly placeholder
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct InlineAsmPlaceholder<S> {
    pub operand_idx: usize,
    pub modifier: Option<char>,
    pub span: S,
}

/// Inline assembly operand
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum InlineAsmOperand<S, Tokens = std::convert::Infallible> {
    In(InlineAsmRegOrRegClass<S>, Expr<S, Tokens>),
    Out(InlineAsmRegOrRegClass<S>, Option<Expr<S, Tokens>>),
    InOut(
        InlineAsmRegOrRegClass<S>,
        Expr<S, Tokens>,
        Option<Expr<S, Tokens>>,
    ),
    SplitInOut(
        InlineAsmRegOrRegClass<S>,
        Expr<S, Tokens>,
        Option<Expr<S, Tokens>>,
    ),
    Const(Expr<S, Tokens>),
    Sym(Expr<S, Tokens>),
    Label(Block<S, Tokens>),
}

/// Inline assembly register or register class
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum InlineAsmRegOrRegClass<S> {
    Reg(crate::Ident<S>),
    RegClass(crate::Ident<S>),
}

/// Inline assembly options
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InlineAsmOptions {
    pub pure: bool,
    pub nomem: bool,
    pub readonly: bool,
    pub preserves_flags: bool,
    pub noreturn: bool,
    pub nostack: bool,
    pub att_syntax: bool,
    pub raw: bool,
    pub may_unwind: bool,
}

/// Underscore expression (_)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprUnderscore<S> {
    pub underscore_token: Token![S => _],
}

/// Error expression (for error recovery)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprErr<S> {
    pub span: S,
}
