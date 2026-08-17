//! Separator handling between derived fields.
//!
//! A plain field skips whatever separator the source reports; `#[joint]` demands there was none and
//! `#[alone]` demands there was one. On text the separator is whitespace; on a `TokenStream` it is
//! the spacing the lexer recorded.

use syan::parse::Parse;
use syan::symbol::Token;

type Text = syan::source::string::Span;

#[derive(Parse)]
struct Plain<S> {
    _a: Token![S => x],
    _b: Token![S => y],
}

#[derive(Parse)]
struct Adjacent<S> {
    _a: Token![S => x],
    #[joint]
    _b: Token![S => y],
}

#[derive(Parse)]
struct Separated<S> {
    _a: Token![S => x],
    #[alone]
    _b: Token![S => y],
}

fn plain(src: &str) -> bool {
    Parse::parse(src).map(|_: Plain<Text>| ()).is_ok()
}
fn adjacent(src: &str) -> bool {
    Parse::parse(src).map(|_: Adjacent<Text>| ()).is_ok()
}
fn separated(src: &str) -> bool {
    Parse::parse(src).map(|_: Separated<Text>| ()).is_ok()
}

#[test]
fn plain_field_accepts_either_spacing() {
    assert!(plain("xy"));
    assert!(plain("x y"));
    assert!(plain("x    y"));
    assert!(plain("x\n\ty"));
}

#[test]
fn joint_field_demands_adjacency() {
    assert!(adjacent("xy"));
    assert!(!adjacent("x y"), "#[joint] must reject a separator");
    assert!(!adjacent("x\ny"), "#[joint] must reject a newline too");
}

#[test]
fn alone_field_demands_a_separator() {
    assert!(separated("x y"));
    assert!(separated("x   y"));
    assert!(!separated("xy"), "#[alone] must reject adjacency");
}

/// Leading whitespace is a separator like any other, so the first field skips it too.
#[test]
fn leading_whitespace_is_skipped() {
    assert!(plain("  x y"));
}

#[cfg(feature = "proc_macro2")]
mod over_tokens {
    use super::*;

    type Tokens = syan::source::proc_macro2::Span;

    fn parse<T: Parse<proc_macro2::TokenTree>>(src: &str) -> bool {
        let ts: proc_macro2::TokenStream = src.parse().unwrap();
        T::parse(ts).is_ok()
    }

    /// A `TokenStream` carries no whitespace atoms, so a plain field is unaffected either way.
    #[test]
    fn plain_field_over_tokens() {
        assert!(parse::<Plain<Tokens>>("x y"));
        assert!(!parse::<Plain<Tokens>>("xy"), "`xy` lexes as one ident");
    }
}

/// A group and a `Punctuated` skip separators between their own parts, as a struct does between
/// fields. `Vec<T>` is plain repetition and has no such boundary.
#[cfg(feature = "proc_macro2")]
mod combinator_boundaries {
    // `paren` is read by `#[group(self.paren)]`, which clippy cannot see.
    #![allow(dead_code, clippy::type_complexity)]

    use super::*;
    use syan::nested::group::GroupParen;
    use syan::nested::Punctuated;
    use syan::source::proc_macro2::literal::Integer;

    #[derive(Parse)]
    struct Call<S> {
        _name: Token![S => f],
        paren: GroupParen<(), S>,
        #[group(self.paren)]
        args: Punctuated<Integer, Token![S => ,]>,
    }

    fn args(src: &str) -> Option<usize> {
        Parse::parse(src).ok().map(|c: Call<Text>| c.args.len())
    }

    #[test]
    fn group_and_punctuated_skip_between_parts() {
        assert_eq!(args("f(1,2,3)"), Some(3));
        assert_eq!(args("f(1, 2, 3)"), Some(3));
        assert_eq!(args("f( 1 , 2 , 3 )"), Some(3));
        assert_eq!(args("f(\n  1,\n  2\n)"), Some(2));
    }

    #[test]
    fn empty_group_tolerates_a_separator() {
        assert_eq!(args("f()"), Some(0));
        assert_eq!(args("f( )"), Some(0));
    }

    #[test]
    fn a_trailing_separator_does_not_invent_an_item() {
        assert_eq!(args("f(1,)"), None, "trailing comma is not a valid item");
    }
}
