use syan::symbol::Symbol;

#[test]
fn test_symbol_idents() {
    assert_eq!(&<Symbol![hello]>::default().to_string(), "hello");
    assert_eq!(&<Symbol![x]>::default().to_string(), "x");
    assert_eq!(&<Symbol![test_123]>::default().to_string(), "test_123");
    assert_eq!(&<Symbol![hello_world]>::default().to_string(), "hello_world");
    assert_eq!(&<Symbol![MyStruct]>::default().to_string(), "MyStruct");
}

#[test]
fn test_symbol_literals() {
    assert_eq!(&<Symbol![42]>::default().to_string(), "42");
    assert_eq!(&<Symbol![1 2 3]>::default().to_string(), "123");
    assert_eq!(&<Symbol!['a']>::default().to_string(), "a");
    assert_eq!(&<Symbol!['a' 'b']>::default().to_string(), "ab");
}

#[test]
fn test_symbol_puncts() {
    assert_eq!(&<Symbol![+]>::default().to_string(), "+");
    assert_eq!(&<Symbol![::]>::default().to_string(), "::");
    assert_eq!(&<Symbol![->]>::default().to_string(), "->");
    assert_eq!(
        &<Symbol![test :: 42 'a' +]>::default().to_string(),
        "test::42a+"
    );
}

#[test]
fn test_symbol_very_long_sequences() {
    // Sequences past `MAX_TUPLE_SIZE` (12), which is where `Symbol!` starts chunking into nested
    // `Joint`s — a single long identifier and a long mixed ident/lit/punct run.
    assert_eq!(
        &<Symbol![very_long_function_name_42]>::default().to_string(),
        "very_long_function_name_42"
    );
    assert_eq!(&<Symbol![this is a very long sequence of many different tokens that should definitely exceed the fourteen character limit and trigger recursive Joint structures]>::default().to_string(), "thisisaverylongsequenceofmanydifferenttokensthatshoulddefinitelyexceedthefourteencharacterlimitandtriggerrecursiveJointstructures");
    assert_eq!(&<Symbol![1 2 3 4 5 6 7 8 9 0 'a' 'b' 'c' 'd' 'e' 'f' 'g' 'h' 'i' 'j' + - * / :: -> < > = != <= >= && ||]>::default().to_string(), "1234567890abcdefghij+-*/::-><>=!=<=>=&&||");
}
