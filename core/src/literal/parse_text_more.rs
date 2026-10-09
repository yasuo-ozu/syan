//! `Parse` over *text* atoms (`char` and `u8`) for every literal type except
//! [`Integer`](super::Integer), which lives in [`parse_text_impl`](super::parse_text_impl).
//!
//! Text arrives undelimited, so each parser scans the literal an atom at a time. On failure it
//! hands every atom it took back to the stream in reverse — `ParseStream::push` prepends — so the
//! alternative that follows sees the input exactly as it was. [`Scan`] is what keeps that
//! discipline in one place.
//!
//! Where these differ from the `TokenTree` impls, which validate a literal the lexer has already
//! delimited:
//!
//! * A `u8` atom takes part only if it is ASCII, so a byte stream carries ASCII literal contents
//!   only; a byte `>= 0x80` ends the literal rather than being decoded.
//! * [`Float`] also accepts an exponent with no fractional part (`1e10`) and a bare suffix
//!   (`42f64`). Both are Rust float literals; the `TokenTree` impl rejects them for want of a `.`.
//! * The character forms resolve the same six escapes as the `TokenTree` impls (`\n`, `\t`, `\r`,
//!   `\\`, `\'`, `\0`); `\x41` and `\u{41}` are not resolved. The string forms keep their contents
//!   exactly as written, escapes unresolved, as the `TokenTree` impls do.

use super::{Bool, ByteChar, ByteStr, ByteStrRaw, CStr, CStrRaw, Char, Float, Str, StrRaw};
use crate::error::{LitKind, ParseError};
use crate::parse::{Parse, ParseStream};
use crate::span::{Span, Spanned, WithSpan};

/// A cursor that remembers every atom it took, so a failed scan can give them all back.
pub(crate) struct Scan<'a, S: ParseStream> {
    stream: &'a mut S,
    as_char: fn(&S::Atom) -> Option<char>,
    taken: Vec<S::Atom>,
}

impl<'a, S: ParseStream> Scan<'a, S> {
    pub(crate) fn new(stream: &'a mut S, as_char: fn(&S::Atom) -> Option<char>) -> Self {
        Scan {
            stream,
            as_char,
            taken: Vec::new(),
        }
    }

    /// Where the next atom sits, for an error message. Call before scanning anything.
    pub(crate) fn span<Sp: Span>(&mut self) -> Sp
    where
        S::Atom: Spanned<Span = Sp>,
    {
        self.stream.peek().map(|a| a.span()).unwrap_or_default()
    }

    /// The next atom as a `char`, or `None` at end of input or on an atom with no `char` form.
    pub(crate) fn peek(&mut self) -> Option<char> {
        let as_char = self.as_char;
        self.stream.peek().and_then(as_char)
    }

    pub(crate) fn next_char(&mut self) -> Option<char> {
        let c = self.peek()?;
        let atom = self.stream.next()?;
        self.taken.push(atom);
        Some(c)
    }

    /// Take the atom [`peek`](Self::peek) just reported.
    pub(crate) fn bump(&mut self) {
        let _ = self.next_char();
    }

    pub(crate) fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.bump();
            true
        } else {
            false
        }
    }

    /// Takes as much of `prefix` as matches; a partial match is left for the caller to unwind.
    pub(crate) fn eat_str(&mut self, prefix: &str) -> bool {
        prefix.chars().all(|c| self.eat(c))
    }

    pub(crate) fn mark(&self) -> usize {
        self.taken.len()
    }

    /// Give back everything taken since `mark`, newest first so the original order is restored.
    pub(crate) fn rewind(&mut self, mark: usize) {
        while self.taken.len() > mark {
            let atom = self.taken.pop().unwrap();
            self.stream.push(atom);
        }
    }

    pub(crate) fn unwind(&mut self) {
        self.rewind(0)
    }
}

pub(crate) fn char_atom<Sp>(atom: &WithSpan<char, Sp>) -> Option<char> {
    Some(atom.slot)
}

pub(crate) fn byte_atom<Sp>(atom: &WithSpan<u8, Sp>) -> Option<char> {
    atom.slot.is_ascii().then_some(atom.slot as char)
}

fn unescape(c: char) -> Option<char> {
    match c {
        'n' => Some('\n'),
        't' => Some('\t'),
        'r' => Some('\r'),
        '\\' => Some('\\'),
        '\'' => Some('\''),
        '0' => Some('\0'),
        _ => None,
    }
}

fn scan_bool<S: ParseStream>(s: &mut Scan<'_, S>) -> Option<Bool> {
    let mut word = String::new();
    while let Some(c) = s.peek() {
        if !(c.is_ascii_alphanumeric() || c == '_') {
            break;
        }
        s.bump();
        word.push(c);
    }
    match word.as_str() {
        "true" => Some(Bool { value: true }),
        "false" => Some(Bool { value: false }),
        _ => None,
    }
}

fn scan_char_lit<S: ParseStream>(s: &mut Scan<'_, S>, prefix: &str) -> Option<char> {
    if !s.eat_str(prefix) || !s.eat('\'') {
        return None;
    }
    let value = match s.next_char()? {
        '\'' => return None,
        '\\' => unescape(s.next_char()?)?,
        c => c,
    };
    s.eat('\'').then_some(value)
}

fn scan_quoted<S: ParseStream>(s: &mut Scan<'_, S>, prefix: &str) -> Option<String> {
    if !s.eat_str(prefix) || !s.eat('"') {
        return None;
    }
    let mut value = String::new();
    loop {
        match s.next_char()? {
            '"' => return Some(value),
            '\\' => {
                value.push('\\');
                value.push(s.next_char()?);
            }
            c => value.push(c),
        }
    }
}

