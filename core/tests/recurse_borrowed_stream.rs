//! Parsing a `#[recurse]` cycle from a parse stream whose type carries a **non-`'static` lifetime**.
//!
//! Every source shipped with syan (`String`, `proc_macro2::TokenStream`) owns its input, so the
//! `Atom`, the `Span` and the stream are all `'static` and the recursion machinery is never asked to
//! carry a borrow. This test builds a source that borrows instead — `Tok<'a>` and `Sp<'a>` both hold
//! `&'a str` into the caller's buffer — and parses a mutually recursive `Expr` ↔ `Stmt` cycle out of
//! it.
//!
//! The part that could plausibly break is the recursion boundary. It used to be `syan::parse::erase`,
//! which `#[recurse]` wrapped around every field-parse call to pin the callee's stream type to one
//! `&mut dyn ParseStream` layer — and a trait object defaults to a `'static` bound, so an erasure
//! returning `&mut dyn ParseStream<…>` rather than `&mut (dyn ParseStream<…> + 'a)` would have
//! rejected every borrowed stream. The boundary is now a plain reborrow (`&mut *stream`), which has no
//! `'static` default to get wrong, but the test still earns its keep: it is the only one where the
//! stream, the atom and the span all carry a caller-chosen `'a`. The signatures in
//! `parse_expr`/`parse_stmt` below are the assertion — written with no `'static` anywhere, they do not
//! compile unless the borrow threads all the way through the cycle.

use syan::error::ParseError;
use syan::parse::{recurse, Parse, ParseStream};

// ---------------------------------------------------------------------------------------------
// A borrowed source
// ---------------------------------------------------------------------------------------------

/// A span that points into the source text rather than owning a copy of it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sp<'a> {
    pub text: &'a str,
}

impl<'a> syan::span::Span for Sp<'a> {
    fn migrate(self, other: Self) -> Self {
        // Widest wins; enough to satisfy the trait, the choice is not what's under test.
        if other.text.len() > self.text.len() {
            other
        } else {
            self
        }
    }
}

/// One whitespace-separated token, borrowing its text.
#[derive(Clone, Debug)]
pub struct Tok<'a> {
    pub text: &'a str,
}

impl<'a> syan::span::Spanned for Tok<'a> {
    type Span = Sp<'a>;

    fn span(&self) -> Sp<'a> {
        Sp { text: self.text }
    }
}

/// Lex by splitting on whitespace. The tokens borrow `src`.
pub fn lex(src: &str) -> Vec<Tok<'_>> {
    src.split_whitespace().map(|text| Tok { text }).collect()
}

/// The stream's own error type — it **borrows** the offending text, so `Self::Error` carries a
/// lifetime too. The recursion boundary reborrows the stream as `&mut S`, so `S::Error` travels with it, and
/// `Error` is a binding on that trait object exactly like `Atom` is; giving it a lifetime is what
/// checks that the binding survives erasure rather than being quietly required to be `'static`.
#[derive(Clone, Debug, PartialEq)]
pub struct LexError<'a> {
    pub at: &'a str,
}

/// Third-party errors reach the universal error through `From`, which replaced the old
/// `Error::into_parse_error` — that method named `ParseError` concretely and could not survive it
/// gaining a span parameter.
impl<'a> From<LexError<'a>> for ParseError<Sp<'a>> {
    fn from(e: LexError<'a>) -> Self {
        ParseError::other(Sp { text: e.at }, "lex error")
    }
}

impl<'a> syan::error::Error for LexError<'a> {
    fn from_cause(cause: Vec<Self>) -> Self {
        // Keep the first cause's location; the choice is not what's under test.
        cause.into_iter().next().unwrap_or(LexError { at: "" })
    }


}

/// A stream over a borrowed slice. TWO independent non-`'static` lifetimes: `'s` borrows the token
/// slice, `'a` is the source text the atoms point into. Keeping them apart is what lets the parsed
/// AST outlive the token buffer (see `the_ast_outlives_the_token_buffer`) — with a single lifetime
/// the two would be conflated and that property could not even be stated.
pub struct SliceStream<'s, 'a> {
    toks: &'s [Tok<'a>],
    pos: usize,
    /// Atoms handed back by `push` (backtracking), most recent first.
    buf: Vec<Tok<'a>>,
    /// One `(pos, buf)` snapshot per open checkpoint — a hand-rolled trio rather than
    /// `syan::parse::Tape`, since this stream indexes a slice it already has rather than pulling
    /// from an iterator. Keeping one impl off `Tape` also keeps the trio's contract under test
    /// independently of that helper.
    saves: Vec<(usize, Vec<Tok<'a>>)>,
}

