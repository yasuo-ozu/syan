/// [`Attempt`] — parse a `T`, rewinding the stream if it fails.
pub mod attempt;
/// [`Group`](group::Group) and its `GroupParen`/`GroupBrace`/`GroupBracket` aliases — a delimited group.
pub mod group;
/// [`Joint`] — a tuple of parts with no separator allowed between them.
pub mod joint;
/// [`Punctuated`] — a list of items separated by a punctuation token.
pub mod punctuated;
/// [`Unordered`] — two things that may appear in either order.
pub mod unordered;

pub use attempt::Attempt;
pub use joint::Joint;
pub use punctuated::Punctuated;
pub use unordered::Unordered;
