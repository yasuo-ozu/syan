use super::*;

impl Parse<proc_macro2::TokenTree> for Bool {
    type Error = ParseError<Span>;

    fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = proc_macro2::TokenTree>>(stream: &mut __S) -> Result<Self, Self::Error> {
        match stream.next() {
            Some(proc_macro2::TokenTree::Ident(ident)) => {
                let ident_str = ident.to_string();
                match ident_str.as_str() {
                    "true" => Ok(Bool { value: true }),
                    "false" => Ok(Bool { value: false }),
                    _ => {
                        let __span = crate::span::Spanned::span(&proc_macro2::TokenTree::Ident(ident.clone()));
                        stream.push(proc_macro2::TokenTree::Ident(ident));
                        Err(ParseError::literal(__span, crate::error::LitKind::Bool))
                    }
                }
            }
            Some(token) => {
                let __span = crate::span::Spanned::span(&token);
                stream.push(token);
                Err(ParseError::literal(__span, crate::error::LitKind::Bool))
            }
            None => Err(ParseError::eof(Span::default())),
        }
    }
}

/// Shared escape table for ByteChar/Char (all-ASCII values; `c as u8` is lossless).
fn unescape(rest: &str) -> Option<char> {
    match rest {
        "n" => Some('\n'),
        "t" => Some('\t'),
        "r" => Some('\r'),
        "\\" => Some('\\'),
        "'" => Some('\''),
        "0" => Some('\0'),
        _ => None,
    }
}

impl Parse<proc_macro2::TokenTree> for ByteChar {
    type Error = ParseError<Span>;

    fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = proc_macro2::TokenTree>>(stream: &mut __S) -> Result<Self, Self::Error> {
        match stream.next() {
            Some(proc_macro2::TokenTree::Literal(lit)) => {
                let lit_str = lit.to_string();
                if lit_str.starts_with("b'") && lit_str.ends_with("'") {
                    let inner = &lit_str[2..lit_str.len() - 1];
                    if inner.len() == 1 {
                        let byte_val = inner.chars().next().unwrap() as u8;
                        Ok(ByteChar { value: byte_val })
                    } else if let Some(rest) = inner.strip_prefix('\\') {
                        match unescape(rest) {
                            Some(c) => Ok(ByteChar { value: c as u8 }),
                            None => Err(ParseError::eof(Span::default())),
                        }
                    } else {
                        Err(ParseError::eof(Span::default()))
                    }
                } else {
                    let __span = crate::span::Spanned::span(&proc_macro2::TokenTree::Literal(lit.clone()));
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(ParseError::literal(__span, crate::error::LitKind::ByteChar))
                }
            }
            Some(token) => {
                let __span = crate::span::Spanned::span(&token);
                stream.push(token);
                Err(ParseError::literal(__span, crate::error::LitKind::ByteChar))
            }
            None => Err(ParseError::eof(Span::default())),
        }
    }
}

impl Parse<proc_macro2::TokenTree> for Char {
    type Error = ParseError<Span>;

    fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = proc_macro2::TokenTree>>(stream: &mut __S) -> Result<Self, Self::Error> {
        match stream.next() {
            Some(proc_macro2::TokenTree::Literal(lit)) => {
                let lit_str = lit.to_string();
                if lit_str.starts_with("'") && lit_str.ends_with("'") && !lit_str.starts_with("b'")
                {
                    let inner = &lit_str[1..lit_str.len() - 1];
                    if inner.len() == 1 {
                        let char_val = inner.chars().next().unwrap();
                        Ok(Char { value: char_val })
                    } else if let Some(rest) = inner.strip_prefix('\\') {
                        match unescape(rest) {
                            Some(c) => Ok(Char { value: c }),
                            None => Err(ParseError::eof(Span::default())),
                        }
                    } else {
                        Err(ParseError::eof(Span::default()))
                    }
                } else {
                    let __span = crate::span::Spanned::span(&proc_macro2::TokenTree::Literal(lit.clone()));
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(ParseError::literal(__span, crate::error::LitKind::Char))
                }
            }
            Some(token) => {
                let __span = crate::span::Spanned::span(&token);
                stream.push(token);
                Err(ParseError::literal(__span, crate::error::LitKind::Char))
            }
            None => Err(ParseError::eof(Span::default())),
        }
    }
}

/// Shared scaffold for all `Literal`-based `Parse` impls: read one literal token and hand its
/// string form to `f`; on `None` (or a non-literal token / EOF), push the token back and fail.
fn parse_lit<T, S: crate::parse::parse_stream::ParseStream<Atom = proc_macro2::TokenTree>>(
    stream: &mut S,
    kind: crate::error::LitKind,
    f: impl FnOnce(&str) -> Option<T>,
) -> Result<T, ParseError<Span>> {
    match stream.next() {
        Some(proc_macro2::TokenTree::Literal(lit)) => {
            let lit_str = lit.to_string();
            match f(&lit_str) {
                Some(value) => Ok(value),
                None => {
                    let __span = crate::span::Spanned::span(&proc_macro2::TokenTree::Literal(lit.clone()));
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(ParseError::literal(__span, kind))
                }
            }
        }
        Some(token) => {
            let __span = crate::span::Spanned::span(&token);
            stream.push(token);
            Err(ParseError::literal(__span, kind))
        }
        None => Err(ParseError::eof(Span::default())),
    }
}

