// TokenStream and TokenTree imports removed as they're unused
use syan::nested::punctuated::Punctuated;
use syan::parse::Parse;
use syan::symbol::Token;
use template_quote::quote;
use type_macro_derive_tricks::macro_derive;

#[macro_derive(Parse)]
struct NestedContainer<S> {
    // Level 1: Bracket group [...]
    pub bracket_group: syan::nested::group::GroupBracket<(), S>,
    #[group(self.bracket_group)]
    pub outer_content: Punctuated<syan::source::proc_macro2::literal::Integer, Token![S => ,]>,
    #[group(self.bracket_group)]
    pub comma_token: Token![S => ,],

    // Level 2: Parentheses group (...) inside bracket group
    #[group(self.bracket_group)]
    pub paren_group: syan::nested::group::GroupParen<(), S>,
    #[group(self.paren_group)]
    pub middle_content: Punctuated<syan::source::proc_macro2::literal::Integer, Token![S => ;]>,
    #[group(self.paren_group)]
    pub comma_token_2: Token![S => ,],

    // Level 3: Brace group {...} inside parentheses group
    #[group(self.paren_group)]
    pub brace_group: syan::nested::group::GroupBrace<(), S>,
    #[group(self.brace_group)]
    pub inner_content: Punctuated<syan::source::proc_macro2::literal::Integer, Token![S => ,]>,
    #[group(self.brace_group)]
    pub final_element: Option<Token![S => !]>,
}

#[test]
fn test_case_1() {
    let tokens = quote! { [ 123, ( 234; 345, { 678 }) ]};
    let container: NestedContainer<_> = Parse::parse(tokens.clone()).unwrap();
    assert_eq!(container.outer_content.len(), 1);
    assert_eq!(container.middle_content.len(), 2);
    assert_eq!(container.inner_content.len(), 1);
    assert!(container.final_element.is_none());
}

#[test]
fn test_case_2_with_final_element() {
    let tokens = quote! { [ 123, ( 234; 345, { 678 ! }) ]};
    let container: NestedContainer<_> = Parse::parse(tokens.clone()).unwrap();
    assert_eq!(container.outer_content.len(), 1);
    assert_eq!(container.middle_content.len(), 2);
    assert_eq!(container.inner_content.len(), 1);
    assert!(container.final_element.is_some());
}

#[test]
fn test_case_3_multiple_outer_elements() {
    let tokens = quote! { [ 111, 222, 333, ( 444; 555; 666, { 777, 888 }) ]};
    let container: NestedContainer<_> = Parse::parse(tokens.clone()).unwrap();
    assert_eq!(container.outer_content.len(), 3);
    assert_eq!(container.middle_content.len(), 3);
    assert_eq!(container.inner_content.len(), 2);
    assert!(container.final_element.is_none());
}

#[test]
fn test_case_4_empty_groups() {
    let tokens = quote! { [ 100, ( , { }) ]};
    let container: NestedContainer<_> = Parse::parse(tokens.clone()).unwrap();
    assert_eq!(container.outer_content.len(), 1);
    assert_eq!(container.middle_content.len(), 0);
    assert_eq!(container.inner_content.len(), 0);
    assert!(container.final_element.is_none());
}

#[test]
fn test_case_5_complex_nested_structure() {
    let tokens = quote! { [ 10, 20, 30, ( 40; 50; 60; 70, { 80, 90, 100 ! }) ]};
    let container: NestedContainer<_> = Parse::parse(tokens.clone()).unwrap();
    assert_eq!(container.outer_content.len(), 3);
    assert_eq!(container.middle_content.len(), 4);
    assert_eq!(container.inner_content.len(), 3);
    assert!(container.final_element.is_some());
}

#[test]
fn test_case_6_minimal_structure() {
    let tokens = quote! { [ 42, ( , { ! }) ]};
    let container: NestedContainer<_> = Parse::parse(tokens.clone()).unwrap();
    assert_eq!(container.outer_content.len(), 1);
    assert_eq!(container.middle_content.len(), 0);
    assert_eq!(container.inner_content.len(), 0);
    assert!(container.final_element.is_some());
}

#[test]
fn test_case_7_single_elements_at_each_level() {
    let tokens = quote! { [ 1, ( 2, { 3 }) ]};
    let container: NestedContainer<_> = Parse::parse(tokens.clone()).unwrap();
    assert_eq!(container.outer_content.len(), 1);
    assert_eq!(container.middle_content.len(), 1);
    assert_eq!(container.inner_content.len(), 1);
    assert!(container.final_element.is_none());
}

#[test]
fn test_case_8_large_numbers() {
    let tokens = quote! { [ 999999, ( 888888; 777777, { 666666, 555555, 444444 ! }) ]};
    let container: NestedContainer<_> = Parse::parse(tokens.clone()).unwrap();
    assert_eq!(container.outer_content.len(), 1);
    assert_eq!(container.middle_content.len(), 2);
    assert_eq!(container.inner_content.len(), 3);
    assert!(container.final_element.is_some());
}

// Additional test structure for error cases and edge cases
#[macro_derive(Parse)]
struct SimpleContainer<S> {
    pub bracket_group: syan::nested::group::GroupBracket<(), S>,
    #[group(self.bracket_group)]
    pub content: Punctuated<syan::source::proc_macro2::literal::Integer, Token![S => ,]>,
}

#[test]
fn test_simple_container_basic() {
    let tokens = quote! { [ 1, 2, 3 ] };
    let container: SimpleContainer<_> = Parse::parse(tokens).unwrap();
    assert_eq!(container.content.len(), 3);
    // The bracket group's slot is the unit type (empty group).
    let () = container.bracket_group.slot;
    // Verify the bracket group contains the expected tokens
    assert!(format!("{}", container.bracket_group.open).contains('['));
    assert!(format!("{}", container.bracket_group.close).contains(']'));
}

