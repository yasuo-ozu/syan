
pub mod attr;
pub mod expr;
pub mod generics;
pub mod item;
pub mod lit;
pub mod pat;
pub mod path;
pub mod stmt;
pub mod tokens;
pub mod ty;
pub mod vis;

pub use attr::*;
pub use expr::*;
pub use generics::*;
pub use item::*;
pub use lit::*;
pub use pat::*;
pub use path::*;
pub use stmt::*;
pub use tokens::*;
pub use ty::*;
pub use vis::*;

/// Top-level Rust source file

pub struct File<S, Tokens = std::convert::Infallible> {
    pub shebang: Option<String>,
    pub attrs: Vec<Attribute<S>>,
    pub items: Vec<Item<S, Tokens>>,
}
