use super::ParseStream;
use core::convert::Infallible;

pub trait IntoParseStream: Sized {
    type Atom;
    type Output: ParseStream<Atom = Self::Atom>;

    fn into_parse_stream(self) -> Self::Output;
}

impl<T> IntoParseStream for T
where
    T: ParseStream,
{
    type Atom = T::Atom;
    type Output = T;

    fn into_parse_stream(self) -> Self::Output {
        self
    }
}

/// A [`ParseStream`] backed by an owned `Vec` of atoms — the canonical output of an eager (fully
/// tokenized) lexer. Atoms are yielded in order; `push` records LIFO pushback in `buf`, exactly like
/// [`crate::source::string::Stream`], so backtracking via [`ParseStream::dup`] restores position.
pub struct BufStream<A> {
    items: std::vec::IntoIter<A>,
    buf: Vec<A>,
}

impl<A> BufStream<A> {
    pub fn new(v: Vec<A>) -> Self {
        Self {
            items: v.into_iter(),
            buf: Vec::new(),
        }
    }
}

impl<A> ParseStream for BufStream<A> {
    type Atom = A;
    type Error = Infallible;

    fn next(&mut self) -> Option<A> {
        self.buf.pop().or_else(|| self.items.next())
    }

    fn peek(&mut self) -> Option<&A> {
        if self.buf.is_empty() {
            if let Some(a) = self.items.next() {
                self.buf.push(a);
            }
        }
        self.buf.last()
    }

    fn push(&mut self, a: A) {
        self.buf.push(a);
    }

    fn get_error(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    // Pre-tokenized: separators (if any) are already atoms, so nothing is ever skipped.
    fn skip_sep(&mut self) -> bool {
        false
    }
}

impl<A: Clone> IntoParseStream for Vec<A> {
    type Atom = A;
    type Output = BufStream<A>;

    fn into_parse_stream(self) -> Self::Output {
        BufStream::new(self)
    }
}

impl<A: Clone> IntoParseStream for &[A] {
    type Atom = A;
    type Output = BufStream<A>;

    fn into_parse_stream(self) -> Self::Output {
        BufStream::new(self.to_vec())
    }
}
