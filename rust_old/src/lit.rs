use syan::{
    span::WithSpan,
    source::proc_macro2::literal::*,
    parse::{Parse, Unparse},
};
use type_macro_derive_tricks::macro_derive;

/// Wrapper for Float that implements PartialEq, Eq, Hash
#[derive(Clone, Debug)]
pub struct FloatWrapper(pub Float);

impl PartialEq for FloatWrapper {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_string() == other.0.to_string()
    }
}

impl Eq for FloatWrapper {}

impl std::hash::Hash for FloatWrapper {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_string().hash(state);
    }
}

/// A Rust literal
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum Lit<S> {
    Str(WithSpan<Str, S>),
    StrRaw(WithSpan<StrRaw, S>),
    ByteStr(WithSpan<ByteStr, S>),
    ByteStrRaw(WithSpan<ByteStrRaw, S>),
    CStr(WithSpan<CStr, S>),
    CStrRaw(WithSpan<CStrRaw, S>),
    Byte(WithSpan<ByteChar, S>),
    Char(WithSpan<Char, S>),
    Int(WithSpan<Integer, S>),
    Float(WithSpan<FloatWrapper, S>),
    Bool(WithSpan<Bool, S>),
}
