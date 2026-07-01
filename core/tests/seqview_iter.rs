//! `SeqView` after the `edit_each`/`for_each_mut` → `iter_mut` interface change: in-place iteration via
//! `iter_mut`, structural changes via `retain_mut`/`push`/`insert`/`remove`. Fully-qualified trait calls
//! are used because on a concrete `Vec` the trait's element type is ambiguous to infer (the `Wrap`
//! blanket gives `Vec<Box<T>>` both `SeqView<T>` and `SeqView<Box<T>>`) — and, per the last test, a bare
//! `.iter_mut()`/`.push()` resolves to the inherent std method anyway.

use syan::visit::{OptView, SeqView};

#[test]
fn iter_reads_every_element() {
    let v = vec![1, 2, 3];
    assert_eq!(<Vec<i32> as SeqView<i32>>::iter(&v).copied().collect::<Vec<_>>(), vec![1, 2, 3]);
    assert_eq!(<Vec<i32> as SeqView<i32>>::iter(&v).len(), 3);
}

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
fn optview_iter_and_iter_mut() {
    let some: Option<i32> = Some(5);
    let none: Option<i32> = None;
    assert_eq!(<Option<i32> as OptView<i32>>::iter(&some).count(), 1);
    assert_eq!(<Option<i32> as OptView<i32>>::iter(&none).count(), 0);

    let mut o: Option<i32> = Some(1);
    for x in <Option<i32> as OptView<i32>>::iter_mut(&mut o) {
        *x += 100;
    }
    assert_eq!(o, Some(101));

    // box-transparent: `Option<Box<T>>: OptView<T>` iterates the inner `T`.
    let mut b: Option<Box<i32>> = Some(Box::new(7));
    for x in <Option<Box<i32>> as OptView<i32>>::iter_mut(&mut b) {
        *x += 1;
    }
    assert_eq!(b.map(|x| *x), Some(8));
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
    assert_eq!(v.into_iter().map(|b| *b).collect::<Vec<_>>(), vec![101, 102]);
}

// `push`/`retain_mut` are inherent *on `Vec`*, so they win resolution over the same-named `SeqView`
// methods when `SeqView` is imported. NOTE the asymmetry: `iter`/`iter_mut` are inherent on the *slice*
// (reached via `Deref`), so `SeqView::iter`/`iter_mut` — being on `Vec` directly — SHADOW them; a bare
// `vec.iter()` with `SeqView` in scope hits the trait method (and is ambiguous). Use `vec.as_slice()`
// (or don't import `SeqView`) for slice iteration in such scopes.
#[test]
fn push_and_retain_mut_prefer_inherent_over_seqview() {
    let mut v: Vec<i32> = vec![1, 2, 3];
    v.push(4); // inherent Vec::push (SeqView::push exists but inherent wins)
    v.retain_mut(|x| *x % 2 == 0); // inherent Vec::retain_mut
    assert_eq!(v, vec![2, 4], "inherent methods applied, no silent trait fallback");
    assert_eq!(<Vec<i32> as SeqView<i32>>::len(&v), 2); // trait still reachable via UFCS
    let _view: &dyn SeqView<i32> = &v;
}
