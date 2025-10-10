use crate::error::{ParseError, UnionWith};
use crate::parse::{Parse, ParseStream, Unparse};
use crate::span::Spanned;
use parametrized::{Parametrized, ParametrizedIntoIter, ParametrizedIterMut};

#[parametrized::parametrized(default = 0, iter_mut = 0, into_iter = 0)]
#[derive(Clone, Debug, PartialEq, Eq, Hash, Unparse, Spanned)]
#[syan(crate)]
struct PunctuatedInner<Item, Punct>(Option<(Box<Item>, Vec<(Punct, Item)>)>);

/// An punctuated list representation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Unparse, Spanned)]
#[syan(crate)]
pub struct Punctuated<Item, Punct> {
    inner: PunctuatedInner<Item, Punct>,
}

impl<Item, Punct> Default for Punctuated<Item, Punct> {
    fn default() -> Self {
        Self {
            inner: PunctuatedInner(None),
        }
    }
}

impl<Item, Punct> Punctuated<Item, Punct> {
    pub fn iter(&self) -> Iter<'_, Item, Punct> {
        self.into_iter()
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, Item, Punct> {
        self.into_iter()
    }

    pub fn len(&self) -> usize {
        match &self.inner.0 {
            None => 0,
            Some((_, vec)) => 1 + vec.len(),
        }
    }

    pub fn first(&self) -> Option<&Item> {
        self.inner.0.as_ref().map(|(first, _)| first.as_ref())
    }

    pub fn last(&self) -> Option<&Item> {
        match &self.inner.0 {
            None => None,
            Some((first, vec)) => {
                if vec.is_empty() {
                    Some(first.as_ref())
                } else {
                    vec.last().map(|(_, item)| item)
                }
            }
        }
    }

    pub fn first_mut(&mut self) -> Option<&mut Item> {
        self.inner.0.as_mut().map(|(first, _)| first.as_mut())
    }

    pub fn last_mut(&mut self) -> Option<&mut Item> {
        match &mut self.inner.0 {
            None => None,
            Some((first, vec)) => {
                if vec.is_empty() {
                    Some(first.as_mut())
                } else {
                    vec.last_mut().map(|(_, item)| item)
                }
            }
        }
    }

    pub fn remove(&mut self, index: usize) -> Option<Item> {
        match self.inner.0.take() {
            None => None,
            Some((first_item, mut vec)) => {
                if index == 0 {
                    let removed = *first_item;
                    if vec.is_empty() {
                        // List becomes empty
                    } else {
                        let (_, new_first) = vec.remove(0);
                        self.inner.0 = Some((Box::new(new_first), vec));
                    }
                    Some(removed)
                } else if index <= vec.len() {
                    let removed = vec.remove(index - 1).1;
                    self.inner.0 = Some((first_item, vec));
                    Some(removed)
                } else {
                    // Index out of bounds, restore the original state
                    self.inner.0 = Some((first_item, vec));
                    None
                }
            }
        }
    }
}

impl<Item, Punct: Default> Punctuated<Item, Punct> {
    pub fn push(&mut self, item: Item) {
        match &mut self.inner.0 {
            None => {
                self.inner.0 = Some((Box::new(item), Vec::new()));
            }
            Some((_, ref mut vec)) => {
                vec.push((Punct::default(), item));
            }
        }
    }

    pub fn insert(&mut self, index: usize, item: Item) {
        match (&mut self.inner.0, index) {
            (None, 0) => {
                self.inner.0 = Some((Box::new(item), Vec::new()));
            }
            (Some((first_item, ref mut vec)), 0) => {
                let old_first = std::mem::replace(first_item, Box::new(item));
                vec.insert(0, (Punct::default(), *old_first));
            }
            (Some((_, ref mut vec)), i) if i <= vec.len() + 1 => {
                vec.insert(i - 1, (Punct::default(), item));
            }
            _ => {
                panic!("Index out of bounds");
            }
        }
    }
}

pub struct Iter<'a, Item: 'a, Punct: 'a>(
    <PunctuatedInner<Item, Punct> as Parametrized<0>>::Iter<'a>,
);

impl<'a, Item: Sized, Punct> Iterator for Iter<'a, Item, Punct> {
    type Item = &'a Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<'a, Item: Sized, Punct> ExactSizeIterator for Iter<'a, Item, Punct> where
    <PunctuatedInner<Item, Punct> as Parametrized<0>>::Iter<'a>: ExactSizeIterator
{
}

impl<'a, Item: Sized, Punct> std::iter::FusedIterator for Iter<'a, Item, Punct> where
    <PunctuatedInner<Item, Punct> as Parametrized<0>>::Iter<'a>: std::iter::FusedIterator
{
}

pub struct IterMut<'a, Item: 'a, Punct: 'a>(
    <PunctuatedInner<Item, Punct> as ParametrizedIterMut<0>>::IterMut<'a>,
);

