use crate::tokens::{Ident, Label, Lit, RangeLimits};
use syan::nested::group::{GroupAngle, GroupBrace, GroupParen};
use syan::nested::punctuated::Punctuated;
use syan::parse::{Parse, Unparse};
use syan::symbol::Token;
use type_macro_derive_tricks::macro_derive;

#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct Path<S> {
    heading: PathHeading<S>,
    segments: Punctuated<PathSegment<S>, Token![S => ::]>,
}

#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum PathHeading<S> {
    QSelf {
        angle_token: GroupAngle<(), S>,
        #[group(self.angle_token)]
        ty: Type<S>,
        #[group(self.angle_token)]
        as_trait: Option<(
            Token![S => as],
            Option<Token![S => ::]>,
            Punctuated<PathSegment<S>, Token![S => ::]>,
        )>,
        semisemi_token: Token![S => ::],
    },
    Raw {
        semisemi_token: Option<Token![S => ::]>,
    },
}

#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct PathSegment<S> {
    pub ident: Ident<S>,
    pub args: Option<GenericArguments<S>>,
}

#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum Pat<S> {
    Ident(Ident<S>),
    Struct {
        path: Path<S>,
        brace_token: GroupBrace<(), S>,
        #[group(self.brace_token)]
        fields: Punctuated<(Ident<S>, Option<(Token![S => :], Pat<S>)>), Token![S => ,]>,
        #[group(self.brace_token)]
        rest: Option<(Token![S => ,], Token![S => ..])>,
    },
    TupleStruct {
        path: Path<S>,
        paren_token: GroupParen<(), S>,
        #[group(self.paren_token)]
        elems: Punctuated<Pat<S>, Token![S => ,]>,
        #[group(self.paren_token)]
        trailing_comma: Option<Token![S => ,]>,
    },
    Path(Path<S>),
    Tuple {
        paren_token: GroupParen<(), S>,
        #[group(self.paren_token)]
        elems: Punctuated<Pat<S>, Token![S => ,]>,
        #[group(self.paren_token)]
        trailing_comma: Option<Token![S => ,]>,
    },
    Box {
        box_token: Token![S => box],
        pat: Box<Pat<S>>,
    },
    Ref {
        and_token: Token![S => &],
        mutability: Option<Token![S => mut]>,
        pat: Box<Pat<S>>,
    },
    Lit(Box<Expr<S>>),
    Range {
        lo: Option<Box<Expr<S, Tokens>>>,
        limits: RangeLimits<S>,
        hi: Option<Box<Expr<S, Tokens>>>,
    },
    Slice {
        bracket_token: GroupBracket<(), S>,
        #[group(self.bracket_token)]
        elems: Punctuated<Pat<S>, Token![S => ,]>,
        trailing_comma: Option<Token![S => ,]>,
    },
    Rest(Token! {S => ..}),
    Paren {
        paren_token: GroupParen<(), S>,
        #[group(self.paren_token)]
        pat: Box<Pat<S>>,
    },
    // Macro
    Or(Punctuated<Pat<S>, Token![S => |]>),
    Never(Token![S => !]),
}
