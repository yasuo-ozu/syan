use syan::error::ParseError;
use syan::parse::{Parse, Unparse};
use syan::source::proc_macro2::Span;

pub struct Ident<S>(String, S);

impl<S> core::hash::Hash for Ident<S> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

#[cfg(feature = "proc_macro2")]
impl Parse<proc_macro2::TokenTree> for Ident<Span> {
    type Error = ParseError;

    fn parse(
        stream: impl syan::parse::IntoParseStream<Atom = proc_macro2::TokenTree>,
    ) -> Result<Self, Self::Error> {
        use proc_macro2::TokenTree;
        use syan::parse::ParseStream;

        let mut stream = stream.into_parse_stream();
        let span = match stream.next() {
            Some(TokenTree::Ident(ident)) => {
                return Ok(Ident(format!({}, &ident), ident.span().into()))
            }
            Some(o) => {
                let span = o.span().into();
                stream.push(o);
                span
            }
            _ => Default::default(),
        };
        Err(ParseError::new(span, "Bad ident"))
    }
}
