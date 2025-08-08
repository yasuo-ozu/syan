use crate::parse::unparse::Emitter;
use crate::parse::{IntoParseStream, Parse, ParseStream, Unparse};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Bool {
    pub value: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ByteChar {
    pub value: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Char {
    pub value: char,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Integer {
    pub value: String,
    pub suffix: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Float {
    pub value: String,
    pub suffix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Str {
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StrRaw {
    pub value: String,
    pub hash_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ByteStr {
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ByteStrRaw {
    pub value: Vec<u8>,
    pub hash_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CStr {
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CStrRaw {
    pub value: String,
    pub hash_count: usize,
}

impl Parse<proc_macro2::TokenTree> for Bool {
    type Error = ();

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
                        Err(())
                    }
                }
            }
            Some(token) => {
                stream.push(token);
                Err(())
            }
            None => Err(()),
        }
    }
}

impl Parse<proc_macro2::TokenTree> for ByteChar {
    type Error = ();

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
                    } else if inner.starts_with('\\') {
                        // Handle escape sequences
                        match &inner[1..] {
                            "n" => Ok(ByteChar { value: b'\n' }),
                            "t" => Ok(ByteChar { value: b'\t' }),
                            "r" => Ok(ByteChar { value: b'\r' }),
                            "\\" => Ok(ByteChar { value: b'\\' }),
                            "'" => Ok(ByteChar { value: b'\'' }),
                            "0" => Ok(ByteChar { value: 0 }),
                            _ => Err(()),
                        }
                    } else {
                        Err(())
                    }
                } else {
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(())
                }
            }
            Some(token) => {
                stream.push(token);
                Err(())
            }
            None => Err(()),
        }
    }
}

impl Parse<proc_macro2::TokenTree> for Char {
    type Error = ();

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
                    } else if inner.starts_with('\\') {
                        // Handle escape sequences
                        match &inner[1..] {
                            "n" => Ok(Char { value: '\n' }),
                            "t" => Ok(Char { value: '\t' }),
                            "r" => Ok(Char { value: '\r' }),
                            "\\" => Ok(Char { value: '\\' }),
                            "'" => Ok(Char { value: '\'' }),
                            "0" => Ok(Char { value: '\0' }),
                            _ => Err(()),
                        }
                    } else {
                        Err(())
                    }
                } else {
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(())
                }
            }
            Some(token) => {
                stream.push(token);
                Err(())
            }
            None => Err(()),
        }
    }
}

impl Parse<proc_macro2::TokenTree> for Integer {
    type Error = ();

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
                        Err(())
                    }
                } else {
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(())
                }
            }
            Some(token) => {
                stream.push(token);
                Err(())
            }
            None => Err(()),
        }
    }
}

impl Parse<proc_macro2::TokenTree> for Float {
    type Error = ();

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
                        Err(())
                    }
                } else {
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(())
                }
            }
            Some(token) => {
                stream.push(token);
                Err(())
            }
            None => Err(()),
        }
    }
}

impl Parse<proc_macro2::TokenTree> for Str {
    type Error = ();

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
                    Err(())
                }
            }
            Some(token) => {
                stream.push(token);
                Err(())
            }
            None => Err(()),
        }
    }
}

impl Parse<proc_macro2::TokenTree> for StrRaw {
    type Error = ();

    fn parse(
        stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
    ) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        match stream.next() {
            Some(proc_macro2::TokenTree::Literal(lit)) => {
                let lit_str = lit.to_string();
                if lit_str.starts_with('r') {
                    // Count hash marks
                    let mut hash_count = 0;
                    let mut chars = lit_str[1..].chars();
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
                        Err(())
                    }
                } else {
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(())
                }
            }
            Some(token) => {
                stream.push(token);
                Err(())
            }
            None => Err(()),
        }
    }
}

impl Parse<proc_macro2::TokenTree> for ByteStr {
    type Error = ();

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
                    Err(())
                }
            }
            Some(token) => {
                stream.push(token);
                Err(())
            }
            None => Err(()),
        }
    }
}

impl Parse<proc_macro2::TokenTree> for ByteStrRaw {
    type Error = ();

    fn parse(
        stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
    ) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        match stream.next() {
            Some(proc_macro2::TokenTree::Literal(lit)) => {
                let lit_str = lit.to_string();
                if lit_str.starts_with("br") {
                    // Count hash marks after "br"
                    let mut hash_count = 0;
                    let mut chars = lit_str[2..].chars();
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
                        Err(())
                    }
                } else {
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(())
                }
            }
            Some(token) => {
                stream.push(token);
                Err(())
            }
            None => Err(()),
        }
    }
}

