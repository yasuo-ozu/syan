use crate::error::ParseError;
use crate::parse::unparse::Emitter;
use crate::parse::{IntoParseStream, Parse, Unparse};
use crate::span::{Span, Spanned};
use core::marker::PhantomData;
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Group<T, O, C> {
    pub open: O,
    pub slot: T,
    pub close: C,
}

// A group's span is the range its delimiters cover; the slot's content lies *between* `open` and
// `close`, so the span is taken from the delimiters only. This deliberately does NOT require `T:
// Spanned`, so an empty group (`Group<(), ..>`) — or one whose content isn't `Spanned` — still has a
// span (the hand-written impl replaces what `#[derive(Spanned)]` would emit, which folds the slot too).
impl<T, O, C> Spanned for Group<T, O, C>
where
    O: Spanned,
    C: Spanned<Span = O::Span>,
{
    type Span = O::Span;

    fn span(&self) -> Self::Span {
        crate::span::Span::migrate(self.open.span(), self.close.span())
    }
}

// The **flat-family** impls: `open`, `slot`, `close` are three ordinary atoms of the same stream, so a
// group parses/unparses delimiter-wise. This is the family a flat-token downstream (whose `(` and `)`
// are separate leaf atoms) uses — it supplies its own open/close leaf types and gets both impls from
// core, no orphan impl of its own. Coherence with the **tree-family** impls (`source::proc_macro2`,
// where a delimited group is a single atom keyed on `TOpen`/`TClose`) rests on those tree carriers
// implementing neither `Parse` nor `Unparse` for any atom: the compiler proves the `O: Parse`/`O:
// Unparse` bound here can never hold for a `TOpen` head (a crate-local type downstream cannot extend),
// so the two families never overlap.
impl<Atom, T, O, C> Parse<Atom> for Group<T, O, C>
where
    T: Parse<Atom>,
    O: Parse<Atom>,
    C: Parse<Atom>,
{
    type Error = ParseError;

    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        let open = O::parse(&mut stream).map_err(crate::error::Error::into_parse_error)?;
        let slot = T::parse(&mut stream).map_err(crate::error::Error::into_parse_error)?;
        let close = C::parse(&mut stream).map_err(crate::error::Error::into_parse_error)?;
        Ok(Group { open, slot, close })
    }
}

impl<Atom, T, O, C> Unparse<Atom> for Group<T, O, C>
where
    T: Unparse<Atom>,
    O: Unparse<Atom>,
    C: Unparse<Atom>,
{
    fn unparse<E: Emitter<Atom>>(&self, sink: &mut E) -> Result<(), E::Error> {
        self.open.unparse(sink)?;
        self.slot.unparse(sink)?;
        self.close.unparse(sink)
    }
}

/// The delimiter kind of a **tree-family** [`Group`] carrier ([`TOpen`]/[`TClose`]).
///
/// Implemented by the zero-sized markers [`Paren`], [`Brace`], and [`Bracket`]. A tree-shaped atom
/// (one whose delimited groups arrive as a *single* atom, e.g. `proc_macro2::TokenTree`) keys its
/// `Group` `Parse`/`Unparse` impls on this marker instead of on standalone open/close leaf tokens, so
/// one delimiter-generic impl serves every delimiter.
pub trait Delim {
    /// The opening delimiter character.
    const OPEN: char;
    /// The closing delimiter character.
    const CLOSE: char;
}

