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

// Documents the current contract: a `SeqCursor` read (`get`/`get_mut`/`replace`) after removing the
// current node is a logic error and aborts (rather than returning `Option`) — a deliberate ergonomic
// tradeoff, not a graceful path. (Audit finding, low severity.)
#[test]
#[should_panic(expected = "cursor in bounds")]
fn reading_cursor_after_removing_panics() {
    let mut v = vec![1];
    v.edit_each(|c| {
        c.remove(); // the only element is gone; the cursor now points past the end
        let _ = c.get(); // reading it aborts
    });
}

// The `Wrap`-based blanket makes `Vec<T>: SeqView<T>` for every `T`, so importing `SeqView` puts
// `push`/`retain_mut` in scope on concrete `Vec`s. This pins that the *inherent* std methods still win
// name resolution — no silent trait fallback and no ambiguity — while the trait remains reachable via
// UFCS / a `dyn` coercion. (Audit finding, low severity: benign name-shadow / API-surface leak.)
#[test]
fn seqview_in_scope_does_not_shadow_inherent_vec_methods() {
    use syan::visit::SeqView;
    let mut v: Vec<i32> = vec![1, 2, 3];
    v.push(4); // inherent Vec::push (SeqView::push exists but inherent wins)
    v.retain_mut(|x| *x % 2 == 0); // inherent Vec::retain_mut
    assert_eq!(v, vec![2, 4], "inherent methods applied, no silent trait fallback");
    // The trait is still implemented on the concrete Vec (the blanket) and reachable explicitly:
    assert_eq!(SeqView::len(&v), 2);
    let _view: &dyn SeqView<i32> = &v;
}
