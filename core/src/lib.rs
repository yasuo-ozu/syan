pub mod error;
pub mod nested;
pub mod parse;
pub mod source;
pub mod span;
pub mod symbol;
pub mod tuple;
pub mod visit;

#[doc(hidden)]
pub mod _imp {
    pub use crate::parse::{Parse, Unparse};
    pub use syan_macro;
}

/// Re-export of the `decycle` cycle-breaking crate, referenced by `#[recurse]`-generated `#[decycle]`
/// modules as `::syan::__decycle` (so the generated code and its runtime `__reentry` registry resolve
/// without the downstream crate depending on `decycle` directly). Phase-1 recurse-via-decycle; gated
/// on the `recurse-decycle` feature so default builds pull in nothing new.
#[cfg(feature = "recurse-decycle")]
#[doc(hidden)]
pub use ::decycle as __decycle;
