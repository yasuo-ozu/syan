use crate::expr::Expr;
use crate::path::Path;
use syan::{
    nested::group::{GroupBrace, GroupBracket, GroupParen},
    parse::{Parse, Unparse},
    symbol::Token,
};
use type_macro_derive_tricks::macro_derive;

/// An attribute like ``
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct Attribute<S, Tokens = std::convert::Infallible> {
    pub pound_token: Token![S => #],
    pub style: AttrStyle<S>,
    pub bracket_token: GroupBracket<(), S>,
    // TODO: safety
    #[group(self.bracket_token)]
    pub path: Path<S, Tokens>,
    #[group(self.bracket_token)]
    pub args: AttrArgs<S, Tokens>,
}

/// Attribute style (outer vs inner)
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum AttrStyle<S> {
    Outer,
    Inner(Token![S => !]),
}

/// Attribute metadata
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum AttrArgs<S, Tokens = std::convert::Infallible> {
    Delim {
        delim: AttrMacroDelimiter<(), S>,
        #[group(self.delim)]
        tokens: Tokens,
    },
    Eq {
        eq_token: Token![S => =],
        expr: Expr<S, Tokens>,
    },
    Empty,
}

/// Attribute macro delimiter
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum AttrMacroDelimiter<T, S> {
    Paren(GroupParen<T, S>),
    Brace(GroupBrace<T, S>),
    Bracket(GroupBracket<T, S>),
}

impl<S> syan::nested::group::EmptyGroup for AttrMacroDelimiter<(), S> {
    type Fill<Slot> = AttrMacroDelimiter<Slot, S>;

    fn unfill<Slot>(group: Self::Fill<Slot>) -> (Slot, Self) {
        match group {
            AttrMacroDelimiter::Paren(paren) => {
                let (slot, empty_paren) = syan::nested::group::EmptyGroup::unfill(paren);
                (slot, AttrMacroDelimiter::Paren(empty_paren))
            }
            AttrMacroDelimiter::Brace(brace) => {
                let (slot, empty_brace) = syan::nested::group::EmptyGroup::unfill(brace);
                (slot, AttrMacroDelimiter::Brace(empty_brace))
            }
            AttrMacroDelimiter::Bracket(bracket) => {
                let (slot, empty_bracket) = syan::nested::group::EmptyGroup::unfill(bracket);
                (slot, AttrMacroDelimiter::Bracket(empty_bracket))
            }
        }
    }

    fn fill<Slot>(self, slot: Slot) -> Self::Fill<Slot> {
        match self {
            AttrMacroDelimiter::Paren(paren) => {
                AttrMacroDelimiter::Paren(syan::nested::group::EmptyGroup::fill(paren, slot))
            }
            AttrMacroDelimiter::Brace(brace) => {
                AttrMacroDelimiter::Brace(syan::nested::group::EmptyGroup::fill(brace, slot))
            }
            AttrMacroDelimiter::Bracket(bracket) => {
                AttrMacroDelimiter::Bracket(syan::nested::group::EmptyGroup::fill(bracket, slot))
            }
        }
    }
}
