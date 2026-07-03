use super::*;

impl Parse<proc_macro2::TokenTree> for Bool {
    type Error = ParseError;

    fn parse(
        stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
    ) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        match stream.next() {
            Some(proc_macro2::TokenTree::Ident(ident)) => {
                let ident_str = ident.to_string();
                match ident_str.as_str() {
                    "true" => Ok(Bool { value: true }),
                    "false" => Ok(Bool { value: false }),
                    _ => {
                        stream.push(proc_macro2::TokenTree::Ident(ident));
                        Err(ParseError::new(Span::default(), "parse failed"))
                    }
                }
            }
            Some(token) => {
                stream.push(token);
                Err(ParseError::new(Span::default(), "parse failed"))
            }
            None => Err(ParseError::new(Span::default(), "parse failed")),
        }
    }

}

impl Parse<proc_macro2::TokenTree> for ByteChar {
    type Error = ParseError;

    fn parse(
        stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
    ) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        match stream.next() {
            Some(proc_macro2::TokenTree::Literal(lit)) => {
                let lit_str = lit.to_string();
                if lit_str.starts_with("b'") && lit_str.ends_with("'") {
                    let inner = &lit_str[2..lit_str.len() - 1];
                    if inner.len() == 1 {
                        let byte_val = inner.chars().next().unwrap() as u8;
                        Ok(ByteChar { value: byte_val })
                    } else if let Some(rest) = inner.strip_prefix('\\') {
                        // Handle escape sequences
                        match rest {
                            "n" => Ok(ByteChar { value: b'\n' }),
                            "t" => Ok(ByteChar { value: b'\t' }),
                            "r" => Ok(ByteChar { value: b'\r' }),
                            "\\" => Ok(ByteChar { value: b'\\' }),
                            "'" => Ok(ByteChar { value: b'\'' }),
                            "0" => Ok(ByteChar { value: 0 }),
                            _ => Err(ParseError::new(Span::default(), "parse failed")),
                        }
                    } else {
                        Err(ParseError::new(Span::default(), "parse failed"))
                    }
                } else {
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(ParseError::new(Span::default(), "parse failed"))
                }
            }
            Some(token) => {
                stream.push(token);
                Err(ParseError::new(Span::default(), "parse failed"))
            }
            None => Err(ParseError::new(Span::default(), "parse failed")),
        }
    }

}

impl Parse<proc_macro2::TokenTree> for Char {
    type Error = ParseError;

    fn parse(
        stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
    ) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
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
                        // Handle escape sequences
                        match rest {
                            "n" => Ok(Char { value: '\n' }),
                            "t" => Ok(Char { value: '\t' }),
                            "r" => Ok(Char { value: '\r' }),
                            "\\" => Ok(Char { value: '\\' }),
                            "'" => Ok(Char { value: '\'' }),
                            "0" => Ok(Char { value: '\0' }),
                            _ => Err(ParseError::new(Span::default(), "parse failed")),
                        }
                    } else {
                        Err(ParseError::new(Span::default(), "parse failed"))
                    }
                } else {
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(ParseError::new(Span::default(), "parse failed"))
                }
            }
            Some(token) => {
                stream.push(token);
                Err(ParseError::new(Span::default(), "parse failed"))
            }
            None => Err(ParseError::new(Span::default(), "parse failed")),
        }
    }

}

impl Parse<proc_macro2::TokenTree> for Integer {
    type Error = ParseError;

    fn parse(
        stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
    ) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        match stream.next() {
            Some(proc_macro2::TokenTree::Literal(lit)) => {
                let lit_str = lit.to_string();
                // Check if it's an integer (doesn't contain '.' and is numeric)
                if !lit_str.contains('.') {
                    // Split on common integer suffixes
                    let suffixes = [
                        "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64",
                        "i128", "isize",
                    ];
                    for suffix in &suffixes {
                        if lit_str.ends_with(suffix) {
                            let value = lit_str[..lit_str.len() - suffix.len()].to_string();
                            // Validate that the remaining part is numeric
                            if value.chars().all(|c| {
                                c.is_ascii_digit()
                                    || c == '_'
                                    || (c == '-' && value.starts_with('-'))
                            }) {
                                return Ok(Integer {
                                    value,
                                    suffix: Some(suffix.to_string()),
                                });
                            }
                        }
                    }
                    // No suffix found, check if it's a plain integer
                    if lit_str.chars().all(|c| {
                        c.is_ascii_digit() || c == '_' || (c == '-' && lit_str.starts_with('-'))
                    }) {
                        Ok(Integer {
                            value: lit_str,
                            suffix: None,
                        })
                    } else {
                        stream.push(proc_macro2::TokenTree::Literal(lit));
                        Err(ParseError::new(Span::default(), "parse failed"))
                    }
                } else {
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(ParseError::new(Span::default(), "parse failed"))
                }
            }
            Some(token) => {
                stream.push(token);
                Err(ParseError::new(Span::default(), "parse failed"))
            }
            None => Err(ParseError::new(Span::default(), "parse failed")),
        }
    }

}