impl<'s, 'a> SliceStream<'s, 'a> {
    pub fn new(toks: &'s [Tok<'a>]) -> Self {
        Self {
            toks,
            pos: 0,
            buf: Vec::new(),
            saves: Vec::new(),
        }
    }
}

// Both lifetimes are named in the impl header, and BOTH associated types carry one: `Atom = Tok<'a>`
// and `Error = LexError<'a>`. `'s` (the token-slice borrow) appears only in `Self` and is therefore
// never named at the recursion boundary — all that survives of it is the well-formedness obligation that
// the stream outlive the `&mut` borrow.
impl<'s, 'a> ParseStream for SliceStream<'s, 'a> {
    type Atom = Tok<'a>;
    type Error = LexError<'a>;

    fn next(&mut self) -> Option<Tok<'a>> {
        if let Some(buffered) = self.buf.pop() {
            return Some(buffered);
        }
        let tok = self.toks.get(self.pos)?.clone();
        self.pos += 1;
        Some(tok)
    }

    fn peek(&mut self) -> Option<&Tok<'a>> {
        if self.buf.is_empty() {
            self.toks.get(self.pos)
        } else {
            self.buf.last()
        }
    }

    fn push(&mut self, atom: Tok<'a>) {
        self.buf.push(atom);
    }

    fn checkpoint_raw(&mut self) -> u64 {
        self.saves.push((self.pos, self.buf.clone()));
        (self.saves.len() - 1) as u64
    }

    fn rollback_raw(&mut self, raw: u64) {
        self.saves.truncate(raw as usize + 1);
        if let Some((pos, buf)) = self.saves.pop() {
            self.pos = pos;
            self.buf = buf;
        }
    }

    fn commit_raw(&mut self, raw: u64) {
        self.saves.truncate(raw as usize);
    }

    fn get_error(&mut self) -> Result<(), LexError<'a>> {
        Ok(())
    }

    fn skip_sep(&mut self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------------------------
// Leaf types, hand-written against the borrowed atom
// ---------------------------------------------------------------------------------------------

/// Consume one token whose text equals `TEXT`, keeping a borrowed span.
macro_rules! literal_leaf {
    ($name:ident, $text:literal) => {
        #[derive(Clone, Debug, PartialEq)]
        pub struct $name<S> {
            pub span: S,
        }

        impl<'a> Parse<Tok<'a>> for $name<Sp<'a>> {
            type Error = ParseError<Sp<'a>>;

            fn parse_stream<__S: syan::parse::ParseStream<Atom = Tok<'a>>>(stream: &mut __S) -> Result<Self, ParseError<Sp<'a>>> {
                match stream.next() {
                    Some(tok) if tok.text == $text => Ok($name {
                        span: Sp { text: tok.text },
                    }),
                    Some(tok) => {
                        // Put it back so an enclosing `dup` sees an untouched stream.
                        stream.push(tok);
                        Err(ParseError::other(Sp::default(), concat!("expected `", $text, "`")))
                    }
                    None => Err(ParseError::other(
                        Sp::default(),
                        concat!("expected `", $text, "`, found end of input"),
                    )),
                }
            }
        }
    };
}

literal_leaf!(LParen, "(");
literal_leaf!(RParen, ")");

/// An identifier-ish token. Its `text` is owned, but its `span` still borrows.
#[derive(Clone, Debug, PartialEq)]
pub struct Word<S> {
    pub text: String,
    pub span: S,
}

impl<'a> Parse<Tok<'a>> for Word<Sp<'a>> {
    type Error = ParseError<Sp<'a>>;

    fn parse_stream<__S: syan::parse::ParseStream<Atom = Tok<'a>>>(stream: &mut __S) -> Result<Self, ParseError<Sp<'a>>> {
        match stream.next() {
            Some(tok) if tok.text.chars().all(char::is_alphanumeric) => Ok(Word {
                text: tok.text.to_string(),
                span: Sp { text: tok.text },
            }),
            Some(tok) => {
                stream.push(tok);
                Err(ParseError::other(Sp::default(), "expected a word"))
            }
            None => Err(ParseError::other(
                Sp::default(),
                "expected a word, found end of input",
            )),
        }
    }
}

/// Like [`Word`], but it **borrows** its text instead of copying it — so a type holding one needs a
/// lifetime parameter of its own.
#[derive(Clone, Debug, PartialEq)]
pub struct Ref<'a, S> {
    pub text: &'a str,
    pub span: S,
}

impl<'a> Parse<Tok<'a>> for Ref<'a, Sp<'a>> {
    type Error = ParseError<Sp<'a>>;

