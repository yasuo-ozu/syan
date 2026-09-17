//! [`Ident`]: the general word leaf a text grammar needs before anything else — opcodes, symbol
//! names, block labels.
//!
//! Which characters count is a type parameter, so a grammar picks [`Rust`], [`Dotted`], or its own
//! [`IdentClass`]. Fixed words (keywords, punctuation) are already covered by
//! [`Symbol!`](macro@crate::symbol::Symbol) and [`Token!`](macro@crate::symbol::Token), so only the
//! general case lives here.

use super::parse_text_more::{byte_atom, char_atom, Scan};
use crate::error::ParseError;
use crate::parse::{Parse, ParseStream};
use crate::span::{Span, Spanned, WithSpan};
use core::marker::PhantomData;

/// Which characters an [`Ident`] is spelled with.
///
/// ```
/// use syan::literal::ident::{Ident, IdentClass};
/// use syan::parse::Parse;
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// struct Opcode;
///
/// impl IdentClass for Opcode {
///     const EXPECTED: &'static str = "an opcode";
///     fn is_start(c: char) -> bool { c.is_ascii_uppercase() }
///     fn is_continue(c: char) -> bool { c.is_ascii_uppercase() || c.is_ascii_digit() }
/// }
///
/// type Span = syan::source::string::Span;
/// let op: Ident<Span, Opcode> = Parse::parse("MOV2").unwrap();
/// assert_eq!(op.as_str(), "MOV2");
/// assert!(Ident::<Span, Opcode>::parse("mov").is_err());
/// ```
pub trait IdentClass {
    /// What the parser was looking for, as it appears in a [`ParseError::Expected`].
    const EXPECTED: &'static str = "an identifier";

    /// Whether `c` may open an identifier.
    fn is_start(c: char) -> bool;

    /// Whether `c` may continue one.
    fn is_continue(c: char) -> bool;
}

/// Rust's own identifier shape: a letter or `_`, then letters, digits and `_`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Rust;

impl IdentClass for Rust {
    fn is_start(c: char) -> bool {
        c.is_alphabetic() || c == '_'
    }

    fn is_continue(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }
}

/// [`Rust`], plus `.` as a continuing character, for a dotted path spelled as one word.
///
/// Scanning is greedy, so `a.` parses as the single word `a.`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Dotted;

impl IdentClass for Dotted {
    const EXPECTED: &'static str = "a dotted identifier";

    fn is_start(c: char) -> bool {
        Rust::is_start(c)
    }

    fn is_continue(c: char) -> bool {
        Rust::is_continue(c) || c == '.'
    }
}

/// A word parsed from a text source: one [`IdentClass::is_start`] character followed by a run of
/// [`IdentClass::is_continue`] ones.
///
/// `S` is the span type of the atoms it is parsed from, and `C` the character class.
///
/// ```
/// use syan::literal::Ident;
/// use syan::literal::ident::Dotted;
/// use syan::parse::Parse;
///
/// type Span = syan::source::string::Span;
///
/// let word: Ident<Span> = Parse::parse("total_count").unwrap();
/// assert_eq!(word.as_str(), "total_count");
///
/// let path: Ident<Span, Dotted> = Parse::parse("core.mem.swap").unwrap();
/// assert_eq!(path.as_str(), "core.mem.swap");
///
/// assert!(Ident::<Span>::parse("42").is_err());
/// ```
pub struct Ident<S, C = Rust> {
    /// The characters the word is spelled with.
    pub value: String,
    /// Where the word starts.
    pub span: S,
    class: PhantomData<fn() -> C>,
}

impl<S, C> Ident<S, C> {
    /// A word with the given spelling and position. The spelling is not checked against `C`.
    pub fn new(value: impl Into<String>, span: S) -> Self {
        Ident {
            value: value.into(),
            span,
            class: PhantomData,
        }
    }

    /// The spelling.
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl<S: Clone, C> Clone for Ident<S, C> {
    fn clone(&self) -> Self {
        Ident::new(self.value.clone(), self.span.clone())
    }
}

impl<S: core::fmt::Debug, C> core::fmt::Debug for Ident<S, C> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Ident")
            .field("value", &self.value)
            .field("span", &self.span)
            .finish()
    }
}

impl<S, C> core::fmt::Display for Ident<S, C> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.value)
    }
}

impl<S: PartialEq, C> PartialEq for Ident<S, C> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value && self.span == other.span
    }
}

impl<S: Eq, C> Eq for Ident<S, C> {}

impl<S: core::hash::Hash, C> core::hash::Hash for Ident<S, C> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.value.hash(state);
        self.span.hash(state);
    }
}

impl<S: Span, C> Spanned for Ident<S, C> {
    type Span = S;

    fn span(&self) -> Self::Span {
        self.span.clone()
    }
}

fn scan_ident<S: ParseStream, C: IdentClass>(scan: &mut Scan<'_, S>) -> Option<String> {
    let first = scan.peek()?;
    if !C::is_start(first) {
        return None;
    }
    scan.bump();
    let mut value = String::from(first);
    while let Some(c) = scan.peek() {
        if !C::is_continue(c) {
            break;
        }
        scan.bump();
        value.push(c);
    }
    Some(value)
}

macro_rules! impl_ident_parse {
    ($slot:ty, $as_char:ident) => {
        impl<Sp: Span, C: IdentClass> Parse<WithSpan<$slot, Sp>> for Ident<Sp, C> {
            type Error = ParseError<Sp>;

            fn parse_stream<__S: ParseStream<Atom = WithSpan<$slot, Sp>>>(
                stream: &mut __S,
            ) -> Result<Self, Self::Error> {
                let mut cursor = Scan::new(stream, $as_char::<Sp>);
                let span = cursor.span();
                match scan_ident::<__S, C>(&mut cursor) {
                    Some(value) => Ok(Ident::new(value, span)),
                    None => {
                        cursor.unwind();
                        Err(ParseError::expected(span, C::EXPECTED))
                    }
                }
            }
        }
    };
}

impl_ident_parse!(char, char_atom);
impl_ident_parse!(u8, byte_atom);
