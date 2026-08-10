use super::*;

impl Parse<proc_macro2::TokenTree> for Bool {
    type Error = ParseError;

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
                        Err(ParseError::new(__span, "parse failed"))
                    }
                }
            }
            Some(token) => {
                let __span = crate::span::Spanned::span(&token);
                stream.push(token);
                Err(ParseError::new(__span, "parse failed"))
            }
            None => Err(ParseError::new(Span::default(), "parse failed")),
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
    type Error = ParseError;

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
                            None => Err(ParseError::new(Span::default(), "parse failed")),
                        }
                    } else {
                        Err(ParseError::new(Span::default(), "parse failed"))
                    }
                } else {
                    let __span = crate::span::Spanned::span(&proc_macro2::TokenTree::Literal(lit.clone()));
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(ParseError::new(__span, "parse failed"))
                }
            }
            Some(token) => {
                let __span = crate::span::Spanned::span(&token);
                stream.push(token);
                Err(ParseError::new(__span, "parse failed"))
            }
            None => Err(ParseError::new(Span::default(), "parse failed")),
        }
    }
}

impl Parse<proc_macro2::TokenTree> for Char {
    type Error = ParseError;

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
                            None => Err(ParseError::new(Span::default(), "parse failed")),
                        }
                    } else {
                        Err(ParseError::new(Span::default(), "parse failed"))
                    }
                } else {
                    let __span = crate::span::Spanned::span(&proc_macro2::TokenTree::Literal(lit.clone()));
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(ParseError::new(__span, "parse failed"))
                }
            }
            Some(token) => {
                let __span = crate::span::Spanned::span(&token);
                stream.push(token);
                Err(ParseError::new(__span, "parse failed"))
            }
            None => Err(ParseError::new(Span::default(), "parse failed")),
        }
    }
}

/// Shared scaffold for all `Literal`-based `Parse` impls: read one literal token,
/// hand its string form to `f`; on `None` (or a non-literal token / EOF), restore
/// the stream and fail — mirrors the existing push-back-on-failure behavior.
fn parse_lit<T, S: crate::parse::parse_stream::ParseStream<Atom = proc_macro2::TokenTree>>(
    stream: &mut S,
    f: impl FnOnce(&str) -> Option<T>,
) -> Result<T, ParseError> {
    match stream.next() {
        Some(proc_macro2::TokenTree::Literal(lit)) => {
            let lit_str = lit.to_string();
            match f(&lit_str) {
                Some(value) => Ok(value),
                None => {
                    let __span = crate::span::Spanned::span(&proc_macro2::TokenTree::Literal(lit.clone()));
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(ParseError::new(__span, "parse failed"))
                }
            }
        }
        Some(token) => {
            let __span = crate::span::Spanned::span(&token);
            stream.push(token);
            Err(ParseError::new(__span, "parse failed"))
        }
        None => Err(ParseError::new(Span::default(), "parse failed")),
    }
}

macro_rules! impl_parse_lit {
    ($Ty:ident, $body:expr) => {
        impl Parse<proc_macro2::TokenTree> for $Ty {
            type Error = ParseError;

            fn parse_stream<__S: crate::parse::parse_stream::ParseStream<Atom = proc_macro2::TokenTree>>(stream: &mut __S) -> Result<Self, Self::Error> {
                parse_lit(&mut *stream, $body)
            }
        }
    };
}

/// Shared by StrRaw/ByteStrRaw/CStrRaw after the caller strips the "r"/"br"/"cr" prefix.
/// NOTE: intentionally reproduces the current hash-counting loop bit-for-bit — the
/// `while let Some('#') = chars.next()` loop consumes the first non-'#' char (the
/// opening quote) via the failed match arm, so `remaining` never starts with '"' and
/// every raw-string literal currently FAILS to parse, at any hash count including zero.
/// Pre-existing latent bug, identical in all three impls today (tests tolerate it via
/// `if let Ok`); this fold preserves it — do NOT "fix" it while applying this design.
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

impl_parse_lit!(Str, |s: &str| {
    (s.starts_with('"')
        && s.ends_with('"')
        && !s.starts_with('r')
        && !s.starts_with('b')
        && !s.starts_with('c'))
    .then(|| Str {
        value: s[1..s.len() - 1].to_string(),
    })
});

impl_parse_lit!(ByteStr, |s: &str| {
    (s.starts_with("b\"") && s.ends_with('"') && !s.starts_with("br")).then(|| ByteStr {
        value: s[2..s.len() - 1].bytes().collect(),
    })
});

impl_parse_lit!(CStr, |s: &str| {
    (s.starts_with("c\"") && s.ends_with('"') && !s.starts_with("cr")).then(|| CStr {
        value: s[2..s.len() - 1].to_string(),
    })
});

impl_parse_lit!(StrRaw, |s: &str| {
    let (value, hash_count) = parse_raw(s.strip_prefix('r')?)?;
    Some(StrRaw { value, hash_count })
});

impl_parse_lit!(ByteStrRaw, |s: &str| {
    let (value, hash_count) = parse_raw(s.strip_prefix("br")?)?;
    Some(ByteStrRaw {
        value: value.bytes().collect(),
        hash_count,
    })
});

impl_parse_lit!(CStrRaw, |s: &str| {
    let (value, hash_count) = parse_raw(s.strip_prefix("cr")?)?;
    Some(CStrRaw { value, hash_count })
});

impl_parse_lit!(Integer, |s: &str| {
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
            // no early return on a failed suffix match — the original loop keeps trying
        }
    }
    (s.chars()
        .all(|c| c.is_ascii_digit() || c == '_' || (c == '-' && s.starts_with('-'))))
    .then(|| Integer {
        value: s.to_string(),
        suffix: None,
    })
});

impl_parse_lit!(Float, |s: &str| {
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