#[test]
fn test_simple_container_single_element() {
    let tokens = quote! { [ 42 ] };
    let container: SimpleContainer<_> = Parse::parse(tokens).unwrap();
    assert_eq!(container.content.len(), 1);
    // The bracket group's slot is the unit type (empty group).
    let () = container.bracket_group.slot;
    // Verify the bracket group contains the expected tokens
    assert!(format!("{}", container.bracket_group.open).contains('['));
    assert!(format!("{}", container.bracket_group.close).contains(']'));
}

#[test]
fn test_simple_container_empty() {
    let tokens = quote! { [ ] };
    let container: SimpleContainer<_> = Parse::parse(tokens).unwrap();
    assert_eq!(container.content.len(), 0);
    // The bracket group's slot is the unit type (empty group).
    let () = container.bracket_group.slot;
    // Verify the bracket group contains the expected tokens
    assert!(format!("{}", container.bracket_group.open).contains('['));
    assert!(format!("{}", container.bracket_group.close).contains(']'));
}

// Test structure with only two levels of nesting
#[macro_derive(Parse)]
struct TwoLevelContainer<S> {
    pub outer_brace: syan::nested::group::GroupBrace<(), S>,
    #[group(self.outer_brace)]
    pub key: syan::source::proc_macro2::literal::Integer,
    #[group(self.outer_brace)]
    pub comma1: Token![S => ,],
    
    #[group(self.outer_brace)]
    pub inner_paren: syan::nested::group::GroupParen<(), S>,
    #[group(self.inner_paren)]
    pub values: Punctuated<syan::source::proc_macro2::literal::Integer, Token![S => ;]>,
}

#[test]
fn test_two_level_container() {
    let tokens = quote! { { 100, ( 200; 300; 400 ) } };
    let container: TwoLevelContainer<_> = Parse::parse(tokens).unwrap();
    assert_eq!(container.values.len(), 3);
}

#[test]
fn test_two_level_container_single_value() {
    let tokens = quote! { { 500, ( 600 ) } };
    let container: TwoLevelContainer<_> = Parse::parse(tokens).unwrap();
    assert_eq!(container.values.len(), 1);
}

#[test]
fn test_two_level_container_empty_inner() {
    let tokens = quote! { { 700, ( ) } };
    let container: TwoLevelContainer<_> = Parse::parse(tokens).unwrap();
    assert_eq!(container.values.len(), 0);
}

// Test data access patterns
#[test]
fn test_data_access_outer_content() {
    let tokens = quote! { [ 11, 22, 33, ( 44; 55, { 66 }) ]};
    let container: NestedContainer<_> = Parse::parse(tokens).unwrap();
    
    // Test that we can access the lengths of punctuated lists
    assert_eq!(container.outer_content.len(), 3);
    assert_eq!(container.middle_content.len(), 2);
    assert_eq!(container.inner_content.len(), 1);
    
    // Test that we can iterate over punctuated content
    let outer_count = container.outer_content.iter().count();
    let middle_count = container.middle_content.iter().count();
    let inner_count = container.inner_content.iter().count();
    
    assert_eq!(outer_count, 3);
    assert_eq!(middle_count, 2);
    assert_eq!(inner_count, 1);
}

#[test]
fn test_punctuated_lengths_comprehensive() {
    // Test various combinations of punctuated list lengths
    let test_cases = vec![
        (quote! { [ 1, ( 2, { 3 }) ]}, 1, 1, 1),
        (quote! { [ 1, 2, ( 3; 4, { 5, 6 }) ]}, 2, 2, 2),
        (quote! { [ 1, 2, 3, ( 4; 5; 6, { 7, 8, 9 }) ]}, 3, 3, 3),
        (quote! { [ 1, 2, 3, 4, ( 5; 6; 7; 8, { 9, 10, 11, 12 }) ]}, 4, 4, 4),
    ];
    
    for (tokens, expected_outer, expected_middle, expected_inner) in test_cases {
        let container: NestedContainer<_> = Parse::parse(tokens).unwrap();
        assert_eq!(container.outer_content.len(), expected_outer);
        assert_eq!(container.middle_content.len(), expected_middle);
        assert_eq!(container.inner_content.len(), expected_inner);
        assert!(container.final_element.is_none());
    }
}

#[test]
fn test_final_element_variations() {
    // Test cases with and without final element
    let with_final = quote! { [ 1, ( 2, { 3 ! }) ]};
    let container_with: NestedContainer<_> = Parse::parse(with_final).unwrap();
    assert!(container_with.final_element.is_some());
    
    let without_final = quote! { [ 1, ( 2, { 3 }) ]};
    let container_without: NestedContainer<_> = Parse::parse(without_final).unwrap();
    assert!(container_without.final_element.is_none());
}

#[test]  
fn test_stress_test_large_structure() {
    // Stress test with larger nested structure
    let tokens = quote! { 
        [ 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, ( 
            11; 12; 13; 14; 15; 16; 17; 18; 19; 20, { 
                21, 22, 23, 24, 25, 26, 27, 28, 29, 30 !
            }
        )]
    };
    let container: NestedContainer<_> = Parse::parse(tokens).unwrap();
    assert_eq!(container.outer_content.len(), 10);
    assert_eq!(container.middle_content.len(), 10); 
    assert_eq!(container.inner_content.len(), 10);
    assert!(container.final_element.is_some());
}