impl Parse<proc_macro2::TokenTree> for Float {
    type Error = ParseError;

    fn parse(
        stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
    ) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        match stream.next() {
            Some(proc_macro2::TokenTree::Literal(lit)) => {
                let lit_str = lit.to_string();
                // Check if it contains a decimal point
                if lit_str.contains('.') {
                    // Split on common float suffixes
                    let suffixes = ["f32", "f64"];
                    for suffix in &suffixes {
                        if lit_str.ends_with(suffix) {
                            let value = lit_str[..lit_str.len() - suffix.len()].to_string();
                            // Validate that it's a valid float
                            if value.parse::<f64>().is_ok() {
                                return Ok(Float {
                                    value,
                                    suffix: Some(suffix.to_string()),
                                });
                            }
                        }
                    }
                    // No suffix found, check if it's a plain float
                    if lit_str.parse::<f64>().is_ok() {
                        Ok(Float {
                            value: lit_str,
                            suffix: None,
                        })
                    } else {
                        stream.push(proc_macro2::TokenTree::Literal(lit));
                        Err(ParseError::new(Span::default(), "parse failed"))
                    }
                } else {
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(ParseError::new(Span::default(), "parse failed"))
                }
            }
            Some(token) => {
                stream.push(token);
                Err(ParseError::new(Span::default(), "parse failed"))
            }
            None => Err(ParseError::new(Span::default(), "parse failed")),
        }
    }

}

impl Parse<proc_macro2::TokenTree> for Str {
    type Error = ParseError;

    fn parse(
        stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
    ) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        match stream.next() {
            Some(proc_macro2::TokenTree::Literal(lit)) => {
                let lit_str = lit.to_string();
                if lit_str.starts_with('"')
                    && lit_str.ends_with('"')
                    && !lit_str.starts_with("r")
                    && !lit_str.starts_with("b")
                    && !lit_str.starts_with("c")
                {
                    // Regular string literal
                    let value = lit_str[1..lit_str.len() - 1].to_string();
                    Ok(Str { value })
                } else {
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(ParseError::new(Span::default(), "parse failed"))
                }
            }
            Some(token) => {
                stream.push(token);
                Err(ParseError::new(Span::default(), "parse failed"))
            }
            None => Err(ParseError::new(Span::default(), "parse failed")),
        }
    }

}

impl Parse<proc_macro2::TokenTree> for StrRaw {
    type Error = ParseError;

    fn parse(
        stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
    ) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        match stream.next() {
            Some(proc_macro2::TokenTree::Literal(lit)) => {
                let lit_str = lit.to_string();
                if let Some(rest) = lit_str.strip_prefix('r') {
                    // Count hash marks
                    let mut hash_count = 0;
                    let mut chars = rest.chars();
                    while let Some('#') = chars.next() {
                        hash_count += 1;
                    }
                    // Should be followed by a quote
                    let remaining: String = chars.collect();
                    if remaining.starts_with('"') && remaining.ends_with('"') {
                        let value = remaining[1..remaining.len() - 1].to_string();
                        Ok(StrRaw { value, hash_count })
                    } else {
                        stream.push(proc_macro2::TokenTree::Literal(lit));
                        Err(ParseError::new(Span::default(), "parse failed"))
                    }
                } else {
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(ParseError::new(Span::default(), "parse failed"))
                }
            }
            Some(token) => {
                stream.push(token);
                Err(ParseError::new(Span::default(), "parse failed"))
            }
            None => Err(ParseError::new(Span::default(), "parse failed")),
        }
    }

}

