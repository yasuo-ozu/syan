use syan::{
    parse::{Parse, ParseStream, Unparse, IntoParseStream},
    span::{Span, Spanned},
    nested::{Choice, Punctuated},
};

pub mod item;
pub mod expr;
pub mod stmt;
pub mod ty;
pub mod pat;
pub mod path;
pub mod lit;
pub mod token;

pub use item::*;
pub use expr::*;
pub use stmt::*;
pub use ty::*;
pub use pat::*;
pub use path::*;
pub use lit::*;
pub use token::*;

/// Top-level Rust source file
#[derive(Debug, Clone)]
pub struct File<S: Span> {
    pub items: Vec<Item<S>>,
}

impl<S: Span> Spanned for File<S> {
    type Span = S;
    
    fn span(&self) -> Self::Span {
        self.items.span()
    }
}

impl<Atom: Spanned<Span = S>, S: Span> Parse<Atom> for File<S>
where
    Item<S>: Parse<Atom, Error = ()>,
{
    type Error = ();
    
    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        let mut items = Vec::new();
        
        while stream.peek().is_some() {
            items.push(Item::parse(&mut stream)?);
        }
        
        Ok(File { items })
    }
}

impl<Atom, S: Span> Unparse<Atom> for File<S>
where
    Item<S>: Unparse<Atom>,
{
    fn unparse<SS: syan::parse::unparse::Emitter<Atom>>(
        &self,
        sink: &mut SS,
    ) -> Result<(), SS::Error> {
        for item in &self.items {
            item.unparse(sink)?;
        }
        Ok(())
    }
}