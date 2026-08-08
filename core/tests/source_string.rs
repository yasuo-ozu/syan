use syan::parse::{IntoParseStream, Parse, ParseStream};
use syan::source::string::{Span, Stream};
use syan::span::{Span as SpanTrait, WithSpan};
use syan::symbol::{chars, Symbol};

#[test]
fn test_span_default() {
    let span: Span = Default::default();
    assert_eq!(span.line, 0);
    assert_eq!(span.col, 0);
    assert_eq!(span.loc, 0);
}

#[test]
fn test_span_migrate() {
    let span1 = Span {
        line: 1,
        col: 5,
        loc: 5,
    };
    let span2 = Span {
        line: 2,
        col: 3,
        loc: 10,
    };
    
    // migrate should prefer the span with higher loc
    let result = span1.clone().migrate(span2.clone());
    assert_eq!(result.line, 2);
    assert_eq!(result.col, 3);
    assert_eq!(result.loc, 10);
    
    let result = span2.migrate(span1);
    assert_eq!(result.line, 2);
    assert_eq!(result.col, 3);
    assert_eq!(result.loc, 10);
}

#[test]
fn test_span_migrate_equal_loc() {
    let span1 = Span {
        line: 1,
        col: 5,
        loc: 10,
    };
    let span2 = Span {
        line: 2,
        col: 3,
        loc: 10,
    };
    
    // When loc is equal, should prefer the first span
    let result = span1.clone().migrate(span2);
    assert_eq!(result.line, 1);
    assert_eq!(result.col, 5);
    assert_eq!(result.loc, 10);
}

#[test]
fn test_stream_empty_string() {
    let mut stream = Stream::new("".to_string());
    assert!(stream.next().is_none());
    assert!(stream.peek().is_none());
}

#[test]
fn test_stream_multiple_chars() {
    let mut stream = Stream::new("abc".to_string());
    
    let atom1 = stream.next().unwrap();
    assert_eq!(atom1.slot, 'a');
    assert_eq!(atom1.span.line, 1);
    assert_eq!(atom1.span.col, 1);
    assert_eq!(atom1.span.loc, 0);
    
    let atom2 = stream.next().unwrap();
    assert_eq!(atom2.slot, 'b');
    assert_eq!(atom2.span.line, 1);
    assert_eq!(atom2.span.col, 2);
    assert_eq!(atom2.span.loc, 1);
    
    let atom3 = stream.next().unwrap();
    assert_eq!(atom3.slot, 'c');
    assert_eq!(atom3.span.line, 1);
    assert_eq!(atom3.span.col, 3);
    assert_eq!(atom3.span.loc, 2);
    
    assert!(stream.next().is_none());
}

#[test]
fn test_stream_peek() {
    let mut stream = Stream::new("abc".to_string());
    
    // Peek should show first char without consuming
    let peeked = stream.peek().unwrap();
    assert_eq!(peeked.slot, 'a');
    assert_eq!(peeked.span.line, 1);
    assert_eq!(peeked.span.col, 1);
    assert_eq!(peeked.span.loc, 0);
    
    // Peek again should show same char
    let peeked2 = stream.peek().unwrap();
    assert_eq!(peeked2.slot, 'a');
    
    // Next should return the peeked char
    let atom = stream.next().unwrap();
    assert_eq!(atom.slot, 'a');
    
    // Now peek should show next char
    let peeked3 = stream.peek().unwrap();
    assert_eq!(peeked3.slot, 'b');
}

#[test]
fn test_stream_push() {
    let mut stream = Stream::new("bc".to_string());
    
    let pushed_atom = WithSpan {
        slot: 'a',
        span: Span {
            line: 1,
            col: 0,
            loc: 0,
        },
    };
    
    stream.push(pushed_atom);
    
    // Should get the pushed atom first
    let atom = stream.next().unwrap();
    assert_eq!(atom.slot, 'a');
    assert_eq!(atom.span.line, 1);
    assert_eq!(atom.span.col, 0);
    assert_eq!(atom.span.loc, 0);
    
    // Then the original string chars
    let atom2 = stream.next().unwrap();
    assert_eq!(atom2.slot, 'b');
    
    let atom3 = stream.next().unwrap();
    assert_eq!(atom3.slot, 'c');
}

#[test]
fn test_stream_get_error() {
    let mut stream = Stream::new("test".to_string());
    assert!(stream.get_error().is_ok());
}

#[test]
fn test_stream_skip_sep() {
    let mut stream = Stream::new("test".to_string());
    assert!(stream.skip_sep());
}

