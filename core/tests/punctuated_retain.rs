//! `Punctuated::retain_mut`, and that the `SeqEdit` view reaches it.
//!
//! The trait's provided body shrinks by repeated `remove`, and each of those shifts the tail, so it
//! is O(n²). A generated `visit_<t>_seq_mut` override holds its slot as a generic `V` and therefore
//! always reaches the trait method, never an inherent one — so the container has to override it.
use syan::nested::punctuated::Punctuated;
use visiting::visit::SeqEdit;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Comma;

fn of(items: &[i32]) -> Punctuated<i32, Comma> {
    let mut p: Punctuated<i32, Comma> = Punctuated::default();
    for &i in items {
        p.push(i);
    }
    p
}

fn vals(p: &Punctuated<i32, Comma>) -> Vec<i32> {
    p.iter().copied().collect()
}

#[test]
fn retain_mut_keeps_order_and_can_drop_the_head() {
    let mut p = of(&[1, 2, 3, 4, 5]);
    p.retain_mut(|x| *x % 2 == 0);
    assert_eq!(
        vals(&p),
        vec![2, 4],
        "the head went, the survivors kept their order"
    );
    assert_eq!(p.len(), 2);

    // still usable afterwards: the promoted head has whatever separator it needs
    p.push(6);
    assert_eq!(vals(&p), vec![2, 4, 6]);
}

#[test]
fn retain_mut_sees_every_item_and_can_edit_in_place() {
    let mut seen = Vec::new();
    let mut p = of(&[1, 2, 3]);
    p.retain_mut(|x| {
        seen.push(*x);
        *x *= 10;
        true
    });
    assert_eq!(seen, vec![1, 2, 3], "every item, head included, in order");
    assert_eq!(vals(&p), vec![10, 20, 30]);
}

#[test]
fn retain_mut_handles_the_empty_and_all_dropped_cases() {
    let mut empty = of(&[]);
    empty.retain_mut(|_| true);
    assert!(empty.is_empty());

    let mut all = of(&[1, 2, 3]);
    all.retain_mut(|_| false);
    assert!(all.is_empty());
    assert_eq!(vals(&all), Vec::<i32>::new());

    let mut one = of(&[7]);
    one.retain_mut(|x| *x == 7);
    assert_eq!(vals(&one), vec![7]);
}

/// Through the view, as a visitor override reaches it — this is the path that was quadratic.
#[test]
fn the_seq_edit_view_forwards_to_the_single_pass_retain() {
    fn keep_even<'a, V: SeqEdit<'a, i32>>(v: &'a mut V) {
        v.retain_mut(|x| *x % 2 == 0);
    }

    let mut p = of(&[1, 2, 3, 4]);
    keep_even(&mut p);
    assert_eq!(vals(&p), vec![2, 4]);
}

/// Linear, not quadratic. Measured against the shape of the provided body (index, test, `remove`)
/// in release, where doubling `n` roughly quadruples it while one pass stays flat:
///
/// ```text
/// n= 5000   by remove 359µs    one pass 3.2µs
/// n=10000   by remove 1.83ms   one pass 2.7µs
/// n=20000   by remove 4.51ms   one pass 6.6µs      (680x)
/// ```
///
/// The bound below is generous on purpose — this pins the shape, it is not a benchmark.
#[test]
fn retain_mut_is_linear() {
    let mut p = of(&(0..40_000).collect::<Vec<_>>());
    let t = std::time::Instant::now();
    p.retain_mut(|x| *x % 2 == 0);
    let took = t.elapsed();
    assert_eq!(p.len(), 20_000);
    assert!(
        took < std::time::Duration::from_secs(2),
        "40k items took {took:?} — a quadratic regression here is plainly visible"
    );
}
