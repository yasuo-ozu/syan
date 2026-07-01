//! `SeqView::edit_each` / `SeqCursor` index bookkeeping — including combining structural ops in one
//! visit (regression for the audit finding where `remove` + `insert_after` silently skipped an element).

use syan::visit::SeqView;

#[test]
fn combined_remove_and_insert_after_skips_nothing() {
    let mut v = vec![1, 2, 3];
    let mut visited = Vec::new();
    v.edit_each(|c| {
        visited.push(*c.get());
        if *c.get() == 1 {
            c.remove();
            c.insert_after(9);
        }
    });
    assert_eq!(v, vec![2, 9, 3], "1 removed, 9 inserted");
    assert_eq!(visited, vec![1, 2, 9, 3], "2 and 3 still visited — nothing skipped");
}

#[test]
fn insert_before_and_after_around_current() {
    let mut v = vec![10, 20];
    let mut visited = Vec::new();
    v.edit_each(|c| {
        visited.push(*c.get());
        if *c.get() == 10 {
            c.insert_before(1);
            c.insert_after(11);
        }
    });
    assert_eq!(v, vec![1, 10, 11, 20]);
    // the before/after inserts are not re-visited; 10 and 20 are.
    assert_eq!(visited, vec![10, 20]);
}

#[test]
fn multiple_insert_after_preserve_order_and_are_skipped() {
    let mut v = vec![1, 2];
    let mut visited = Vec::new();
    v.edit_each(|c| {
        visited.push(*c.get());
        if *c.get() == 1 {
            c.insert_after(8);
            c.insert_after(9);
        }
    });
    assert_eq!(v, vec![1, 8, 9, 2], "call order preserved");
    assert_eq!(visited, vec![1, 2], "inserted-after nodes not re-visited");
}

#[test]
fn plain_remove_visits_shifted_element() {
    let mut v = vec![1, 2, 3];
    let mut visited = Vec::new();
    v.edit_each(|c| {
        visited.push(*c.get());
        if *c.get() % 2 == 1 {
            c.remove();
        }
    });
    assert_eq!(v, vec![2], "odds removed");
    assert_eq!(visited, vec![1, 2, 3], "every element visited exactly once");
}
