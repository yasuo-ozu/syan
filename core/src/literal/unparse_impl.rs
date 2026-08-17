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

/// Shared by the `Parse<proc_macro2::Literal>`-roundtrippable impls: parse `s` as a proc-macro2
/// literal (falling back to `fallback()` if `s` doesn't lex as one), stamp it with the call-site
/// span, and emit it.
fn emit_parsed<S: Emitter<proc_macro2::TokenTree>>(
    sink: &mut S,
    s: String,
    fallback: impl FnOnce() -> proc_macro2::Literal,
) -> Result<(), S::Error> {
    let mut literal = s
        .parse::<proc_macro2::Literal>()
        .unwrap_or_else(|_| fallback());
    literal.set_span(proc_macro2::Span::call_site());
    sink.write_one(proc_macro2::TokenTree::Literal(literal))
}

impl Unparse<proc_macro2::TokenTree> for Integer {
    fn unparse<S: Emitter<proc_macro2::TokenTree>>(&self, sink: &mut S) -> Result<(), S::Error> {
        emit_parsed(sink, self.to_string(), || {
            proc_macro2::Literal::i64_unsuffixed(0)
        })
    }
}

impl Unparse<proc_macro2::TokenTree> for Float {
    fn unparse<S: Emitter<proc_macro2::TokenTree>>(&self, sink: &mut S) -> Result<(), S::Error> {
        emit_parsed(sink, self.to_string(), || {
            proc_macro2::Literal::f64_unsuffixed(0.0)
        })
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
        emit_parsed(sink, self.to_string(), || {
            proc_macro2::Literal::string(&self.value)
        })
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
        emit_parsed(sink, self.to_string(), || {
            proc_macro2::Literal::byte_string(&self.value)
        })
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
        emit_parsed(sink, self.to_string(), || {
            proc_macro2::Literal::string(&self.value)
        })
    }
}
