// AUDIT (hygiene): the Parse derive's internal parse-stream local `__syan_stream` is built with
// `parse_quote!(__syan_stream)` (call-site span), not Span::mixed_site(), so it shares the user's
// hygiene context. A field literally named `__syan_stream` shadows it: the generated
// `let __syan_stream = <Ty>::parse(&mut __syan_stream)?` rebinds the stream to the parsed value, so
// the next field calls `parse(&mut <value>)` -> cryptic `&mut Integer: ParseStream` (E0277). Unlike
// the visitor path (which fresh-names its helpers), these derive locals make no hygiene effort.
use syan::parse::{Parse, Unparse};
use syan::literal::Integer;

#[derive(Parse, Unparse)]
pub struct Collide {
    __syan_stream: Integer,
    b: Integer,
}

fn main() {}
