//! Ready-made input sources. Each submodule supplies an atom type, a span type, and a
//! [`ParseStream`](crate::parse::ParseStream) that feeds atoms to a parser.

#[cfg(feature = "proc_macro2")]
pub mod proc_macro2;

pub mod string;
