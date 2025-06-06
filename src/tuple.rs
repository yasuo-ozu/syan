pub trait PopHead: Sized {
    type Head;
    type Rem;
    fn unsplit(head: Self::Head, rem: Self::Rem) -> Self;
}

macro_rules! impl_for_tup {
    (@impl $a0:ident $A0:ident $($a:ident $A:ident)*) => {
        impl<$A0: Sized $(,$A: Sized)*> PopHead for ($A0, $($A),*) {
            type Head = $A0;
            type Rem = ($($A,)*);
            fn unsplit($a0: Self::Head, ($($a,)*): Self::Rem) -> Self {
                ($a0, $($a,)*)
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
