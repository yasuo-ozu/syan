//! A tiny formatter, with comments.
//!
//!     cargo run --example formatter
//!
//! Comments are the part that makes a formatter more than a pretty-printer:
//! they are not in the tree. `Parse` yields tokens, so anything between two
//! tokens — spacing, newlines, comments — is gone by the time you walk it.
//!
//! The way out is that every token carries a SPAN. So the tree gives the
//! structure and the original source gives the trivia: before emitting a
//! token at offset `loc`, flush whatever comments the source holds ahead of
//! it. The parser and the printer read the same text, one for shape and one
//! for what the shape left out.
//!
//! One wrinkle first. `/` opens both a comment and a division, so the parser
//! must not see comments at all — untouched, `1 + /* c */ 2` parses as `1`
//! (the `/` matches `Op::Div`, `Rest` then fails on `*`, and the `Vec` stops
//! quietly), and the formatter deletes half the expression. `blank` replaces
//! each comment with spaces of the same length, so the parser sees only
//! whitespace and every span still indexes the ORIGINAL source.

use syan::parse::Parse;
use syan::source::string::Span;

#[syan::parse::recurse]
pub mod ast {
    use syan::literal::Integer;
    use syan::nested::group::GroupParen;
    use syan::parse::Parse;
    use syan::source::string::Span;
    use syan::span::WithSpan;
    use syan::symbol::Token;
    use type_macro_derive_tricks::macro_derive;

    /// `atom (op atom)*`. A formatter keeps what was written, so it needs no
    /// precedence of its own.
    #[macro_derive(Parse, Debug)]
    pub struct Expr<S> {
        pub head: Atom<S>,
        pub tail: Vec<Rest<S>>,
    }

    #[macro_derive(Parse, Debug)]
    pub struct Rest<S> {
        pub op: Op<S>,
        pub rhs: Atom<S>,
    }

    #[macro_derive(Parse, Debug)]
    pub enum Op<S> {
        Add(Token![S => +]),
        Sub(Token![S => -]),
        Mul(Token![S => *]),
        Div(Token![S => /]),
    }

    #[macro_derive(Parse, Debug)]
    pub enum Atom<S> {
        Paren {
            paren: GroupParen<(), S>,
            #[group(self.paren)]
            inner: Box<Expr<S>>,
        },
        // `WithSpan` is what gives a literal a position: `Integer` itself
        // keeps only its digits.
        Lit(WithSpan<Integer, Span>),
    }
}

use ast::{Atom, Expr, Op};

/// `(start, end, is_line)` for every comment, in source order.
fn comments(src: &str) -> Vec<(usize, usize, bool)> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < b.len() {
        if b[i] == b'/' && b[i + 1] == b'/' {
            let end = src[i..].find('\n').map(|n| i + n).unwrap_or(b.len());
            out.push((i, end, true));
            i = end;
        } else if b[i] == b'/' && b[i + 1] == b'*' {
            let end = src[i + 2..]
                .find("*/")
                .map(|n| i + 2 + n + 2)
                .unwrap_or(b.len());
            out.push((i, end, false));
            i = end;
        } else {
            i += 1;
        }
    }
    out
}

/// The same text with comments blanked out, so spans still line up.
fn blank(src: &str, cs: &[(usize, usize, bool)]) -> String {
    let mut s: Vec<char> = src.chars().collect();
    // Byte offsets index a `&str`; this example's inputs are ASCII, so the
    // two coincide. A real formatter would carry char indices throughout.
    for &(start, end, _) in cs {
        for c in s.iter_mut().take(end).skip(start) {
            if *c != '\n' {
                *c = ' ';
            }
        }
    }
    s.into_iter().collect()
}

struct Out<'a> {
    src: &'a str,
    cs: Vec<(usize, usize, bool)>,
    next: usize,
    buf: String,
}

impl Out<'_> {
    /// Emit every comment that stood before `loc`.
    fn flush(&mut self, loc: usize) {
        while let Some(&(start, end, line)) = self.cs.get(self.next) {
            if start >= loc {
                break;
            }
            self.next += 1;
            self.pad();
            self.buf.push_str(self.src[start..end].trim_end());
            // A line comment runs to the end of its line, so what follows it
            // has to start on the next one.
            if line {
                self.buf.push('\n');
            }
        }
    }

    /// One space between things — but not at the start, not after a line
    /// comment's newline, and not just inside a `(`.
    fn pad(&mut self) {
        let skip = self.buf.is_empty() || self.buf.ends_with('\n') || self.buf.ends_with('(');
        if !skip {
            self.buf.push(' ');
        }
    }

    fn token(&mut self, loc: usize, text: &str) {
        self.flush(loc);
        self.pad();
        self.buf.push_str(text);
    }
}

fn expr(e: &Expr<Span>, o: &mut Out) {
    atom(&e.head, o);
    for r in &e.tail {
        let (loc, text) = match &r.op {
            Op::Add(t) => (t.span.loc, "+"),
            Op::Sub(t) => (t.span.loc, "-"),
            Op::Mul(t) => (t.span.loc, "*"),
            Op::Div(t) => (t.span.loc, "/"),
        };
        o.token(loc, text);
        atom(&r.rhs, o);
    }
}

fn atom(a: &Atom<Span>, o: &mut Out) {
    match a {
        Atom::Lit(i) => o.token(i.span.loc, &i.slot.value),
        Atom::Paren { paren, inner } => {
            o.token(paren.open.span.loc, "(");
            expr(inner, o);
            o.flush(paren.close.span.loc);
            o.buf.push(')');
        }
    }
}

fn format(src: &str) -> Result<String, String> {
    let cs = comments(src);
    let e: Expr<Span> = Parse::parse(blank(src, &cs).as_str()).map_err(|e| format!("{e:?}"))?;
    let mut o = Out {
        src,
        cs,
        next: 0,
        buf: String::new(),
    };
    expr(&e, &mut o);
    // Anything after the last token.
    o.flush(src.len());
    Ok(o.buf.trim_end().to_string())
}

fn main() {
    let cases = [
        "1+2 *( 3-4 )/  5",
        "1 + /* c */ 2",
        "1 /*a*/ +/*b*/ 2",
        "( 1 /* inner */ + 2 ) * 3",
        "1 + 2 // trailing",
        "1 + // why\n2",
    ];
    for src in cases {
        let pretty = format(src).expect("parse");
        assert_eq!(pretty, format(&pretty).expect("reparse"), "not idempotent");
        println!("{:22} -> {}", format!("{src:?}"), pretty.replace('\n', "\\n"));
    }
    // The property that matters: no comment is dropped.
    for src in cases {
        let n = comments(src).len();
        assert_eq!(comments(&format(src).unwrap()).len(), n, "lost a comment");
    }
    println!("idempotent, and every comment survives");
}
