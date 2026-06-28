//! A **small** parseable subset of Rust — enough statements/expressions to round-trip a non-trivial
//! snippet and to drive a `visitor!`. Exercises the whole stack at once: `#[recurse]` (the `Stmt`/`Expr`
//! mutual cycle), `Group` delimiters (`{ … }`, `( … )`), `#[group]` repetition, literal/keyword/punct
//! tokens via `#[macro_derive]` + `Token!`, and the generated `Visit` API.
//!
//! Grammar:
//! ```text
//! Stmt  = "let" Ident "=" Expr ";"  |  Expr ";"
//! Expr  = "{" Stmt* "}"  |  "(" Expr ")"  |  IntLiteral  |  Ident
//! ```

use syan::error::ParseError;
use syan::parse::unparse::Emitter;
use syan::parse::{IntoParseStream, Parse, ParseStream, Unparse};

/// An arbitrary identifier (a binding or variable name), parsed from one `Ident` token. A leaf for the
/// grammar — the core has no general identifier parser, so it's hand-written here.
#[derive(Clone, Debug)]
pub struct Ident {
    pub name: String,
    span: proc_macro2::Span,
}

impl Ident {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), span: proc_macro2::Span::call_site() }
    }
}

// Compare by name only (the span is positional, not semantic).
impl PartialEq for Ident {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl Eq for Ident {}

impl Parse<proc_macro2::TokenTree> for Ident {
    type Error = ParseError;

    fn parse(
        stream: impl IntoParseStream<Atom = proc_macro2::TokenTree>,
    ) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        match stream.next() {
            Some(proc_macro2::TokenTree::Ident(id)) => {
                Ok(Ident { name: id.to_string(), span: id.span() })
            }
            Some(other) => {
                stream.push(other);
                Err(ParseError::new((), "expected an identifier"))
            }
            None => Err(ParseError::new((), "expected an identifier, found end of input")),
        }
    }
}

impl Unparse<proc_macro2::TokenTree> for Ident {
    fn unparse<S: Emitter<proc_macro2::TokenTree>>(&self, sink: &mut S) -> Result<(), S::Error> {
        sink.write_one(proc_macro2::TokenTree::Ident(proc_macro2::Ident::new(
            &self.name, self.span,
        )))
    }
}

/// The recursive `Stmt`/`Expr` cycle. `#[recurse]` turns it into natural recursive types whose
/// `Parse`/`Unparse` are delegated through a depth-limited engine.
//
// The engine's depth-limited `Parse` truncates nesting past `limit`, so the limit caps how deeply
// blocks/parens may nest in a round-trippable snippet. A *flat* program (many sibling statements) stays
// shallow regardless of statement count, so `limit` need not be huge. (Too large would overflow rustc's
// trait-solver recursion limit on the depth-`limit` engine type.)
//
// The grammar types use `#[macro_derive]` (from `type-macro-derive-tricks`) rather than `#[derive]`
// because their fields contain `Token![…]` *type-position macros*, which rustc forbids under a plain
// `#[derive]`. `#[recurse]` recognizes `#[macro_derive]` and routes the engine derives through it too.
#[syan::parse::recurse(limit = 12)]
pub mod ast {
    use super::Ident;
    use syan::nested::group::{GroupBrace, GroupParen};
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;
    use syan::symbol::Token;
    use syan::visit::Ast;
    use type_macro_derive_tricks::macro_derive;

    /// `let NAME = EXPR ;` or `EXPR ;`.
    #[macro_derive(Ast, Parse, Unparse, Debug)]
    #[subast(crate::rustsub::ast::Expr)]
    pub enum Stmt<S> {
        Let {
            let_kw: Token![S => let],
            name: Ident,
            eq: Token![S => =],
            value: Expr<S>,
            semi: Token![S => ;],
        },
        Expr {
            value: Expr<S>,
            semi: Token![S => ;],
        },
    }

    /// `{ stmts… }`, `( expr )`, an integer literal, or a variable. The recursive children sit in a
    /// `#[group]` field next to their delimiter (a peelable `Vec`/`Box`), which the engine conversion
    /// understands — a recursive child *inside* a `Group<…>` slot is not supported.
    #[macro_derive(Ast, Parse, Unparse, Debug)]
    #[subast(crate::rustsub::ast::Stmt)]
    pub enum Expr<S> {
        Block {
            brace: GroupBrace<(), S>,
            #[group(self.brace)]
            stmts: Vec<Stmt<S>>,
        },
        Paren {
            paren: GroupParen<(), S>,
            #[group(self.paren)]
            inner: Box<Expr<S>>,
        },
        Lit(Integer),
        Var(Ident),
    }
}

/// A `Visit`/`VisitMut` over the grammar (closures, struct visitors, `.visit()`).
pub mod visit {
    syan::visit::visitor!(crate::rustsub::ast::Stmt, crate::rustsub::ast::Expr);
}