impl<'a, Item: Sized, Punct> Iterator for IterMut<'a, Item, Punct> {
    type Item = &'a mut Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<'a, Item: Sized, Punct> ExactSizeIterator for IterMut<'a, Item, Punct> where
    <PunctuatedInner<Item, Punct> as ParametrizedIterMut<0>>::IterMut<'a>: ExactSizeIterator
{
}

impl<'a, Item: Sized, Punct> std::iter::FusedIterator for IterMut<'a, Item, Punct> where
    <PunctuatedInner<Item, Punct> as ParametrizedIterMut<0>>::IterMut<'a>: std::iter::FusedIterator
{
}

pub struct IntoIter<Item, Punct>(
    <PunctuatedInner<Item, Punct> as ParametrizedIntoIter<0>>::IntoIter,
);

impl<Item: Sized, Punct> Iterator for IntoIter<Item, Punct> {
    type Item = Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<Item: Sized, Punct> ExactSizeIterator for IntoIter<Item, Punct> where
    <PunctuatedInner<Item, Punct> as ParametrizedIntoIter<0>>::IntoIter: ExactSizeIterator
{
}

impl<Item: Sized, Punct> std::iter::FusedIterator for IntoIter<Item, Punct> where
    <PunctuatedInner<Item, Punct> as ParametrizedIntoIter<0>>::IntoIter: std::iter::FusedIterator
{
}

impl<'a, Item: Sized, Punct> std::iter::IntoIterator for &'a Punctuated<Item, Punct> {
    type Item = &'a Item;
    type IntoIter = Iter<'a, Item, Punct>;

    fn into_iter(self) -> Self::IntoIter {
        Iter(<PunctuatedInner<Item, Punct> as Parametrized<0>>::param_iter(&self.inner))
    }
}

impl<'a, Item: Sized, Punct> std::iter::IntoIterator for &'a mut Punctuated<Item, Punct> {
    type Item = &'a mut Item;
    type IntoIter = IterMut<'a, Item, Punct>;

    fn into_iter(self) -> Self::IntoIter {
        IterMut(
            <PunctuatedInner<Item, Punct> as ParametrizedIterMut<0>>::param_iter_mut(
                &mut self.inner,
            ),
        )
    }
}

impl<Item: Sized, Punct> std::iter::IntoIterator for Punctuated<Item, Punct> {
    type Item = Item;
    type IntoIter = IntoIter<Item, Punct>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter(
            <PunctuatedInner<Item, Punct> as ParametrizedIntoIter<0>>::param_into_iter(self.inner),
        )
    }
}

impl<Item, Punct: Default> std::iter::FromIterator<Item> for Punctuated<Item, Punct> {
    fn from_iter<T: IntoIterator<Item = Item>>(iter: T) -> Self {
        let mut punctuated = Self::default();
        punctuated.extend(iter);
        punctuated
    }
}

impl<Item, Punct: Default> std::iter::Extend<Item> for Punctuated<Item, Punct> {
    fn extend<T: IntoIterator<Item = Item>>(&mut self, iter: T) {
        for item in iter {
            self.push(item);
        }
    }
}

impl<Atom: Clone + Spanned, Item, Punct> Parse<Atom> for Punctuated<Item, Punct>
where
    Item: Parse<Atom>,
    Punct: Parse<Atom>,
    Item::Error: crate::error::UnionWith<Punct::Error>,
    <Item::Error as crate::error::UnionWith<Punct::Error>>::Output: Into<ParseError>,
{
    type Error = <Item::Error as crate::error::UnionWith<Punct::Error>>::Output;

    fn parse(stream: impl crate::parse::IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();

        // Try to parse the first item
        let first_item = match stream.dup(|stream| Item::parse(stream)) {
            Ok(item) => item,
            Err(_) => {
                // No items, return empty punctuated list
                return Ok(Self::default());
            }
        };

        let mut pairs = Vec::new();

        // Parse subsequent (punct, item) pairs
        loop {
            let pair: Result<_, Self::Error> = stream.dup(|mut stream| {
                let punct = Punct::parse(&mut stream)
                    .map_err(<Item::Error as UnionWith<Punct::Error>>::use_right)?;
                let item = Item::parse(&mut stream)
                    .map_err(<Item::Error as UnionWith<Punct::Error>>::use_left)?;
                Ok((punct, item))
            });

            match pair {
                Ok((punct, item)) => pairs.push((punct, item)),
                Err(_) => break,
            }
        }

        Ok(Self {
            inner: PunctuatedInner(Some((Box::new(first_item), pairs))),
        })
    }
}
