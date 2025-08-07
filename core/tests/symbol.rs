use syan::symbol::Symbol;

#[test]
fn test_symbol_basic() {
    assert_eq!(&<Symbol![hello]>::default().to_string(), "hello");
    assert_eq!(&<Symbol![world]>::default().to_string(), "world");
    assert_eq!(&<Symbol![test]>::default().to_string(), "test");
    assert_eq!(&<Symbol![foo]>::default().to_string(), "foo");
    assert_eq!(&<Symbol![bar]>::default().to_string(), "bar");
}

#[test]
fn test_symbol_with_numbers() {
    assert_eq!(&<Symbol![test_123]>::default().to_string(), "test_123");
    assert_eq!(&<Symbol![hello_42]>::default().to_string(), "hello_42");
    assert_eq!(&<Symbol![value_456]>::default().to_string(), "value_456");
    assert_eq!(&<Symbol![item_99]>::default().to_string(), "item_99");
}

#[test]
fn test_symbol_underscores() {
    assert_eq!(
        &<Symbol![hello_world]>::default().to_string(),
        "hello_world"
    );
    assert_eq!(&<Symbol![test_var]>::default().to_string(), "test_var");
    assert_eq!(&<Symbol![data_set]>::default().to_string(), "data_set");
    assert_eq!(&<Symbol![func_name]>::default().to_string(), "func_name");
}

#[test]
fn test_symbol_single_char() {
    assert_eq!(&<Symbol![x]>::default().to_string(), "x");
    assert_eq!(&<Symbol![y]>::default().to_string(), "y");
    assert_eq!(&<Symbol![z]>::default().to_string(), "z");
    assert_eq!(&<Symbol![a]>::default().to_string(), "a");
    assert_eq!(&<Symbol![b]>::default().to_string(), "b");
}

#[test]
fn test_symbol_longer() {
    assert_eq!(&<Symbol![identifier]>::default().to_string(), "identifier");
    assert_eq!(
        &<Symbol![struct_field]>::default().to_string(),
        "struct_field"
    );
}

#[test]
fn test_symbol_rust_keywords() {
    assert_eq!(&<Symbol![Clone]>::default().to_string(), "Clone");
    assert_eq!(&<Symbol![Debug]>::default().to_string(), "Debug");
    assert_eq!(&<Symbol![Send]>::default().to_string(), "Send");
    assert_eq!(&<Symbol![Sync]>::default().to_string(), "Sync");
    assert_eq!(&<Symbol![Vec]>::default().to_string(), "Vec");
    assert_eq!(&<Symbol![Option]>::default().to_string(), "Option");
    assert_eq!(&<Symbol![Result]>::default().to_string(), "Result");
}

#[test]
fn test_symbol_mixed_case() {
    assert_eq!(&<Symbol![MyStruct]>::default().to_string(), "MyStruct");
    assert_eq!(&<Symbol![SomeType]>::default().to_string(), "SomeType");
    assert_eq!(&<Symbol![TestCase]>::default().to_string(), "TestCase");
    assert_eq!(&<Symbol![DataType]>::default().to_string(), "DataType");
}

#[test]
fn test_symbol_literal_integers() {
    assert_eq!(&<Symbol![42]>::default().to_string(), "42");
    assert_eq!(&<Symbol![123]>::default().to_string(), "123");
    assert_eq!(&<Symbol![0]>::default().to_string(), "0");
    assert_eq!(&<Symbol![999]>::default().to_string(), "999");
    assert_eq!(&<Symbol![1 2 3]>::default().to_string(), "123");
    assert_eq!(&<Symbol![100 200]>::default().to_string(), "100200");
}

#[test]
fn test_symbol_literal_characters() {
    assert_eq!(&<Symbol!['a']>::default().to_string(), "a");
    assert_eq!(&<Symbol!['x']>::default().to_string(), "x");
    assert_eq!(&<Symbol!['Z']>::default().to_string(), "Z");
    assert_eq!(&<Symbol!['1']>::default().to_string(), "1");
    assert_eq!(&<Symbol!['a' 'b']>::default().to_string(), "ab");
    assert_eq!(&<Symbol!['x' 'y' 'z']>::default().to_string(), "xyz");
}

#[test]
fn test_symbol_punctuation() {
    assert_eq!(&<Symbol![+]>::default().to_string(), "+");
    assert_eq!(&<Symbol![-]>::default().to_string(), "-");
    assert_eq!(&<Symbol![*]>::default().to_string(), "*");
    assert_eq!(&<Symbol![/]>::default().to_string(), "/");
    assert_eq!(&<Symbol![::]>::default().to_string(), "::");
    assert_eq!(&<Symbol![->]>::default().to_string(), "->");
    assert_eq!(&<Symbol![< >]>::default().to_string(), "<>");
    assert_eq!(&<Symbol![+ -]>::default().to_string(), "+-");
}

#[test]
fn test_symbol_mixed_tokens() {
    assert_eq!(&<Symbol![hello 42]>::default().to_string(), "hello42");
    assert_eq!(&<Symbol![test 'a']>::default().to_string(), "testa");
    assert_eq!(&<Symbol![func+]>::default().to_string(), "func+");
    assert_eq!(&<Symbol![x 1 'a']>::default().to_string(), "x1a");
    assert_eq!(&<Symbol![data::path]>::default().to_string(), "data::path");
    assert_eq!(
        &<Symbol![value + 42 'x']>::default().to_string(),
        "value+42x"
    );
    assert_eq!(
        &<Symbol![hello 'w' 123 ->]>::default().to_string(),
        "hellow123->"
    );
}

