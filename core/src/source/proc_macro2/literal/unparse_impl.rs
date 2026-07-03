use super::*;

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
        let mut literal = lit_str
            .parse::<proc_macro2::Literal>()
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
        let mut literal = lit_str
            .parse::<proc_macro2::Literal>()
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
        let mut literal = lit_str
            .parse::<proc_macro2::Literal>()
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
        let mut literal = lit_str
            .parse::<proc_macro2::Literal>()
            .unwrap_or_else(|_| proc_macro2::Literal::byte_string(&self.value));
        literal.set_span(proc_macro2::Span::call_site());
        sink.write_one(proc_macro2::TokenTree::Literal(literal))
    }
}

impl Unparse<proc_macro2::TokenTree> for CStr {
    fn unparse<S: Emitter<proc_macro2::TokenTree>>(&self, sink: &mut S) -> Result<(), S::Error> {
        let lit_str = format!("c\"{}\"", self.value);
        let mut literal = lit_str
            .parse::<proc_macro2::Literal>()
            .unwrap_or_else(|_| proc_macro2::Literal::string(&self.value));
        literal.set_span(proc_macro2::Span::call_site());
        sink.write_one(proc_macro2::TokenTree::Literal(literal))
    }
}

impl Unparse<proc_macro2::TokenTree> for CStrRaw {
    fn unparse<S: Emitter<proc_macro2::TokenTree>>(&self, sink: &mut S) -> Result<(), S::Error> {
        let hashes = "#".repeat(self.hash_count);
        let lit_str = format!("cr{}\"{}\"{}", hashes, self.value, hashes);
        let mut literal = lit_str
            .parse::<proc_macro2::Literal>()
            .unwrap_or_else(|_| proc_macro2::Literal::string(&self.value));
        literal.set_span(proc_macro2::Span::call_site());
        sink.write_one(proc_macro2::TokenTree::Literal(literal))
    }
}
