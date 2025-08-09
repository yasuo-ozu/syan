pub mod error;
pub mod nested;
pub mod parse;
pub mod source;
pub mod span;
pub mod symbol;
pub mod tuple;

#[doc(hidden)]
pub mod _imp {
    use crate::parse::Parse;
    pub use syan_macro;

    pub trait ParseImpl<const COUNT: usize, Atom>: Parse<Atom> {
        // This trait extends Parse with a COUNT const generic
    }
}
