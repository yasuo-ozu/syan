// Parses `proc_macro2` tokens throughout; the whole suite is skipped without the optional
// dependency that provides them.
#![cfg(feature = "proc_macro2")]

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
    pub outer_content: Punctuated<syan::literal::Integer, Token![S => ,]>,
    #[group(self.bracket_group)]
    pub comma_token: Token![S => ,],

    // Level 2: Parentheses group (...) inside bracket group
    #[group(self.bracket_group)]
    pub paren_group: syan::nested::group::GroupParen<(), S>,
    #[group(self.paren_group)]
    pub middle_content: Punctuated<syan::literal::Integer, Token![S => ;]>,
    #[group(self.paren_group)]
    pub comma_token_2: Token![S => ,],

    // Level 3: Brace group {...} inside parentheses group
    #[group(self.paren_group)]
    pub brace_group: syan::nested::group::GroupBrace<(), S>,
    #[group(self.brace_group)]
    pub inner_content: Punctuated<syan::literal::Integer, Token![S => ,]>,
    #[group(self.brace_group)]
    pub final_element: Option<Token![S => !]>,
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

// Additional test structure for error cases and edge cases
#[macro_derive(Parse)]
struct SimpleContainer<S> {
    pub bracket_group: syan::nested::group::GroupBracket<(), S>,
    #[group(self.bracket_group)]
    pub content: Punctuated<syan::literal::Integer, Token![S => ,]>,
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
