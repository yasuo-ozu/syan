//! Growth source (2), INDEPENDENT of `Dup` — note there is no `dup` here.
//!
//! `Parse::parse` takes `impl IntoParseStream`: a generic parameter, which
//! MOVES rather than reborrows, so every descent adds a `&mut` layer. Fixing
//! the `Dup` wrapper alone leaves this untouched.
//!
//! Fails with a MONOMORPHIZATION limit, not a trait-solving overflow:
//!   error: reached the recursion limit while instantiating
//!     `descend::<&mut &mut &mut &mut &mut ...>`
//!
//! The cure is to split `Parse` into an entry `parse(impl IntoParseStream)`
//! and a recursion point `parse_stream<S: ParseStream>(&mut S)`, whose `&mut S`
//! parameter REBORROWS at every call site.
use syan::parse::{IntoParseStream, ParseStream};
use syan::source::string::{Span, Stream};
use syan::span::WithSpan;
type Atom = WithSpan<char, Span>;

fn descend(s: impl IntoParseStream<Atom = Atom>, depth: u32) {
    let mut s = s.into_parse_stream();
    let _ = s.peek();
    if depth > 0 {
        descend(&mut s, depth - 1);
    }
}

fn main() {
    descend(Stream::new("x".to_string()), 40);
}
