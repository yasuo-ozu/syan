//! A node handed a visited type through a generic parameter it does not follow. `peel` never treats a
//! bare type parameter as a head, so `Brackets`' `Vec<T>` is a leaf and the `Qubit`s put there are
//! unreachable however the visitor is written — a hand-implemented `Visit` would not see them either.
//! Previously silent: the walk compiled and visited nothing.
use core::marker::PhantomData;
use syan::visit::Ast;

#[derive(Ast)]
pub struct Qubit<S>(pub i64, pub PhantomData<S>);

#[derive(Ast)]
#[subast(crate::Qubit)]
pub struct Brackets<T, S> {
    pub items: Vec<T>, // `T` is never a head — this field is a LEAF
    pub tag: PhantomData<S>,
}

#[derive(Ast)]
#[subast(crate::Qubit, crate::Brackets)]
pub struct Stmt<S> {
    pub b: Brackets<Qubit<S>, S>,
}

pub mod v {
    syan::visit::visitor!(crate::Qubit, crate::Brackets, crate::Stmt);
}

fn main() {
    let _ = PhantomData::<()>;
}
