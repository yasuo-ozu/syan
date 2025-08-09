use syan::nested::punctuated::Punctuated;
use syan::span::{Span, Spanned};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TestSpan;

impl Span for TestSpan {
    fn migrate(self, _other: Self) -> Self {
        TestSpan
    }
}

impl Spanned for TestSpan {
    type Span = Self;
    
    fn span(&self) -> Self::Span {
        TestSpan
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Comma;

#[test]
fn test_len() {
    let mut punct: Punctuated<i32, Comma> = Punctuated::default();
    assert_eq!(punct.len(), 0);
    
    punct.push(1);
    assert_eq!(punct.len(), 1);
    
    punct.push(2);
    assert_eq!(punct.len(), 2);
    
    punct.push(3);
    assert_eq!(punct.len(), 3);
}

#[test]
fn test_push() {
    let mut punct: Punctuated<i32, Comma> = Punctuated::default();
    
    punct.push(1);
    assert_eq!(punct.len(), 1);
    assert_eq!(punct.first(), Some(&1));
    assert_eq!(punct.last(), Some(&1));
    
    punct.push(2);
    assert_eq!(punct.len(), 2);
    assert_eq!(punct.first(), Some(&1));
    assert_eq!(punct.last(), Some(&2));
    
    punct.push(3);
    assert_eq!(punct.len(), 3);
    assert_eq!(punct.first(), Some(&1));
    assert_eq!(punct.last(), Some(&3));
}

#[test]
fn test_insert() {
    let mut punct: Punctuated<i32, Comma> = Punctuated::default();
    
    // Insert into empty list
    punct.insert(0, 2);
    assert_eq!(punct.len(), 1);
    assert_eq!(punct.first(), Some(&2));
    
    // Insert at beginning
    punct.insert(0, 1);
    assert_eq!(punct.len(), 2);
    assert_eq!(punct.first(), Some(&1));
    assert_eq!(punct.last(), Some(&2));
    
    // Insert at end
    punct.insert(2, 4);
    assert_eq!(punct.len(), 3);
    assert_eq!(punct.last(), Some(&4));
    
    // Insert in middle
    punct.insert(2, 3);
    assert_eq!(punct.len(), 4);
    
    let values: Vec<i32> = punct.iter().cloned().collect();
    assert_eq!(values, vec![1, 2, 3, 4]);
}

#[test]
#[should_panic(expected = "Index out of bounds")]
fn test_insert_out_of_bounds_empty() {
    let mut punct: Punctuated<i32, Comma> = Punctuated::default();
    punct.insert(1, 1);
}

#[test]
#[should_panic(expected = "Index out of bounds")]
fn test_insert_out_of_bounds_non_empty() {
    let mut punct: Punctuated<i32, Comma> = Punctuated::default();
    punct.push(1);
    punct.push(2);
    punct.insert(10, 3);
}

#[test]
fn test_first_and_last() {
    let mut punct: Punctuated<i32, Comma> = Punctuated::default();
    
    // Empty list
    assert_eq!(punct.first(), None);
    assert_eq!(punct.last(), None);
    
    // Single element
    punct.push(1);
    assert_eq!(punct.first(), Some(&1));
    assert_eq!(punct.last(), Some(&1));
    
    // Multiple elements
    punct.push(2);
    punct.push(3);
    assert_eq!(punct.first(), Some(&1));
    assert_eq!(punct.last(), Some(&3));
}

#[test]
fn test_first_mut_and_last_mut() {
    let mut punct: Punctuated<i32, Comma> = Punctuated::default();
    
    // Empty list
    assert_eq!(punct.first_mut(), None);
    assert_eq!(punct.last_mut(), None);
    
    // Single element
    punct.push(1);
    assert_eq!(punct.first_mut(), Some(&mut 1));
    assert_eq!(punct.last_mut(), Some(&mut 1));
    
    // Modify through first_mut
    if let Some(first) = punct.first_mut() {
        *first = 10;
    }
    assert_eq!(punct.first(), Some(&10));
    
    // Multiple elements
    punct.push(2);
    punct.push(3);
    
    // Modify through first_mut
    if let Some(first) = punct.first_mut() {
        *first = 100;
    }
    assert_eq!(punct.first(), Some(&100));
    
    // Modify through last_mut
    if let Some(last) = punct.last_mut() {
        *last = 300;
    }
    assert_eq!(punct.last(), Some(&300));
    
    let values: Vec<i32> = punct.iter().cloned().collect();
    assert_eq!(values, vec![100, 2, 300]);
}

#[test]
fn test_remove() {
    let mut punct: Punctuated<i32, Comma> = Punctuated::default();
    
    // Remove from empty list
    assert_eq!(punct.remove(0), None);
    
    // Single element
    punct.push(1);
    assert_eq!(punct.remove(0), Some(1));
    assert_eq!(punct.len(), 0);
    assert_eq!(punct.first(), None);
    
    // Multiple elements - remove first
    punct.push(1);
    punct.push(2);
    punct.push(3);
    assert_eq!(punct.remove(0), Some(1));
    assert_eq!(punct.len(), 2);
    assert_eq!(punct.first(), Some(&2));
    assert_eq!(punct.last(), Some(&3));
    
    // Remove middle
    assert_eq!(punct.remove(1), Some(3));
    assert_eq!(punct.len(), 1);
    assert_eq!(punct.first(), Some(&2));
    assert_eq!(punct.last(), Some(&2));
    
    // Remove last remaining
    assert_eq!(punct.remove(0), Some(2));
    assert_eq!(punct.len(), 0);
    
    // Remove out of bounds
    assert_eq!(punct.remove(0), None);
    assert_eq!(punct.remove(10), None);
}

#[test]
fn test_remove_multiple_elements() {
    let mut punct: Punctuated<i32, Comma> = Punctuated::default();
    
    // Setup: [1, 2, 3, 4, 5]
    for i in 1..=5 {
        punct.push(i);
    }
    
    // Remove from middle
    assert_eq!(punct.remove(2), Some(3));
    let values: Vec<i32> = punct.iter().cloned().collect();
    assert_eq!(values, vec![1, 2, 4, 5]);
    
    // Remove from end
    assert_eq!(punct.remove(3), Some(5));
    let values: Vec<i32> = punct.iter().cloned().collect();
    assert_eq!(values, vec![1, 2, 4]);
    
    // Remove from beginning
    assert_eq!(punct.remove(0), Some(1));
    let values: Vec<i32> = punct.iter().cloned().collect();
    assert_eq!(values, vec![2, 4]);
}

#[test]
fn test_iterator_after_operations() {
    let mut punct: Punctuated<String, Comma> = Punctuated::default();
    
    punct.push("a".to_string());
    punct.push("b".to_string());
    punct.push("c".to_string());
    
    let values: Vec<String> = punct.iter().cloned().collect();
    assert_eq!(values, vec!["a", "b", "c"]);
    
    punct.insert(1, "x".to_string());
    let values: Vec<String> = punct.iter().cloned().collect();
    assert_eq!(values, vec!["a", "x", "b", "c"]);
    
    punct.remove(2);
    let values: Vec<String> = punct.iter().cloned().collect();
    assert_eq!(values, vec!["a", "x", "c"]);
}

#[test]
fn test_default_construction() {
    let punct: Punctuated<i32, Comma> = Punctuated::default();
    assert_eq!(punct.len(), 0);
    assert_eq!(punct.first(), None);
    assert_eq!(punct.last(), None);
}