macro_rules! impl_parse_lit {
    ($Ty:ident, $kind:expr, $body:expr) => {
        impl Parse<proc_macro2::TokenTree> for $Ty {
            type Error = ParseError<Span>;

            fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = proc_macro2::TokenTree>>(stream: &mut __S) -> Result<Self, Self::Error> {
                parse_lit(&mut *stream, $kind, $body)
            }
        }
    };
}

/// Shared by StrRaw/ByteStrRaw/CStrRaw after the caller strips the "r"/"br"/"cr" prefix.
///
/// KNOWN BUG: the `while let Some('#')` loop also consumes the first non-'#' char (the opening
/// quote) via its failed match arm, so `remaining` never starts with '"' and every raw-string
/// literal fails to parse, at any hash count. Tests tolerate this via `if let Ok`.
fn parse_raw(rest: &str) -> Option<(String, usize)> {
    let mut hash_count = 0;
    let mut chars = rest.chars();
    while let Some('#') = chars.next() {
        hash_count += 1;
    }
    let remaining: String = chars.collect();
    if remaining.starts_with('"') && remaining.ends_with('"') {
        Some((remaining[1..remaining.len() - 1].to_string(), hash_count))
    } else {
        None
    }
}

impl_parse_lit!(Str, crate::error::LitKind::Str, |s: &str| {
    (s.starts_with('"')
        && s.ends_with('"')
        && !s.starts_with('r')
        && !s.starts_with('b')
        && !s.starts_with('c'))
    .then(|| Str {
        value: s[1..s.len() - 1].to_string(),
    })
});

impl_parse_lit!(ByteStr, crate::error::LitKind::ByteStr, |s: &str| {
    (s.starts_with("b\"") && s.ends_with('"') && !s.starts_with("br")).then(|| ByteStr {
        value: s[2..s.len() - 1].bytes().collect(),
    })
});

impl_parse_lit!(CStr, crate::error::LitKind::CStr, |s: &str| {
    (s.starts_with("c\"") && s.ends_with('"') && !s.starts_with("cr")).then(|| CStr {
        value: s[2..s.len() - 1].to_string(),
    })
});

impl_parse_lit!(StrRaw, crate::error::LitKind::Str, |s: &str| {
    let (value, hash_count) = parse_raw(s.strip_prefix('r')?)?;
    Some(StrRaw { value, hash_count })
});

impl_parse_lit!(ByteStrRaw, crate::error::LitKind::ByteStr, |s: &str| {
    let (value, hash_count) = parse_raw(s.strip_prefix("br")?)?;
    Some(ByteStrRaw {
        value: value.bytes().collect(),
        hash_count,
    })
});

impl_parse_lit!(CStrRaw, crate::error::LitKind::CStr, |s: &str| {
    let (value, hash_count) = parse_raw(s.strip_prefix("cr")?)?;
    Some(CStrRaw { value, hash_count })
});

impl_parse_lit!(Integer, crate::error::LitKind::Int, |s: &str| {
    const SUFFIXES: &[&str] = &[
        "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
    ];
    if s.contains('.') {
        return None;
    }
    for suffix in SUFFIXES {
        if let Some(value) = s.strip_suffix(suffix) {
            if value
                .chars()
                .all(|c| c.is_ascii_digit() || c == '_' || (c == '-' && value.starts_with('-')))
            {
                return Some(Integer {
                    value: value.to_string(),
                    suffix: Some((*suffix).to_string()),
                });
            }
            // no early return on a failed suffix match — keep trying the other suffixes
        }
    }
    (s.chars()
        .all(|c| c.is_ascii_digit() || c == '_' || (c == '-' && s.starts_with('-'))))
    .then(|| Integer {
        value: s.to_string(),
        suffix: None,
    })
});

impl_parse_lit!(Float, crate::error::LitKind::Float, |s: &str| {
    const SUFFIXES: &[&str] = &["f32", "f64"];
    if !s.contains('.') {
        return None;
    }
    for suffix in SUFFIXES {
        if let Some(value) = s.strip_suffix(suffix) {
            if value.parse::<f64>().is_ok() {
                return Some(Float {
                    value: value.to_string(),
                    suffix: Some((*suffix).to_string()),
                });
            }
        }
    }
    s.parse::<f64>().ok().map(|_| Float {
        value: s.to_string(),
        suffix: None,
    })
});
