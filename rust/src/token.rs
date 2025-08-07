use syan::span::{Span, Spanned};

/// Identifier
#[derive(Debug, Clone)]
pub struct Ident<S: Span> {
    pub sym: String,
    pub span: S,
}

impl<S: Span> Spanned for Ident<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.span.clone()
    }
}

// Token types - each represents a specific punctuation or keyword
macro_rules! define_token {
    ($name:ident, $display:literal) => {
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
        
        impl<S: Span> std::fmt::Display for $name<S> {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, $display)
            }
        }
    };
}

// Punctuation tokens
define_token!(PlusToken, "+");
define_token!(MinusToken, "-");
define_token!(StarToken, "*");
define_token!(SlashToken, "/");
define_token!(PercentToken, "%");
define_token!(CaretToken, "^");
define_token!(NotToken, "!");
define_token!(AndToken, "&");
define_token!(OrToken, "|");
define_token!(AndAndToken, "&&");
define_token!(OrOrToken, "||");
define_token!(ShlToken, "<<");
define_token!(ShrToken, ">>");
define_token!(LtLtToken, "<<");
define_token!(GtGtToken, ">>");
define_token!(PlusEqToken, "+=");
define_token!(MinusEqToken, "-=");
define_token!(StarEqToken, "*=");
define_token!(SlashEqToken, "/=");
define_token!(PercentEqToken, "%=");
define_token!(CaretEqToken, "^=");
define_token!(AndEqToken, "&=");
define_token!(OrEqToken, "|=");
define_token!(ShlEqToken, "<<=");
define_token!(ShrEqToken, ">>=");
define_token!(EqToken, "=");
define_token!(EqEqToken, "==");
define_token!(NeToken, "!=");
define_token!(GtToken, ">");
define_token!(LtToken, "<");
define_token!(GeToken, ">=");
define_token!(LeToken, "<=");
define_token!(AtToken, "@");
define_token!(UnderscoreToken, "_");
define_token!(DotToken, ".");
define_token!(DotDotToken, "..");
define_token!(DotDotDotToken, "...");
define_token!(DotDotEqToken, "..=");
define_token!(Dot2Token, "..");
define_token!(CommaToken, ",");
define_token!(SemiToken, ";");
define_token!(ColonToken, ":");
define_token!(ColonColonToken, "::");
define_token!(RArrowToken, "->");
define_token!(LArrowToken, "<-");
define_token!(FatArrowToken, "=>");
define_token!(PoundToken, "#");
define_token!(DollarToken, "$");
define_token!(QuestionToken, "?");
define_token!(ApostropheToken, "'");

// Delimiter tokens
#[derive(Debug, Clone)]
pub struct ParenToken<S: Span> {
    pub span: S,
}

impl<S: Span> Spanned for ParenToken<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.span.clone()
    }
}

#[derive(Debug, Clone)]
pub struct BraceToken<S: Span> {
    pub span: S,
}

impl<S: Span> Spanned for BraceToken<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.span.clone()
    }
}

#[derive(Debug, Clone)]
pub struct BracketToken<S: Span> {
    pub span: S,
}

impl<S: Span> Spanned for BracketToken<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.span.clone()
    }
}

// Keyword tokens
define_token!(AbstractToken, "abstract");
define_token!(AsToken, "as");
define_token!(AsyncToken, "async");
define_token!(AwaitToken, "await");
define_token!(BecomeToken, "become");
define_token!(BoxToken, "box");
define_token!(BreakToken, "break");
define_token!(ConstToken, "const");
define_token!(ContinueToken, "continue");
define_token!(CrateToken, "crate");
define_token!(DoToken, "do");
define_token!(DynToken, "dyn");
define_token!(ElseToken, "else");
define_token!(EnumToken, "enum");
define_token!(ExternToken, "extern");
define_token!(FalseToken, "false");
define_token!(FinalToken, "final");
define_token!(FnToken, "fn");
define_token!(ForToken, "for");
define_token!(IfToken, "if");
define_token!(ImplToken, "impl");
define_token!(InToken, "in");
define_token!(LetToken, "let");
define_token!(LoopToken, "loop");
define_token!(MacroToken, "macro");
define_token!(MatchToken, "match");
define_token!(ModToken, "mod");
define_token!(MoveToken, "move");
define_token!(MutToken, "mut");
define_token!(OverrideToken, "override");
define_token!(PrivToken, "priv");
define_token!(PubToken, "pub");
define_token!(RefToken, "ref");
define_token!(ReturnToken, "return");
define_token!(SelfToken, "self");
define_token!(SelfTypeToken, "Self");
define_token!(StaticToken, "static");
define_token!(StructToken, "struct");
define_token!(SuperToken, "super");
define_token!(TraitToken, "trait");
define_token!(TrueToken, "true");
define_token!(TryToken, "try");
define_token!(TypeToken, "type");
define_token!(UnionToken, "union");
define_token!(UnsafeToken, "unsafe");
define_token!(UnsizedToken, "unsized");
define_token!(UseToken, "use");
define_token!(VirtualToken, "virtual");
define_token!(WhereToken, "where");
define_token!(WhileToken, "while");
define_token!(YieldToken, "yield");
define_token!(NeverToken, "!");