impl Parse<proc_macro2::TokenTree> for ByteStr {
    type Error = ParseError;

    fn parse(
        stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
    ) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        match stream.next() {
            Some(proc_macro2::TokenTree::Literal(lit)) => {
                let lit_str = lit.to_string();
                if lit_str.starts_with("b\"")
                    && lit_str.ends_with('"')
                    && !lit_str.starts_with("br")
                {
                    // Regular byte string literal
                    let inner = &lit_str[2..lit_str.len() - 1];
                    let value = inner.bytes().collect();
                    Ok(ByteStr { value })
                } else {
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(ParseError::new(Span::default(), "parse failed"))
                }
            }
            Some(token) => {
                stream.push(token);
                Err(ParseError::new(Span::default(), "parse failed"))
            }
            None => Err(ParseError::new(Span::default(), "parse failed")),
        }
    }

}

impl Parse<proc_macro2::TokenTree> for ByteStrRaw {
    type Error = ParseError;

    fn parse(
        stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
    ) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        match stream.next() {
            Some(proc_macro2::TokenTree::Literal(lit)) => {
                let lit_str = lit.to_string();
                if let Some(rest) = lit_str.strip_prefix("br") {
                    // Count hash marks after "br"
                    let mut hash_count = 0;
                    let mut chars = rest.chars();
                    while let Some('#') = chars.next() {
                        hash_count += 1;
                    }
                    // Should be followed by a quote
                    let remaining: String = chars.collect();
                    if remaining.starts_with('"') && remaining.ends_with('"') {
                        let inner = &remaining[1..remaining.len() - 1];
                        let value = inner.bytes().collect();
                        Ok(ByteStrRaw { value, hash_count })
                    } else {
                        stream.push(proc_macro2::TokenTree::Literal(lit));
                        Err(ParseError::new(Span::default(), "parse failed"))
                    }
                } else {
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(ParseError::new(Span::default(), "parse failed"))
                }
            }
            Some(token) => {
                stream.push(token);
                Err(ParseError::new(Span::default(), "parse failed"))
            }
            None => Err(ParseError::new(Span::default(), "parse failed")),
        }
    }

}

impl Parse<proc_macro2::TokenTree> for CStr {
    type Error = ParseError;

    fn parse(
        stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
    ) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        match stream.next() {
            Some(proc_macro2::TokenTree::Literal(lit)) => {
                let lit_str = lit.to_string();
                if lit_str.starts_with("c\"")
                    && lit_str.ends_with('"')
                    && !lit_str.starts_with("cr")
                {
                    // Regular C string literal
                    let value = lit_str[2..lit_str.len() - 1].to_string();
                    Ok(CStr { value })
                } else {
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(ParseError::new(Span::default(), "parse failed"))
                }
            }
            Some(token) => {
                stream.push(token);
                Err(ParseError::new(Span::default(), "parse failed"))
            }
            None => Err(ParseError::new(Span::default(), "parse failed")),
        }
    }

}

impl Parse<proc_macro2::TokenTree> for CStrRaw {
    type Error = ParseError;

    fn parse(
        stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
    ) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        match stream.next() {
            Some(proc_macro2::TokenTree::Literal(lit)) => {
                let lit_str = lit.to_string();
                if let Some(rest) = lit_str.strip_prefix("cr") {
                    // Count hash marks after "cr"
                    let mut hash_count = 0;
                    let mut chars = rest.chars();
                    while let Some('#') = chars.next() {
                        hash_count += 1;
                    }
                    // Should be followed by a quote
                    let remaining: String = chars.collect();
                    if remaining.starts_with('"') && remaining.ends_with('"') {
                        let value = remaining[1..remaining.len() - 1].to_string();
                        Ok(CStrRaw { value, hash_count })
                    } else {
                        stream.push(proc_macro2::TokenTree::Literal(lit));
                        Err(ParseError::new(Span::default(), "parse failed"))
                    }
                } else {
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(ParseError::new(Span::default(), "parse failed"))
                }
            }
            Some(token) => {
                stream.push(token);
                Err(ParseError::new(Span::default(), "parse failed"))
            }
            None => Err(ParseError::new(Span::default(), "parse failed")),
        }
    }

}