    fn parse_stream<__S: syan::parse::ParseStream<Atom = Tok<'a>>>(stream: &mut __S) -> Result<Self, ParseError<Sp<'a>>> {
        match stream.next() {
            Some(tok) if tok.text.chars().all(char::is_alphanumeric) => Ok(Ref {
                text: tok.text,
                span: Sp { text: tok.text },
            }),
            Some(tok) => {
                stream.push(tok);
                Err(ParseError::other(Sp::default(), "expected a word"))
            }
            None => Err(ParseError::other(
                Sp::default(),
                "expected a word, found end of input",
            )),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The cycle
// ---------------------------------------------------------------------------------------------

// `Expr` reaches `Stmt` and back, so this is a genuine two-type cycle routed through decycle. The
// leaves are named through `super::`, which is also the shape decycle's nesting aliasing has to keep
// resolvable once it re-emits these impls two modules deep.
#[recurse]
mod ast {
    use super::{LParen, RParen, Word};
    use syan::parse::Parse;

    #[derive(Parse, Debug, PartialEq)]
    pub enum Expr<S> {
        /// `( <stmt> )`
        Group {
            open: LParen<S>,
            inner: Box<Stmt<S>>,
            close: RParen<S>,
        },
        Leaf(Word<S>),
    }

    #[derive(Parse, Debug, PartialEq)]
    pub struct Stmt<S> {
        pub expr: Box<Expr<S>>,
    }
}

// The same cycle, but the types carry a **lifetime parameter of their own** rather than reaching the
// borrow only through the span type argument. `'a` has to survive everything decycle generates — the
// ranked twin traits, the rank ladder, the delegating impl and the re-entry helpers all re-emit these
// generics — which the `Expr<S>` cycle above never checks, since there `S` is just an opaque type arg
// that happens to be instantiated with a borrowing type.
#[recurse]
mod ast_lt {
    use super::{LParen, RParen, Ref};
    use syan::parse::Parse;

    #[derive(Parse, Debug, PartialEq)]
    pub enum Expr<'a, S> {
        Group {
            open: LParen<S>,
            inner: Box<Stmt<'a, S>>,
            close: RParen<S>,
        },
        Leaf(Ref<'a, S>),
    }

    #[derive(Parse, Debug, PartialEq)]
    pub struct Stmt<'a, S> {
        pub expr: Box<Expr<'a, S>>,
    }
}

// ---------------------------------------------------------------------------------------------
// The assertions
// ---------------------------------------------------------------------------------------------

// These two signatures ARE the test: `'a` is the caller's, there is no `'static` anywhere, and the
// returned AST borrows from the same buffer the stream did. If anything on the
// recursion path) demanded `'static`, neither would compile.
fn parse_expr<'a>(toks: &[Tok<'a>]) -> Result<ast::Expr<Sp<'a>>, ParseError<Sp<'a>>> {
    Parse::parse(SliceStream::new(toks))
}

fn parse_stmt<'a>(toks: &[Tok<'a>]) -> Result<ast::Stmt<Sp<'a>>, ParseError<Sp<'a>>> {
    Parse::parse(SliceStream::new(toks))
}

#[test]
fn leaf_from_borrowed_stream() {
    let src = String::from("hello");
    let toks = lex(&src);
    let expr = parse_expr(&toks).unwrap();
    let ast::Expr::Leaf(word) = expr else {
        panic!("expected a leaf")
    };
    assert_eq!(word.text, "hello");
    // The span really points into `src`, not at a copy.
    assert!(std::ptr::eq(word.span.text, &src[..]));
}

#[test]
fn deep_recursion_through_the_cycle() {
    // Each `( … )` is one full Expr -> Stmt -> Expr turn of the cycle, so the borrow has to survive
    // an arbitrary number of re-entries, not just one.
    const DEPTH: usize = 40;
    let src = format!("{} core {}", "( ".repeat(DEPTH), ") ".repeat(DEPTH));
    let toks = lex(&src);

    let parsed = parse_expr(&toks).unwrap();

    let mut cur = &parsed;
    for depth in 0..DEPTH {
        let ast::Expr::Group { inner, .. } = cur else {
            panic!("expected a group at depth {depth}")
        };
        cur = &inner.expr;
    }
    let ast::Expr::Leaf(word) = cur else {
        panic!("expected the innermost leaf")
    };
    assert_eq!(word.text, "core");
    let at = src.find("core").unwrap();
    assert!(std::ptr::eq(word.span.text, &src[at..at + "core".len()]));
}

#[test]
fn entry_at_the_other_cycle_member() {
    // Entering at `Stmt` rather than `Expr` exercises the other direction of the same cycle.
    let src = String::from("( ( leaf ) )");
    let toks = lex(&src);
    let stmt = parse_stmt(&toks).unwrap();
    let ast::Expr::Group { inner, .. } = *stmt.expr else {
        panic!("expected a group")
    };
    let ast::Expr::Group { inner, .. } = *inner.expr else {
        panic!("expected a nested group")
    };
    let ast::Expr::Leaf(word) = *inner.expr else {
        panic!("expected a leaf")
    };
    assert_eq!(word.text, "leaf");
}