impl Parse<proc_macro2::TokenTree> for CStr {
    type Error = ();

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
                    Err(())
                }
            }
            Some(token) => {
                stream.push(token);
                Err(())
            }
            None => Err(()),
        }
    }
}

impl Parse<proc_macro2::TokenTree> for CStrRaw {
    type Error = ();

    fn parse(
        stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
    ) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        match stream.next() {
            Some(proc_macro2::TokenTree::Literal(lit)) => {
                let lit_str = lit.to_string();
                if lit_str.starts_with("cr") {
                    // Count hash marks after "cr"
                    let mut hash_count = 0;
                    let mut chars = lit_str[2..].chars();
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
                        Err(())
                    }
                } else {
                    stream.push(proc_macro2::TokenTree::Literal(lit));
                    Err(())
                }
            }
            Some(token) => {
                stream.push(token);
                Err(())
            }
            None => Err(()),
        }
    }
}

impl Unparse<proc_macro2::TokenTree> for Bool {
    fn unparse<S: Emitter<proc_macro2::TokenTree>>(&self, sink: &mut S) -> Result<(), S::Error> {
        let ident = proc_macro2::Ident::new(
            if self.value { "true" } else { "false" },
            proc_macro2::Span::call_site(),
        );
        sink.write_one(proc_macro2::TokenTree::Ident(ident))
    }
}

impl Unparse<proc_macro2::TokenTree> for ByteChar {
    fn unparse<S: Emitter<proc_macro2::TokenTree>>(&self, sink: &mut S) -> Result<(), S::Error> {
        let literal = proc_macro2::Literal::byte_character(self.value);
        sink.write_one(proc_macro2::TokenTree::Literal(literal))
    }
}

impl Unparse<proc_macro2::TokenTree> for Char {
    fn unparse<S: Emitter<proc_macro2::TokenTree>>(&self, sink: &mut S) -> Result<(), S::Error> {
        let literal = proc_macro2::Literal::character(self.value);
        sink.write_one(proc_macro2::TokenTree::Literal(literal))
    }
}

impl Unparse<proc_macro2::TokenTree> for Integer {
    fn unparse<S: Emitter<proc_macro2::TokenTree>>(&self, sink: &mut S) -> Result<(), S::Error> {
        let lit_str = match &self.suffix {
            Some(suffix) => format!("{}{}", self.value, suffix),
            None => self.value.clone(),
        };
        let mut literal = lit_str.parse::<proc_macro2::Literal>()
            .unwrap_or_else(|_| proc_macro2::Literal::i64_unsuffixed(0));
        literal.set_span(proc_macro2::Span::call_site());
        sink.write_one(proc_macro2::TokenTree::Literal(literal))
    }
}

impl Unparse<proc_macro2::TokenTree> for Float {
    fn unparse<S: Emitter<proc_macro2::TokenTree>>(&self, sink: &mut S) -> Result<(), S::Error> {
        let lit_str = match &self.suffix {
            Some(suffix) => format!("{}{}", self.value, suffix),
            None => self.value.clone(),
        };
        let mut literal = lit_str.parse::<proc_macro2::Literal>()
            .unwrap_or_else(|_| proc_macro2::Literal::f64_unsuffixed(0.0));
        literal.set_span(proc_macro2::Span::call_site());
        sink.write_one(proc_macro2::TokenTree::Literal(literal))
    }
}

impl Unparse<proc_macro2::TokenTree> for Str {
    fn unparse<S: Emitter<proc_macro2::TokenTree>>(&self, sink: &mut S) -> Result<(), S::Error> {
        let literal = proc_macro2::Literal::string(&self.value);
        sink.write_one(proc_macro2::TokenTree::Literal(literal))
    }
}

impl Unparse<proc_macro2::TokenTree> for StrRaw {
    fn unparse<S: Emitter<proc_macro2::TokenTree>>(&self, sink: &mut S) -> Result<(), S::Error> {
        let hashes = "#".repeat(self.hash_count);
        let lit_str = format!("r{}\"{}\"{}", hashes, self.value, hashes);
        let mut literal = lit_str.parse::<proc_macro2::Literal>()
            .unwrap_or_else(|_| proc_macro2::Literal::string(&self.value));
        literal.set_span(proc_macro2::Span::call_site());
        sink.write_one(proc_macro2::TokenTree::Literal(literal))
    }
}

impl Unparse<proc_macro2::TokenTree> for ByteStr {
    fn unparse<S: Emitter<proc_macro2::TokenTree>>(&self, sink: &mut S) -> Result<(), S::Error> {
        let literal = proc_macro2::Literal::byte_string(&self.value);
        sink.write_one(proc_macro2::TokenTree::Literal(literal))
    }
}

