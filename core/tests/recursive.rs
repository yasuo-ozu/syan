use syan::parse::{Parse, Unparse};
#[derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum Expr<S> {
    Binary(ExprBinary<S>),
    // MethodCall(ExprMethodCall<S>),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct ExprBinary<S> {
    pub left: Box<Expr<S>>,
    _phantom: core::marker::PhantomData<S>,
}

// #[derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
// pub struct ExprMethodCall<S> {
//     pub turbofish: Option<crate::AngleBracketedGenericArguments<S>>,
//     _phantom: core::marker::PhantomData<S>,
// }
//
// #[derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
// pub struct AngleBracketedGenericArguments<S> {
//     pub args: Vec<GenericArgument<S>>,
// }

#[derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum GenericArgument<S> {
    Const(crate::Expr<S>),
}