macro_rules! define_delim {
    ($(#[doc = $doc:literal] $name:ident = $open:literal .. $close:literal;)*) => {
        $(
            #[doc = $doc]
            #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
            pub struct $name;

            impl Delim for $name {
                const OPEN: char = $open;
                const CLOSE: char = $close;
            }
        )*
    };
}

define_delim! {
    #[doc = "The `(`…`)` delimiter marker (see [`GroupParen`])."]
    Paren = '(' .. ')';
    #[doc = "The `{`…`}` delimiter marker (see [`GroupBrace`])."]
    Brace = '{' .. '}';
    #[doc = "The `[`…`]` delimiter marker (see [`GroupBracket`])."]
    Bracket = '[' .. ']';
}

macro_rules! define_carrier {
    ($carrier:ident, $ch:ident, $doc:literal) => {
        #[doc = $doc]
        ///
        /// It deliberately implements neither `Parse` nor `Unparse` for any atom: the whole tree-family
        /// [`Group`] is parsed/unparsed as one atom (see `source::proc_macro2`), and that absence is what
        /// keeps the tree-family `Group` impls disjoint from the flat-family blankets. Equality and hash
        /// key on the (fixed) delimiter, ignoring the span.
        pub struct $carrier<D, S> {
            /// The span the delimiter covers.
            pub span: S,
            _delim: PhantomData<D>,
        }

        impl<D, S> $carrier<D, S> {
            /// Wrap `span` in this delimiter carrier.
            pub const fn new(span: S) -> Self {
                Self {
                    span,
                    _delim: PhantomData,
                }
            }
        }

        impl<D, S: Default> Default for $carrier<D, S> {
            fn default() -> Self {
                Self::new(S::default())
            }
        }

        impl<D, S: Clone> Clone for $carrier<D, S> {
            fn clone(&self) -> Self {
                Self::new(self.span.clone())
            }
        }

        impl<D, S: core::fmt::Debug> core::fmt::Debug for $carrier<D, S> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_struct(stringify!($carrier))
                    .field("span", &self.span)
                    .finish()
            }
        }

        impl<D, S> PartialEq for $carrier<D, S> {
            fn eq(&self, _: &Self) -> bool {
                true
            }
        }

        impl<D, S> Eq for $carrier<D, S> {}

        impl<D, S> PartialOrd for $carrier<D, S> {
            fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl<D, S> Ord for $carrier<D, S> {
            fn cmp(&self, _: &Self) -> core::cmp::Ordering {
                core::cmp::Ordering::Equal
            }
        }

        impl<D, S> core::hash::Hash for $carrier<D, S> {
            fn hash<H: core::hash::Hasher>(&self, _: &mut H) {}
        }

        impl<D: Delim, S> core::fmt::Display for $carrier<D, S> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, "{}", D::$ch)
            }
        }

        impl<D, S: Span> Spanned for $carrier<D, S> {
            type Span = S;

            fn span(&self) -> Self::Span {
                self.span.clone()
            }
        }
    };
}

define_carrier!(
    TOpen,
    OPEN,
    "The opening carrier of a tree-family [`Group`]: a spanned, delimiter-tagged placeholder that is *never* a standalone atom."
);
define_carrier!(
    TClose,
    CLOSE,
    "The closing carrier of a tree-family [`Group`]: a spanned, delimiter-tagged placeholder that is *never* a standalone atom."
);

pub type GroupParen<T, S> = Group<T, TOpen<Paren, S>, TClose<Paren, S>>;
pub type GroupBrace<T, S> = Group<T, TOpen<Brace, S>, TClose<Brace, S>>;
pub type GroupBracket<T, S> = Group<T, TOpen<Bracket, S>, TClose<Bracket, S>>;

pub trait EmptyGroup {
    type Fill<Slot>;

    fn fill<Slot>(self, slot: Slot) -> Self::Fill<Slot>;
    fn unfill<Slot>(group: Self::Fill<Slot>) -> (Slot, Self);
}

impl<T, O, C> std::fmt::Display for Group<T, O, C>
where
    T: std::fmt::Display,
    O: std::fmt::Display,
    C: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.open.fmt(f)?;
        self.slot.fmt(f)?;
        self.close.fmt(f)
    }
}

impl<T, O, C> std::ops::Deref for Group<T, O, C> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.slot
    }
}

impl<O, C> EmptyGroup for Group<(), O, C> {
    type Fill<Slot> = Group<Slot, O, C>;

    fn unfill<Slot>(group: Self::Fill<Slot>) -> (Slot, Self) {
        (
            group.slot,
            Group {
                slot: (),
                open: group.open,
                close: group.close,
            },
        )
    }
    fn fill<Slot>(self, slot: Slot) -> Self::Fill<Slot> {
        Group {
            slot,
            open: self.open,
            close: self.close,
        }
    }
}