#[test]
fn test_symbol_complex_mixed() {
    assert_eq!(&<Symbol![a 1 b 2 c 3]>::default().to_string(), "a1b2c3");
    assert_eq!(&<Symbol!['x' + 'y' - 'z']>::default().to_string(), "x+y-z");
    assert_eq!(
        &<Symbol![test :: 42 'a' +]>::default().to_string(),
        "test::42a+"
    );
    assert_eq!(&<Symbol![<T>::new]>::default().to_string(), "<T>::new");
    assert_eq!(&<Symbol![Vec<i32>]>::default().to_string(), "Vec<i32>");
    assert_eq!(&<Symbol![0 'A' 1 'B' 2]>::default().to_string(), "0A1B2");
}

#[test]
fn test_symbol_alternating_patterns() {
    assert_eq!(&<Symbol![+ - + -]>::default().to_string(), "+-+-");
    assert_eq!(&<Symbol![1 'a' 2 'b']>::default().to_string(), "1a2b");
    assert_eq!(&<Symbol!['x' 42 'y' 99]>::default().to_string(), "x42y99");
    assert_eq!(
        &<Symbol![hello + world -]>::default().to_string(),
        "hello+world-"
    );
    assert_eq!(
        &<Symbol![< 1 > 'a' < 2 >]>::default().to_string(),
        "<1>a<2>"
    );
}

#[test]
fn test_symbol_long_identifiers() {
    // These are exactly 15+ characters, should trigger Joint mechanism
    assert_eq!(&<Symbol![very_long_identifier]>::default().to_string(), "very_long_identifier");
    assert_eq!(&<Symbol![extremely_long_name]>::default().to_string(), "extremely_long_name");
    assert_eq!(
        &<Symbol![this_is_a_very_long_symbol_name]>::default().to_string(),
        "this_is_a_very_long_symbol_name"
    );
    assert_eq!(
        &<Symbol![function_with_very_descriptive_name]>::default().to_string(),
        "function_with_very_descriptive_name"
    );
}

#[test]
fn test_symbol_long_mixed_tokens() {
    // Mixed tokens that together exceed 14 characters
    assert_eq!(
        &<Symbol![very_long_function_name_42]>::default().to_string(),
        "very_long_function_name_42"
    );
    assert_eq!(
        &<Symbol![hello_world_test_123_456]>::default().to_string(),
        "hello_world_test_123_456"
    );
    assert_eq!(
        &<Symbol![data_structure_field_ 'a' 99]>::default().to_string(),
        "data_structure_field_a99"
    );
    assert_eq!(
        &<Symbol![complex::path::to::something]>::default().to_string(),
        "complex::path::to::something"
    );
}

#[test]
fn test_symbol_long_token_sequences() {
    // Long sequences of various token types
    assert_eq!(
        &<Symbol![a b c d e f g h i j k l m n o p q r s t]>::default().to_string(),
        "abcdefghijklmnopqrst"
    );
    assert_eq!(
        &<Symbol![1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8]>::default().to_string(),
        "123456789012345678"
    );
    assert_eq!(
        &<Symbol!['a' 'b' 'c' 'd' 'e' 'f' 'g' 'h' 'i' 'j' 'k' 'l' 'm' 'n' 'o' 'p']>::default()
            .to_string(),
        "abcdefghijklmnop"
    );
    assert_eq!(
        &<Symbol![+ - * / :: -> < > + - * / :: ->]>::default().to_string(),
        "+-*/::-><>+-*/::->"
    );
}

#[test]
fn test_symbol_mixed_long_patterns() {
    // Complex patterns that exceed 14 characters total
    assert_eq!(
        &<Symbol![HashMap < String Vec < i32 > >]>::default().to_string(),
        "HashMap<StringVec<i32>>"
    );
    assert_eq!(
        &<Symbol![Result < Option < T > Error >]>::default().to_string(),
        "Result<Option<T>Error>"
    );
    assert_eq!(
        &<Symbol![a 1 b 2 c 3 d 4 e 5 f 6 g 7 h 8]>::default().to_string(),
        "a1b2c3d4e5f6g7h8"
    );
    assert_eq!(
        &<Symbol!['x' 1 'y' 2 'z' 3 'a' 4 'b' 5 'c' 6 'd' 7]>::default().to_string(),
        "x1y2z3a4b5c6d7"
    );
}

#[test]
fn test_symbol_very_long_sequences() {
    // Very long sequences to test deeply nested Joint structures
    assert_eq!(&<Symbol![this is a very long sequence of many different tokens that should definitely exceed the fourteen character limit and trigger recursive Joint structures]>::default().to_string(), "thisisaverylongsequenceofmanydifferenttokensthatshoulddefinitelyexceedthefourteencharacterlimitandtriggerrecursiveJointstructures");
    assert_eq!(&<Symbol![1 2 3 4 5 6 7 8 9 0 'a' 'b' 'c' 'd' 'e' 'f' 'g' 'h' 'i' 'j' + - * / :: -> < > = != <= >= && ||]>::default().to_string(), "1234567890abcdefghij+-*/::-><>=!=<=>=&&||");
}
