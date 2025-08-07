use syan::{parse::ParseStream as SyanParseStream, span::Spanned};

/// Wrapper around proc_macro2::Span that implements syan's Span trait
#[derive(Clone, Debug, Default)]
pub struct Span(Option<proc_macro2::Span>);

impl syan::span::Span for Span {
    fn migrate(self, other: Self) -> Self {
        match (self.0, other.0) {
            (None, other) => Span(other),
            (span @ Some(_), None) => Span(span),
            (Some(lhs), Some(rhs)) => {
                // Join spans if possible
                Span(Some(lhs.join(rhs).unwrap_or(lhs)))
            }
        }
    }
}

impl From<proc_macro2::Span> for Span {
    fn from(span: proc_macro2::Span) -> Self {
        Span(Some(span))
    }
}

impl From<Span> for Option<proc_macro2::Span> {
    fn from(span: Span) -> Self {
        span.0
    }
}

/// Wrapper around proc_macro2::TokenStream that implements syan's ParseStream trait
pub struct ParseStream {
    tokens: std::collections::VecDeque<proc_macro2::TokenTree>,
    errors: Vec<syn::Error>,
}

impl ParseStream {
    pub fn new(tokens: proc_macro2::TokenStream) -> Self {
        Self {
            tokens: tokens.into_iter().collect(),
            errors: Vec::new(),
        }
    }

    pub fn errors(&self) -> &[syn::Error] {
        &self.errors
    }
}

impl SyanParseStream for ParseStream {
    type Atom = TokenTree;
    type Error = syn::Error;

    fn next(&mut self) -> Option<Self::Atom> {
        self.tokens.pop_front().map(TokenTree)
    }

    fn peek(&mut self) -> Option<&Self::Atom> {
        // Since we can't return a reference to a wrapped value easily,
        // we'll need to store peeked values
        // This is a simplified implementation
        None
    }

    fn push(&mut self, atom: Self::Atom) {
        self.tokens.push_front(atom.0);
    }

    fn get_error(&mut self) -> Result<(), Self::Error> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors.remove(0))
        }
    }
}

/// Wrapper around proc_macro2::TokenTree that implements Spanned
#[derive(Clone, Debug)]
pub struct TokenTree(pub proc_macro2::TokenTree);

impl Spanned for TokenTree {
    type Span = Span;

    fn span(&self) -> Self::Span {
        Span(Some(self.0.span()))
    }
}

impl From<proc_macro2::TokenTree> for TokenTree {
    fn from(tree: proc_macro2::TokenTree) -> Self {
        TokenTree(tree)
    }
}

impl From<TokenTree> for proc_macro2::TokenTree {
    fn from(tree: TokenTree) -> Self {
        tree.0
    }
}