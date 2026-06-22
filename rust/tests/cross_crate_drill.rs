//! Cross-crate drill-in: a visitor built **downstream** (here) drills through an *upstream*
//! intermediate (`syan_rust::drillable::Wrap`). Drilling consults `Wrap`'s metadata, whose
//! `#[subast]` child (`crate::drillable::Inner`, written upstream) is `$crate`-rooted in the
//! metadata macro — so fetching/resolving it lands in the *upstream* crate, not this one.
//!
//! This file therefore compiles only because of the `$crate`-rooting: without it, `Wrap`'s subast
//! path would resolve to *this* crate's `crate::drillable::Inner` (which does not exist) and the
//! drill's metadata fetch would fail with "cannot find macro". (The upstream chain bottoms out at a
//! leaf — an upstream intermediate cannot reference a downstream visited type — so the drill is a
//! no-op at runtime; the proof is that it builds and the downstream `Root` is still visited.)

use core::marker::PhantomData;
use syan::visit::Ast;
use syan_rust::drillable::{Inner, Wrap};

#[derive(Debug, Ast)]
#[subast(syan_rust::drillable::Wrap)]
pub enum Root<S> {
    W(Wrap<S>),
    Leaf(PhantomData<S>),
}

pub mod visit {
    // `Root` is downstream (so its inherent `visit` is allowed here); `Wrap`/`Inner` are upstream
    // intermediates drilled through.
    syan::visit::visitor!(crate::Root);
}

#[test]
fn downstream_visitor_drills_through_upstream_intermediate() {
    let ast: Root<()> = Root::W(Wrap(Inner(PhantomData)));
    let mut roots = 0usize;
    ast.visit(|_r: &Root<()>| roots += 1);
    assert_eq!(roots, 1, "the downstream Root is visited; the upstream drill resolved via $crate");
}
