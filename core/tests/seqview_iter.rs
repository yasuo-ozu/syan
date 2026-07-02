//! `SeqView`/`OptView` are **bare-element**: `Vec<T>: SeqView<T>`, `Option<T>: OptView<T>`. A transparent
//! single-slot wrapper (`Box<T>`/`Attempt<T>`) is `OptView<T>` (always one node), so a `Vec<Box<T>>` is
//! `SeqView<Box<T>>` (element `Box<T>`) — the visitor descends the box as a further `OptView<T>` level.
//! In-place iteration via `view_iter[_mut]`; structural changes via `retain_mut`/`push`/`insert`/`remove`.

use syan::visit::{OptView, SeqView};

#[test]
fn iter_reads_every_element() {
    let v = vec![1, 2, 3];
    assert_eq!(<Vec<i32> as SeqView<i32>>::view_iter(&v).copied().collect::<Vec<_>>(), vec![1, 2, 3]);
    assert_eq!(<Vec<i32> as SeqView<i32>>::view_iter(&v).len(), 3);
}

#[test]
fn iter_mut_edits_every_element_in_place() {
    let mut v = vec![1, 2, 3];
    for x in <Vec<i32> as SeqView<i32>>::view_iter_mut(&mut v) {
        *x *= 10;
    }
    assert_eq!(v, vec![10, 20, 30]);
    assert_eq!(<Vec<i32> as SeqView<i32>>::view_iter_mut(&mut v).count(), 3, "each element once");
}

#[test]
fn optview_iter_and_iter_mut() {
    let some: Option<i32> = Some(5);
    let none: Option<i32> = None;
    assert_eq!(<Option<i32> as OptView<i32>>::view_iter(&some).count(), 1);
    assert_eq!(<Option<i32> as OptView<i32>>::view_iter(&none).count(), 0);

    let mut o: Option<i32> = Some(1);
    for x in <Option<i32> as OptView<i32>>::view_iter_mut(&mut o) {
        *x += 100;
    }
    assert_eq!(o, Some(101));

    // single-slot wrapper: `Box<T>: OptView<T>` is a 1-element view (always present).
    let mut b: Box<i32> = Box::new(7);
    for x in <Box<i32> as OptView<i32>>::view_iter_mut(&mut b) {
        *x += 1;
    }
    assert_eq!(*b, 8);
    assert!(<Box<i32> as OptView<i32>>::is_some(&b));
}

#[test]
fn retain_mut_visits_then_drops() {
    let mut v = vec![1, 2, 3, 4];
    <Vec<i32> as SeqView<i32>>::retain_mut(&mut v, |x| {
        *x += 1; // visited in place first
        *x % 2 == 0 // keep evens
    });
    assert_eq!(v, vec![2, 4], "1->2 keep, 2->3 drop, 3->4 keep, 4->5 drop");
}

#[test]
fn structural_index_ops() {
    let mut v = vec![1, 3];
    <Vec<i32> as SeqView<i32>>::insert(&mut v, 1, 2);
    assert_eq!(v, vec![1, 2, 3]);
    assert_eq!(<Vec<i32> as SeqView<i32>>::remove(&mut v, 0), 1);
    <Vec<i32> as SeqView<i32>>::push(&mut v, 9);
    assert_eq!(v, vec![2, 3, 9]);
}

#[test]
fn vec_of_box_is_bare_element() {
    // Bare-element: `Vec<Box<T>>: SeqView<Box<T>>` yields `&mut Box<T>` (the element is the box). The
    // visitor descends the box as a further `OptView<T>` level; here we edit through the box directly.
    let mut v: Vec<Box<i32>> = vec![Box::new(1), Box::new(2)];
    for x in <Vec<Box<i32>> as SeqView<Box<i32>>>::view_iter_mut(&mut v) {
        **x += 100;
    }
    assert_eq!(v.iter().map(|b| **b).collect::<Vec<_>>(), vec![101, 102]);
}

// `push`/`retain_mut` are inherent *on `Vec`*, so they win resolution over the same-named `SeqView`
// methods when `SeqView` is imported. The iterators are `view_iter`/`view_iter_mut` precisely so they
// never shadow the slice `iter`/`iter_mut` reached via `Deref` — a bare `vec.iter()` with `SeqView` in
// scope still hits the std slice method.
#[test]
fn push_and_retain_mut_prefer_inherent_over_seqview() {
    let mut v: Vec<i32> = vec![1, 2, 3];
    v.push(4); // inherent Vec::push (SeqView::push exists but inherent wins)
    v.retain_mut(|x| *x % 2 == 0); // inherent Vec::retain_mut
    assert_eq!(v, vec![2, 4], "inherent methods applied, no silent trait fallback");
    assert_eq!(v.iter().copied().sum::<i32>(), 6, "slice `.iter()` unshadowed by SeqView");
    assert_eq!(<Vec<i32> as SeqView<i32>>::len(&v), 2); // trait still reachable via UFCS
    let _view: &dyn SeqView<i32> = &v;
}
