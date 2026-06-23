//! Four-level inheritance across the crate boundary: `base => mid => upper => new`, where `base`,
//! `mid` AND `upper` all live upstream in `syan_rust` and only `new` is built here (downstream).
//!
//! This is what drives the requalification *loop* more than once. `upper` (upstream) records TWO
//! `crate::`-relative ancestors in its exported `@an` — `mid` (`crate::inherit::mid`) and `base`
//! (`crate::inherit::base`, transitively from `mid`). When the downstream `new` inherits
//! `syan_rust::inherit::upper`, it must requalify BOTH against `upper`'s host crate (`syan_rust`) so
//! its `Driver` satisfies the `upper::Visit`, `mid::Visit` AND `base::Visit` supertrait chain. Each
//! ancestor is rewritten exactly once.
//!
//! The visitor descends `File -> Block (upper) -> Item (mid) -> Expr (base) -> Type (base)`.

use core::marker::PhantomData;
use syan::visit::Ast;
use syan_rust::inherit::{Block, Expr, Item, Type};

// A downstream top-level node referencing the UPSTREAM `Block`.
#[derive(Debug, Ast)]
#[subast(syan_rust::inherit::Block)]
pub enum File<S> {
    B(Box<Block<S>>),
    Empty(PhantomData<S>),
}

// `new` inherits the UPSTREAM `upper` (transitively `mid` and `base`, all upstream).
pub mod new {
    syan::visit::visitor!(syan_rust::inherit::upper => crate::File);
}

#[derive(Default)]
struct Counter {
    types: u32,
    exprs: u32,
    items: u32,
    blocks: u32,
    files: u32,
}

impl<S> syan_rust::inherit::base::Visit<S> for Counter {
    fn visit_type(&mut self, i: &Type<S>) {
        self.types += 1;
        syan_rust::inherit::base::visit_type(self, i);
    }
    fn visit_expr(&mut self, i: &Expr<S>) {
        self.exprs += 1;
        syan_rust::inherit::base::visit_expr(self, i);
    }
}

impl<S> syan_rust::inherit::mid::Visit<S> for Counter {
    fn visit_item(&mut self, i: &Item<S>) {
        self.items += 1;
        syan_rust::inherit::mid::visit_item(self, i);
    }
}

impl<S> syan_rust::inherit::upper::Visit<S> for Counter {
    fn visit_block(&mut self, i: &Block<S>) {
        self.blocks += 1;
        syan_rust::inherit::upper::visit_block(self, i);
    }
}

impl<S> new::Visit<S> for Counter {
    fn visit_file(&mut self, i: &File<S>) {
        self.files += 1;
        new::visit_file(self, i);
    }
}

#[test]
fn four_level_inheritance_requalifies_two_upstream_ancestors() {
    // File::B -> Block::I -> Item::Ex -> Expr::Typed -> Type::Unit
    let ast: File<()> = File::B(Box::new(Block::I(Box::new(Item::Ex(Box::new(Expr::Typed(
        Box::new(Type::Unit(PhantomData)),
    )))))));
    let mut counter = Counter::default();
    ast.visit(&mut counter);
    assert_eq!(counter.files, 1, "new (downstream)");
    assert_eq!(counter.blocks, 1, "inherited upper method (upstream)");
    assert_eq!(counter.items, 1, "inherited mid method (upstream, requalified ancestor #1)");
    assert_eq!(counter.exprs, 1, "inherited base method (upstream, requalified ancestor #2)");
    assert_eq!(counter.types, 1, "inherited base method (upstream, requalified ancestor #2)");
}
