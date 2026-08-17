#![doc(html_logo_url = "https://raw.githubusercontent.com/yasuo-ozu/syan/main/syan.png")]
#![doc = include_str!("../../README.md")]
extern crate self as syan;

#[doc(hidden)]
pub use decycle as __decycle;

#[doc(hidden)]
pub mod decycle_traits;
/// Parse failures: `ParseError` and the traits around it.
pub mod error;
/// Combinator types you put in a field: groups, punctuated lists, and friends.
pub mod nested;
/// The `Parse` and `Unparse` traits, and the token stream.
pub mod parse;
pub mod source;
pub mod span;
pub mod symbol;
/// Generic tuple helpers used by the derives.
pub mod tuple;
pub mod visit;

#[doc(hidden)]
pub mod _imp {
    pub use crate::parse::{Parse, Unparse};
    pub use syan_macro;
}
