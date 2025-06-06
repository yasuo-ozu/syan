use parametrized::{Parametrized, ParametrizedIntoIter, ParametrizedIterMut};

#[parametrized::parametrized(default = 0, iter_mut = 0, into_iter = 0)]
struct PunctuatedInner<Item, Punct>(Option<(Item, Vec<(Punct, Item)>)>);

/// An punctuated list representation.
pub struct Punctuated<Item, Punct> {
    inner: PunctuatedInner<Item, Punct>,
}

impl<Item, Punct> Punctuated<Item, Punct> {
    pub fn iter(&self) -> Iter<'_, Item, Punct> {
        self.into_iter()
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, Item, Punct> {
        self.into_iter()
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