fn scan_raw<S: ParseStream>(s: &mut Scan<'_, S>, prefix: &str) -> Option<(String, usize)> {
    if !s.eat_str(prefix) {
        return None;
    }
    let mut hash_count = 0;
    while s.eat('#') {
        hash_count += 1;
    }
    if !s.eat('"') {
        return None;
    }
    let mut value = String::new();
    loop {
        if s.peek()? == '"' {
            let mark = s.mark();
            s.bump();
            let mut seen = 0;
            while seen < hash_count && s.eat('#') {
                seen += 1;
            }
            if seen == hash_count {
                return Some((value, hash_count));
            }
            // Too few `#`: that quote is content after all.
            s.rewind(mark);
        }
        value.push(s.next_char()?);
    }
}

const FLOAT_SUFFIXES: &[&str] = &["f32", "f64"];

/// A run of digits and underscores containing at least one digit; nothing is taken otherwise.
fn scan_digits<S: ParseStream>(s: &mut Scan<'_, S>) -> Option<String> {
    let mark = s.mark();
    let mut out = String::new();
    let mut digits = 0usize;
    while let Some(c) = s.peek() {
        if c.is_ascii_digit() {
            digits += 1;
        } else if c != '_' {
            break;
        }
        s.bump();
        out.push(c);
    }
    if digits == 0 {
        s.rewind(mark);
        return None;
    }
    Some(out)
}

/// A trailing run of ASCII alphanumerics is a suffix only if it is a real one; otherwise it belongs
/// to whatever the grammar expects next.
fn scan_suffix<S: ParseStream>(s: &mut Scan<'_, S>, allowed: &[&str]) -> Option<String> {
    let mark = s.mark();
    let mut tail = String::new();
    while let Some(c) = s.peek() {
        if !c.is_ascii_alphanumeric() {
            break;
        }
        s.bump();
        tail.push(c);
    }
    if allowed.contains(&tail.as_str()) {
        Some(tail)
    } else {
        s.rewind(mark);
        None
    }
}

fn scan_float<S: ParseStream>(s: &mut Scan<'_, S>) -> Option<Float> {
    let mut value = scan_digits(s)?;
    let mut floating = false;

    if s.peek() == Some('.') {
        let mark = s.mark();
        s.bump();
        // Only a digit after the dot makes it a fractional part: `1..2` and `1.foo` keep theirs.
        match s.peek() {
            Some(c) if c.is_ascii_digit() => {
                value.push('.');
                value.push_str(&scan_digits(s)?);
                floating = true;
            }
            _ => s.rewind(mark),
        }
    }

    if matches!(s.peek(), Some('e' | 'E')) {
        let mark = s.mark();
        let exponent = s.next_char()?;
        let sign = match s.peek() {
            Some(c @ ('+' | '-')) => {
                s.bump();
                Some(c)
            }
            _ => None,
        };
        match scan_digits(s) {
            Some(digits) => {
                value.push(exponent);
                if let Some(sign) = sign {
                    value.push(sign);
                }
                value.push_str(&digits);
                floating = true;
            }
            None => s.rewind(mark),
        }
    }

    let suffix = scan_suffix(s, FLOAT_SUFFIXES);
    (floating || suffix.is_some()).then_some(Float { value, suffix })
}

macro_rules! impl_text_lit {
    ($Ty:ident, $kind:ident, $scan:expr) => {
        impl_text_lit!(@atom $Ty, $kind, $scan, char, char_atom);
        impl_text_lit!(@atom $Ty, $kind, $scan, u8, byte_atom);
    };
    (@atom $Ty:ident, $kind:ident, $scan:expr, $slot:ty, $as_char:ident) => {
        impl<Sp: Span> Parse<WithSpan<$slot, Sp>> for $Ty {
            type Error = ParseError<Sp>;

            fn parse_stream<__S: ParseStream<Atom = WithSpan<$slot, Sp>>>(
                stream: &mut __S,
            ) -> Result<Self, Self::Error> {
                let scan: fn(&mut Scan<'_, __S>) -> Option<$Ty> = $scan;
                let mut cursor = Scan::new(stream, $as_char::<Sp>);
                let span = cursor.span();
                match scan(&mut cursor) {
                    Some(value) => Ok(value),
                    None => {
                        cursor.unwind();
                        Err(ParseError::literal(span, LitKind::$kind))
                    }
                }
            }
        }
    };
}

impl_text_lit!(Bool, Bool, scan_bool);
impl_text_lit!(Float, Float, scan_float);

impl_text_lit!(Char, Char, |s| scan_char_lit(s, "")
    .map(|value| Char { value }));
impl_text_lit!(ByteChar, ByteChar, |s| scan_char_lit(s, "b")
    .filter(char::is_ascii)
    .map(|c| ByteChar { value: c as u8 }));

impl_text_lit!(Str, Str, |s| scan_quoted(s, "").map(|value| Str { value }));
impl_text_lit!(ByteStr, ByteStr, |s| scan_quoted(s, "b")
    .filter(|v| v.is_ascii())
    .map(|v| ByteStr {
        value: v.into_bytes()
    }));
impl_text_lit!(CStr, CStr, |s| scan_quoted(s, "c")
    .map(|value| CStr { value }));

impl_text_lit!(StrRaw, Str, |s| scan_raw(s, "r")
    .map(|(value, hash_count)| { StrRaw { value, hash_count } }));
impl_text_lit!(ByteStrRaw, ByteStr, |s| scan_raw(s, "br")
    .filter(|(v, _)| v.is_ascii())
    .map(|(v, hash_count)| ByteStrRaw {
        value: v.into_bytes(),
        hash_count,
    }));
impl_text_lit!(CStrRaw, CStr, |s| scan_raw(s, "cr")
    .map(|(value, hash_count)| CStrRaw { value, hash_count }));