#[test]
fn the_ast_outlives_the_token_buffer() {
    // The stream is dropped inside `parse_expr` and the token slice right after; only `src` — the
    // buffer the spans actually point into — has to stay alive. This is the property a
    // a `'static`-bounded recursion boundary would make impossible to express.
    let src = String::from("( kept )");
    let toks = lex(&src);
    let expr = parse_expr(&toks).unwrap();
    drop(toks);
    let ast::Expr::Group { inner, .. } = expr else {
        panic!("expected a group")
    };
    let ast::Expr::Leaf(word) = *inner.expr else {
        panic!("expected a leaf")
    };
    assert_eq!(word.text, "kept");
}

// ---------------------------------------------------------------------------------------------
// The stream reached through a borrow, and a cycle carrying its own lifetime parameter
// ---------------------------------------------------------------------------------------------

// Entry through `&mut stream` rather than by value: the top-level stream type is then
// `&'m mut SliceStream<'s, 'a>` — a THIRD non-`'static` lifetime, resolved by the blanket
// `impl<T: ?Sized + ParseStream> ParseStream for &mut T`. This is also the shape a reborrow produces
// internally, so it checks that the entry point and the recursion boundary agree on it.
fn parse_expr_borrowed<'m, 's, 'a>(
    stream: &'m mut SliceStream<'s, 'a>,
) -> Result<ast::Expr<Sp<'a>>, ParseError<Sp<'a>>> {
    Parse::parse(stream)
}

// The cycle whose types declare `'a` themselves.
fn parse_expr_lt<'a>(toks: &[Tok<'a>]) -> Result<ast_lt::Expr<'a, Sp<'a>>, ParseError<Sp<'a>>> {
    Parse::parse(SliceStream::new(toks))
}

fn parse_stmt_lt<'a>(toks: &[Tok<'a>]) -> Result<ast_lt::Stmt<'a, Sp<'a>>, ParseError<Sp<'a>>> {
    Parse::parse(SliceStream::new(toks))
}

#[test]
fn entry_through_a_borrowed_stream() {
    let src = String::from("( borrowed )");
    let toks = lex(&src);
    let mut stream = SliceStream::new(&toks);
    let expr = parse_expr_borrowed(&mut stream).unwrap();
    // The stream is still ours afterwards — it was borrowed, not consumed.
    assert!(stream.peek().is_none(), "the whole input should be consumed");
    let ast::Expr::Group { inner, .. } = expr else {
        panic!("expected a group")
    };
    let ast::Expr::Leaf(word) = *inner.expr else {
        panic!("expected a leaf")
    };
    assert_eq!(word.text, "borrowed");
}

#[test]
fn lifetime_parameterised_cycle_leaf() {
    let src = String::from("token");
    let toks = lex(&src);
    let expr = parse_expr_lt(&toks).unwrap();
    let ast_lt::Expr::Leaf(r) = expr else {
        panic!("expected a leaf")
    };
    // `text` is a borrow now, not a copy: the AST's OWN lifetime parameter carries it.
    assert!(std::ptr::eq(r.text, &src[..]));
}

#[test]
fn lifetime_parameterised_cycle_deep() {
    const DEPTH: usize = 40;
    let src = format!("{} deep {}", "( ".repeat(DEPTH), ") ".repeat(DEPTH));
    let toks = lex(&src);
    let parsed = parse_expr_lt(&toks).unwrap();

    let mut cur = &parsed;
    for depth in 0..DEPTH {
        let ast_lt::Expr::Group { inner, .. } = cur else {
            panic!("expected a group at depth {depth}")
        };
        cur = &inner.expr;
    }
    let ast_lt::Expr::Leaf(r) = cur else {
        panic!("expected the innermost leaf")
    };
    assert_eq!(r.text, "deep");
    let at = src.find("deep").unwrap();
    assert!(std::ptr::eq(r.text, &src[at..at + "deep".len()]));
}

#[test]
fn lifetime_parameterised_cycle_outlives_the_token_buffer() {
    let src = String::from("( ( kept ) )");
    let toks = lex(&src);
    let stmt = parse_stmt_lt(&toks).unwrap();
    drop(toks);
    let ast_lt::Expr::Group { inner, .. } = *stmt.expr else {
        panic!("expected a group")
    };
    let ast_lt::Expr::Group { inner, .. } = *inner.expr else {
        panic!("expected a nested group")
    };
    let ast_lt::Expr::Leaf(r) = *inner.expr else {
        panic!("expected a leaf")
    };
    assert!(std::ptr::eq(r.text, &src[src.find("kept").unwrap()..][.."kept".len()]));
}
