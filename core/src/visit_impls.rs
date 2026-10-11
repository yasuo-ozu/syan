//! Walk and view impls for this crate's own containers.
//!
//! The traits live in [`visiting::visit`]; these types are ours, so the impls belong here — a foreign
//! trait for a local type. [`Punctuated`](crate::nested::Punctuated) is an edit target like `Vec`;
//! [`Attempt`](crate::nested::Attempt) and [`Group`](crate::nested::group::Group) hold exactly one
//! node, so they are descent-only.

use crate::nested::punctuated::{Iter, IterMut};
use crate::nested::{group::Group, Attempt, Punctuated};
use visiting::visit::{Elements, SeqEdit, SeqView};
use visiting::walk::{Thru, Walk, WalkMut};

// `SeqView` reaches elements through these, so they come first. Both panic past the end, like every
// other indexed container.
impl<T, P> core::ops::Index<usize> for Punctuated<T, P> {
    type Output = T;
    fn index(&self, i: usize) -> &T {
        Punctuated::get(self, i).expect("index in bounds")
    }
}

impl<T, P> core::ops::IndexMut<usize> for Punctuated<T, P> {
    fn index_mut(&mut self, i: usize) -> &mut T {
        Punctuated::get_mut(self, i).expect("index in bounds")
    }
}

impl<'a, T: 'a, P: 'a> SeqView<'a, T> for Punctuated<T, P> {
    fn len(&self) -> usize {
        Punctuated::len(self)
    }
}

// `insert`/`push` synthesize the separator via `Punct::default()`, hence `P: Default`.
impl<'a, T: 'a, P: Default + 'a> SeqEdit<'a, T> for Punctuated<T, P> {
    fn insert(&mut self, i: usize, value: T) {
        Punctuated::insert(self, i, value);
    }
    fn remove(&mut self, i: usize) -> T {
        Punctuated::remove(self, i).expect("index in bounds")
    }
    fn push(&mut self, value: T) {
        Punctuated::push(self, value);
    }
    // The provided body shrinks by repeated `remove`, and each of those shifts the tail. A
    // generated `visit_<t>_seq_mut` override holds its slot as a generic `V`, so it always reaches
    // the trait method and never an inherent one — the quadratic cost would be real.
    fn retain_mut(&mut self, f: impl FnMut(&mut T) -> bool) {
        Punctuated::retain_mut(self, f);
    }
}

// The root of the view traits: `SeqView` has `Elements` as a supertrait, so this is what makes
// `Punctuated` usable in a `#[seq]` field at all. It hands back `Punctuated`'s own iterators, so
// nothing here is synthesized out of `get`/`get_mut`.
impl<'a, T: 'a, P: 'a> Elements<'a, T> for Punctuated<T, P> {
    type Iter = Iter<'a, T, P>;
    type IterMut = IterMut<'a, T, P>;
    fn elements(&'a self) -> Self::Iter {
        Punctuated::iter(self)
    }
    fn elements_mut(&'a mut self) -> Self::IterMut {
        Punctuated::iter_mut(self)
    }
}

impl<M, H, V: ?Sized, T: Walk<M, H, V>, P> Walk<M, Thru<H>, V> for Punctuated<T, P> {
    fn walk(&self, v: &mut V) {
        for x in self.iter() {
            x.walk(v);
        }
    }
}

impl<M, H, V: ?Sized, T: WalkMut<M, H, V>, P> WalkMut<M, Thru<H>, V> for Punctuated<T, P> {
    fn walk_mut(&mut self, v: &mut V) {
        for x in self.iter_mut() {
            x.walk_mut(v);
        }
    }
}

impl<M, H, V: ?Sized, T: Walk<M, H, V>> Walk<M, Thru<H>, V> for Attempt<T> {
    fn walk(&self, v: &mut V) {
        self.0.walk(v);
    }
}

impl<M, H, V: ?Sized, T: WalkMut<M, H, V>> WalkMut<M, Thru<H>, V> for Attempt<T> {
    fn walk_mut(&mut self, v: &mut V) {
        self.0.walk_mut(v);
    }
}

// A delimited group walks its content and not its delimiters: `O` and `C` are punctuation tokens,
// never AST nodes. Descent-only like every other single-slot wrapper — a `Group` holds exactly one
// `T`, so there is nothing for `SeqEdit`'s `push`/`remove` or `OptEdit`'s `take` to mean, and
// `#[seq]`/`#[opt]` on such a field stays the error it already was.
impl<M, H, V: ?Sized, T: Walk<M, H, V>, O, C> Walk<M, Thru<H>, V> for Group<T, O, C> {
    fn walk(&self, v: &mut V) {
        self.slot.walk(v);
    }
}

impl<M, H, V: ?Sized, T: WalkMut<M, H, V>, O, C> WalkMut<M, Thru<H>, V> for Group<T, O, C> {
    fn walk_mut(&mut self, v: &mut V) {
        self.slot.walk_mut(v);
    }
}
