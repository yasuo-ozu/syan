//! `SeqView` after the `edit_each`/`for_each_mut` → `iter_mut` interface change: in-place iteration via
//! `iter_mut`, structural changes via `retain_mut`/`push`/`insert`/`remove`. Fully-qualified trait calls
//! are used because on a concrete `Vec` the trait's element type is ambiguous to infer (the `Wrap`
//! blanket gives `Vec<Box<T>>` both `SeqView<T>` and `SeqView<Box<T>>`) — and, per the last test, a bare
//! `.iter_mut()`/`.push()` resolves to the inherent std method anyway.

use syan::visit::SeqView;

#[test]
fn iter_mut_edits_every_element_in_place() {
    let mut v = vec![1, 2, 3];
    for x in <Vec<i32> as SeqView<i32>>::iter_mut(&mut v) {
        *x *= 10;
    }
    assert_eq!(v, vec![10, 20, 30]);
    assert_eq!(<Vec<i32> as SeqView<i32>>::iter_mut(&mut v).count(), 3, "each element once");
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
fn box_transparent_iter_mut() {
    // `Vec<Box<T>>: SeqView<T>` yields `&mut T` (through the box).
    let mut v: Vec<Box<i32>> = vec![Box::new(1), Box::new(2)];
    for x in <Vec<Box<i32>> as SeqView<i32>>::iter_mut(&mut v) {
        *x += 100;
    }
    assert_eq!(v.iter().map(|b| **b).collect::<Vec<_>>(), vec![101, 102]);
}

#[test]
fn seqview_in_scope_does_not_shadow_inherent_vec_methods() {
    let mut v: Vec<i32> = vec![1, 2, 3];
    v.push(4); // inherent Vec::push (SeqView::push exists but inherent wins)
    v.retain_mut(|x| *x % 2 == 0); // inherent Vec::retain_mut
    assert_eq!(v, vec![2, 4], "inherent methods applied, no silent trait fallback");
    assert_eq!(<Vec<i32> as SeqView<i32>>::len(&v), 2); // trait still reachable via UFCS
    let _view: &dyn SeqView<i32> = &v;
}