#[test]
fn test_parse_char_classes() {
    // One ok + one err assert per token class (all share one `impl_parse_for_char!`-generated
    // body), plus two empty-string errors.
    assert!(Symbol::<chars::_a>::parse("a".to_string()).is_ok());
    assert!(Symbol::<chars::_a>::parse("b".to_string()).is_err());

    assert!(Symbol::<chars::_A>::parse("A".to_string()).is_ok());
    assert!(Symbol::<chars::_A>::parse("B".to_string()).is_err());

    assert!(Symbol::<chars::_0>::parse("0".to_string()).is_ok());
    assert!(Symbol::<chars::_0>::parse("1".to_string()).is_err());

    assert!(Symbol::<chars::Plus>::parse("+".to_string()).is_ok());
    assert!(Symbol::<chars::Plus>::parse("-".to_string()).is_err());

    assert!(Symbol::<chars::OpenParen>::parse("(".to_string()).is_ok());
    assert!(Symbol::<chars::OpenParen>::parse(")".to_string()).is_err());

    assert!(Symbol::<chars::__>::parse("_".to_string()).is_ok());
    assert!(Symbol::<chars::__>::parse("a".to_string()).is_err());

    assert!(Symbol::<chars::_a>::parse("".to_string()).is_err());
    assert!(Symbol::<chars::Plus>::parse("".to_string()).is_err());
}

#[test]
fn test_parse_with_newlines_and_spaces() {
    // Test parsing chars from multi-line strings
    let mut stream = Stream::new("a\n b\tc".to_string());
    
    // Parse 'a'
    let result = Symbol::<chars::_a>::parse(&mut stream);
    assert!(result.is_ok());
    
    // Parse newline
    let atom = stream.next().unwrap();
    assert_eq!(atom.slot, '\n');
    
    // Parse space
    let result = Symbol::<chars::Space>::parse(&mut stream);
    assert!(result.is_ok());
    
    // Parse 'b'
    let result = Symbol::<chars::_b>::parse(&mut stream);
    assert!(result.is_ok());
}

#[test]
fn test_complex_parsing_sequence() {
    let input = "hello world!".to_string();
    let mut stream = input.into_parse_stream();
    
    // Parse each character in sequence
    assert!(Symbol::<chars::_h>::parse(&mut stream).is_ok());
    assert!(Symbol::<chars::_e>::parse(&mut stream).is_ok());
    assert!(Symbol::<chars::_l>::parse(&mut stream).is_ok());
    assert!(Symbol::<chars::_l>::parse(&mut stream).is_ok());
    assert!(Symbol::<chars::_o>::parse(&mut stream).is_ok());
    assert!(Symbol::<chars::Space>::parse(&mut stream).is_ok());
    assert!(Symbol::<chars::_w>::parse(&mut stream).is_ok());
    assert!(Symbol::<chars::_o>::parse(&mut stream).is_ok());
    assert!(Symbol::<chars::_r>::parse(&mut stream).is_ok());
    assert!(Symbol::<chars::_l>::parse(&mut stream).is_ok());
    assert!(Symbol::<chars::_d>::parse(&mut stream).is_ok());
    assert!(Symbol::<chars::Not>::parse(&mut stream).is_ok()); // '!'
    
    // Should be end of stream
    assert!(stream.next().is_none());
}

#[test]
fn test_parse_pushback_functionality() {
    let input = "abc".to_string();
    let mut stream = input.into_parse_stream();
    
    // Try to parse 'b' (should fail)
    let result = Symbol::<chars::_b>::parse(&mut stream);
    assert!(result.is_err());
    
    // The 'a' should have been pushed back, so we can parse it now
    let result = Symbol::<chars::_a>::parse(&mut stream);
    assert!(result.is_ok());
    
    // Now we can parse 'b'
    let result = Symbol::<chars::_b>::parse(&mut stream);
    assert!(result.is_ok());
    
    // And finally 'c'
    let result = Symbol::<chars::_c>::parse(&mut stream);
    assert!(result.is_ok());
}

#[test]
fn test_multiline_position_tracking() {
    let input = "line1\nline2\n\nline4".to_string();
    let mut stream = input.into_parse_stream();
    
    // First line
    let atom = stream.next().unwrap(); // 'l'
    assert_eq!(atom.span.line, 1);
    assert_eq!(atom.span.col, 1);
    assert_eq!(atom.span.loc, 0);
    
    stream.next(); // 'i'
    stream.next(); // 'n'
    stream.next(); // 'e'
    
    let atom = stream.next().unwrap(); // '1'
    assert_eq!(atom.span.line, 1);
    assert_eq!(atom.span.col, 5);
    assert_eq!(atom.span.loc, 4);
    
    let atom = stream.next().unwrap(); // '\n'
    assert_eq!(atom.span.line, 1);
    assert_eq!(atom.span.col, 6);
    assert_eq!(atom.span.loc, 5);
    
    // Second line
    let atom = stream.next().unwrap(); // 'l'
    assert_eq!(atom.span.line, 2);
    assert_eq!(atom.span.col, 1);
    assert_eq!(atom.span.loc, 6);
    
    stream.next(); // 'i'
    stream.next(); // 'n'
    stream.next(); // 'e'
    stream.next(); // '2'
    stream.next(); // '\n'
    
    // Empty line (just newline)
    let atom = stream.next().unwrap(); // '\n'
    assert_eq!(atom.span.line, 3);
    assert_eq!(atom.span.col, 1);
    assert_eq!(atom.span.loc, 12);
    
    // Fourth line
    let atom = stream.next().unwrap(); // 'l'
    assert_eq!(atom.span.line, 4);
    assert_eq!(atom.span.col, 1);
    assert_eq!(atom.span.loc, 13);
}