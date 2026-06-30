//! Cross-crate structural edits: the AST (`seqast::List` with a `Vec`/`Option` of `Item`) and its
//! visitor are upstream in `syan_rust`; this downstream crate implements the upstream `VisitMut` trait
//! and edits the upstream collections through the `SeqView`/`OptView` views (which live in `syan::visit`,
//! so there's no orphan problem). The inherent `.visit_mut` (generated upstream) drives the traversal.

use core::marker::PhantomData;
use syan::visit::{OptView, SeqView};
use syan_rust::seqast::{Item, List};
use syan_rust::seqvisit;

struct Editor;
impl<S> seqvisit::VisitMut<S> for Editor {
    fn visit_item_seq<V: SeqView<Item<S>>>(&mut self, v: &mut V) {
        v.edit_each(|c| match c.get().0 {
            0 => c.remove(),
            2 => c.replace(Item(102, PhantomData)),
            _ => {}
        });
        v.push(Item(9, PhantomData));
    }
    fn visit_item_opt<O: OptView<Item<S>>>(&mut self, v: &mut O) {
        if matches!(v.get(), Some(i) if i.0 == 0) {
            v.clear();
        }
    }
}

#[test]
fn downstream_edits_upstream_collections() {
    let mut list: List<()> = List {
        items: vec![
            Item(0, PhantomData),
            Item(1, PhantomData),
            Item(0, PhantomData),
            Item(2, PhantomData),
        ],
        last: Some(Item(0, PhantomData)),
    };
    // Inherent `.visit_mut` comes from the upstream `seqvisit` module.
    list.visit_mut(&mut Editor);
    assert_eq!(
        list.items.iter().map(|i| i.0).collect::<Vec<_>>(),
        vec![1, 102, 9],
        "0s removed, 2->102, 9 pushed — all via the cross-crate SeqView"
    );
    assert!(list.last.is_none(), "the zero in the Option tail was cleared via OptView");
}
