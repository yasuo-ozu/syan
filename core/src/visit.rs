//! Visitor-system support items.
//!
//! The generated visitor modules and the `#[derive(Ast)]` metadata macros live in user crates;
//! this module only holds the two cross-crate primitives they rely on:
//!
//! * [`Ast`] — an empty marker trait implemented by `#[derive(Ast)]` for every AST node type.
//! * [`Repeater`] — the `type-leak` indirection trait. `#[derive(Ast)]` emits one
//!   `impl Repeater<N> for <leaker>` per field type that depends on the definition's type context,
//!   so a generated visitor module can name those types portably as
//!   `<leaker as ::syan::visit::Repeater<N>>::Type` regardless of which crate/module it expands in.
//!
//! See `CLAUDE.md` for the full design.

pub use syan_macro::{visitor, Ast};

/// Marker trait implemented by every type carrying `#[derive(Ast)]`.
///
/// It carries no methods; its only purpose is to let generic code (and the `#[visitor]` generator)
/// bound on "is an AST node".
pub trait Ast {}

/// `type-leak` repeater: passes a single type out of the leaker's type context to a referrer.
///
/// `INDEX` distinguishes the type references collected from one leaker definition (in declaration
/// order, matching [`type_leak::Referrer::iter`]). The `#[derive(Ast)]` macro implements this for a
/// generated leaker marker type; generated visitor code refers back through it.
pub trait Repeater<const INDEX: usize> {
    /// The leaked type, valid in the referrer's context via `<Leaker as Repeater<INDEX>>::Type`.
    type Type: ?Sized;
}
