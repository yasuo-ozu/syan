//! Growth source (1): `dup` wraps `Dup<&mut Self>`, and `Dup` is itself a
//! `ParseStream`, so nesting dup-in-dup grows the stream TYPE without bound.
//! No `Parse::parse` here — this isolates the wrapper from the `&mut` flood.
use syan::parse::ParseStream;
use syan::source::string::{Span, Stream};
use syan::span::WithSpan;
type Atom = WithSpan<char, Span>;
fn nest<S: ParseStream<Atom = Atom>>(s: &mut S, depth: u32) {
    if depth == 0 { return }
    let _: Result<(), ()> = s.dup(|d| { nest(d, depth - 1); Ok(()) });
}
fn main() { let mut s = Stream::new("x".to_string()); nest(&mut s, 40); }