impl Unparse<proc_macro2::TokenTree> for ByteStrRaw {
    fn unparse<S: Emitter<proc_macro2::TokenTree>>(&self, sink: &mut S) -> Result<(), S::Error> {
        let hashes = "#".repeat(self.hash_count);
        let value_str = String::from_utf8_lossy(&self.value);
        let lit_str = format!("br{}\"{}\"{}", hashes, value_str, hashes);
        let mut literal = lit_str.parse::<proc_macro2::Literal>()
            .unwrap_or_else(|_| proc_macro2::Literal::byte_string(&self.value));
        literal.set_span(proc_macro2::Span::call_site());
        sink.write_one(proc_macro2::TokenTree::Literal(literal))
    }
}

impl Unparse<proc_macro2::TokenTree> for CStr {
    fn unparse<S: Emitter<proc_macro2::TokenTree>>(&self, sink: &mut S) -> Result<(), S::Error> {
        let lit_str = format!("c\"{}\"", self.value);
        let mut literal = lit_str.parse::<proc_macro2::Literal>()
            .unwrap_or_else(|_| proc_macro2::Literal::string(&self.value));
        literal.set_span(proc_macro2::Span::call_site());
        sink.write_one(proc_macro2::TokenTree::Literal(literal))
    }
}

impl Unparse<proc_macro2::TokenTree> for CStrRaw {
    fn unparse<S: Emitter<proc_macro2::TokenTree>>(&self, sink: &mut S) -> Result<(), S::Error> {
        let hashes = "#".repeat(self.hash_count);
        let lit_str = format!("cr{}\"{}\"{}", hashes, self.value, hashes);
        let mut literal = lit_str.parse::<proc_macro2::Literal>()
            .unwrap_or_else(|_| proc_macro2::Literal::string(&self.value));
        literal.set_span(proc_macro2::Span::call_site());
        sink.write_one(proc_macro2::TokenTree::Literal(literal))
    }
}

// Display implementations
impl std::fmt::Display for Bool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", if self.value { "true" } else { "false" })
    }
}

impl std::fmt::Display for ByteChar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Handle common escape sequences
        match self.value {
            b'\n' => write!(f, "b'\\n'"),
            b'\t' => write!(f, "b'\\t'"),
            b'\r' => write!(f, "b'\\r'"),
            b'\\' => write!(f, "b'\\\\'"),
            b'\'' => write!(f, "b'\\''"),
            0 => write!(f, "b'\\0'"),
            b if b.is_ascii_graphic() || b == b' ' => write!(f, "b'{}'", b as char),
            b => write!(f, "b'\\x{:02x}'", b),
        }
    }
}

impl std::fmt::Display for Char {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Handle common escape sequences
        match self.value {
            '\n' => write!(f, "'\\n'"),
            '\t' => write!(f, "'\\t'"),
            '\r' => write!(f, "'\\r'"),
            '\\' => write!(f, "'\\\\'"),
            '\'' => write!(f, "'\\''"),
            '\0' => write!(f, "'\\0'"),
            c => write!(f, "'{}'", c),
        }
    }
}

impl std::fmt::Display for Integer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.suffix {
            Some(suffix) => write!(f, "{}{}", self.value, suffix),
            None => write!(f, "{}", self.value),
        }
    }
}

impl std::fmt::Display for Float {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.suffix {
            Some(suffix) => write!(f, "{}{}", self.value, suffix),
            None => write!(f, "{}", self.value),
        }
    }
}

impl std::fmt::Display for Str {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\"", self.value.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

impl std::fmt::Display for StrRaw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hashes = "#".repeat(self.hash_count);
        write!(f, "r{}\"{}\"{}", hashes, self.value, hashes)
    }
}

impl std::fmt::Display for ByteStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "b\"")?;
        for &byte in &self.value {
            match byte {
                b'\n' => write!(f, "\\n")?,
                b'\t' => write!(f, "\\t")?,
                b'\r' => write!(f, "\\r")?,
                b'\\' => write!(f, "\\\\")?,
                b'"' => write!(f, "\\\"")?,
                b if b.is_ascii_graphic() || b == b' ' => write!(f, "{}", b as char)?,
                b => write!(f, "\\x{:02x}", b)?,
            }
        }
        write!(f, "\"")
    }
}

impl std::fmt::Display for ByteStrRaw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hashes = "#".repeat(self.hash_count);
        let value_str = String::from_utf8_lossy(&self.value);
        write!(f, "br{}\"{}\"{}", hashes, value_str, hashes)
    }
}

impl std::fmt::Display for CStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "c\"{}\"", self.value.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

impl std::fmt::Display for CStrRaw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hashes = "#".repeat(self.hash_count);
        write!(f, "cr{}\"{}\"{}", hashes, self.value, hashes)
    }
}

