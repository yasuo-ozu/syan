//! `Parse` for the literal types over *text* atoms, `char` and `u8`.
//!
//! The `TokenTree` impls in [`parse_impl`](super::parse_impl) validate a literal the lexer has
//! already delimited; text arrives undelimited, so these scan it an atom at a time. As on a token
//! stream, a leading `-` is punctuation rather than part of the literal.

use super::*;
use crate::span::WithSpan;

/// Longest first, so `u128` is not mistaken for `u1` followed by `28`.
const INT_SUFFIXES: &[&str] = &[
    "usize", "isize", "u128", "i128", "u16", "u32", "u64", "i16", "i32", "i64", "u8", "i8",
];

macro_rules! impl_integer_for_text_atom {
    ($slot:ty, $as_char:expr) => {
        impl<Sp: crate::span::Span> Parse<WithSpan<$slot, Sp>> for Integer {
            type Error = ParseError<Sp>;

            fn parse_stream<
                __S: crate::parse::parse_stream::ParseStream<Atom = WithSpan<$slot, Sp>>,
            >(
                stream: &mut __S,
            ) -> Result<Self, Self::Error> {
                let as_char: fn(&$slot) -> char = $as_char;
                let span = stream.peek().map(|a| a.span.clone()).unwrap_or_default();

                let mut value = String::new();
                let mut taken = Vec::new();
                let mut digits = 0usize;
                while let Some(c) = stream.peek().map(|a| as_char(&a.slot)) {
                    if c.is_ascii_digit() {
                        digits += 1;
                    } else if c != '_' {
                        break;
                    }
                    taken.push(stream.next().unwrap());
                    value.push(c);
                }
                if digits == 0 {
                    // `push` prepends, so unwinding from the tail restores the original order.
                    while let Some(atom) = taken.pop() {
                        stream.push(atom);
                    }
                    return Err(ParseError::literal(span, crate::error::LitKind::Int));
                }

                // A trailing run of ASCII alphanumerics is a suffix only if it is a real one;
                // otherwise it belongs to whatever the grammar expects next.
                let mut tail = String::new();
                let mut tail_atoms = Vec::new();
                while let Some(c) = stream.peek().map(|a| as_char(&a.slot)) {
                    if !c.is_ascii_alphanumeric() {
                        break;
                    }
                    tail_atoms.push(stream.next().unwrap());
                    tail.push(c);
                }
                let suffix = if tail.is_empty() {
                    None
                } else if INT_SUFFIXES.contains(&tail.as_str()) {
                    Some(tail)
                } else {
                    while let Some(atom) = tail_atoms.pop() {
                        stream.push(atom);
                    }
                    None
                };

                Ok(Integer { value, suffix })
            }
        }
    };
}

impl_integer_for_text_atom!(char, |c| *c);
impl_integer_for_text_atom!(u8, |b| *b as char);
