pub trait PopHead: Sized {
    type Head;
    type Rem;
    fn pop_head(self) -> (Self::Head, Self::Rem);
    fn unsplit(head: Self::Head, rem: Self::Rem) -> Self;
}

pub trait AsRef: PopHead {
    type AsRef<'a>: PopHead<Head = &'a Self::Head>
    where
        Self: 'a;
    fn as_ref(&self) -> Self::AsRef<'_>;
}

macro_rules! impl_for_tup {
    (@impl $a0:ident $A0:ident $($a:ident $A:ident)*) => {
        impl<$A0: Sized $(,$A: Sized)*> PopHead for ($A0, $($A),*) {
            type Head = $A0;
            type Rem = ($($A,)*);
            fn pop_head(self) -> (Self::Head, Self::Rem) {
                let ($a0,$($a,)*) = self;
                ($a0, ($($a,)*))
            }
            fn unsplit($a0: Self::Head, ($($a,)*): Self::Rem) -> Self {
                ($a0, $($a,)*)
            }
        }

        impl<$A0 $(,$A)*> AsRef for ($A0, $($A),*) {
            type AsRef<'a> = (&'a $A0, $(&'a $A),*) where Self: 'a;

            fn as_ref(&self) -> Self::AsRef<'_> {
                let ($a0, $($a),*) = self;
                ($a0, $($a),*)
            }
        }
    };
    () => {};
    ($a:ident $A:ident $($t:tt)*) => {
        impl_for_tup!(@impl $a $A $($t)*);
        impl_for_tup!($($t)*);
    };
}
impl_for_tup!(a0 A0 a1 A1 a2 A2 a3 A3 a4 A4 a5 A5 a6 A6 a7 A7 a8 A8 a9 A9 a10 A10 a11 A11 a12 A12 a13 A13);
