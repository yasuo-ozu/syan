use crate::{tokens::*, Expr, Type};
use core::iter::IntoIterator;
use syan::{
    error::ParseError,
    nested::{
        group::{Group, GroupAngle},
        punctuated::Punctuated,
    },
    parse::{IntoParseStream, Parse, Unparse},
    span::Spanned,
    symbol::Token,
};
use type_macro_derive_tricks::macro_derive;

/// A generic parameter
/// Examples: T: Clone, 'a: 'static, const N: usize
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum GenericParam<S> {
    Type {
        ident: Ident<S>,
        bounds: Option<(Token![S => :], Punctuated<crate::Type<S>, Token![S => +]>)>,
    },
    Lifetime {
        lifetime: crate::Lifetime<S>,
        colon_token: Option<Token![S => :]>,
        bounds: Option<(
            Token![S => :],
            Punctuated<crate::Lifetime<S>, Token![S => +]>,
        )>,
    },
    Const {
        const_token: Token![S => const],
        ident: Ident<S>,
        colon_token: Token![S => :],
        ty: Type<S>,
    },
}

#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum GenericDefParam<S, Tokens = core::convert::Infallible> {
    Type {
        ident: Ident<S>,
        bounds: Option<(Token![S => :], Punctuated<crate::Type<S>, Token![S => +]>)>,
        default: Option<(Token![S => =], Type<S>)>,
    },
    Lifetime {
        lifetime: crate::Lifetime<S>,
        bounds: Option<(
            Token![S => :],
            Punctuated<crate::Lifetime<S>, Token![S => +]>,
        )>,
    },
    Const {
        const_token: Token![S => const],
        ident: Ident<S>,
        colon_token: Token![S => :],
        ty: Type<S>,
        default: Option<(Token![S => =], crate::Expr<S, Tokens>)>,
    },
}

/// A generic argument
#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub enum GenericArgument<S, Tokens = std::convert::Infallible> {
    Type(crate::Type<S>),
    Const(crate::Expr<S, Tokens>),
    Lifetime(crate::Lifetime<S>),
    Binding(crate::Binding<S>),
    Constraint(crate::Constraint<S>),
}

impl<S> From<GenericParam<S>> for GenericDefParam<S> {
    fn from(value: GenericParam<S>) -> Self {
        match value {
            GenericParam::Type { ident, bounds } => GenericDefParam::Type {
                ident,
                bounds,
                default: None,
            },
            GenericParam::Lifetime {
                lifetime,
                colon_token,
                bounds,
            } => GenericDefParam::Lifetime {
                lifetime,
                bounds: if let Some(colon) = colon_token {
                    bounds.map(|(_, b)| (colon, b))
                } else {
                    bounds
                },
            },
            GenericParam::Const {
                const_token,
                ident,
                colon_token,
                ty,
            } => GenericDefParam::Const {
                const_token,
                ident,
                colon_token,
                ty,
                default: None,
            },
        }
    }
}

#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Parse, Unparse)]
pub struct TypeGenerics<S> {
    angle_token: GroupAngle<(), S>,
    #[group(self.angle_token)]
    params: Punctuated<GenericArgument<S>, Token![S => ,]>,
    #[group(self.angle_token)]
    trailing_comma: Option<Token![S => ,]>,
}

#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Unparse)]
pub struct ImplGenerics<S> {
    angle_token: GroupAngle<(), S>,
    #[group(self.angle_token)]
    params: Punctuated<GenericParam<S>, Token![S => ,]>,
    trailing_comma: Option<Token![S => ,]>,
}

impl<S, Atom, E> Parse<Atom> for ImplGenerics<S>
where
    S: Spanned,
    GroupAngle<
        (
            Punctuated<GenericParam<S>, Token![S => ,]>,
            Option<Token![S => ,]>,
        ),
        S,
    >: Parse<Atom, Error = E>,
    E: syan::error::UnionWith<ParseError>,
{
    type Error = <E as syan::error::UnionWith<ParseError>>::Output;

    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        let angle_token: GroupAngle<
            (
                Punctuated<GenericParam<S>, Token![S => ,]>,
                Option<Token![S => ,]>,
            ),
            S,
        > = Parse::parse(&mut stream).map_err(<_ as syan::error::UnionWith<_>>::use_left)?;

        // Validate parameter order: lifetimes must come first
        let mut seen_type_or_const = false;
        for param in angle_token.slot.0.iter() {
            match param {
                GenericParam::Lifetime { .. } => {
                    if seen_type_or_const {
                        return Err(<E as syan::error::UnionWith<ParseError>>::use_right(
                            ParseError::new(
                                (),
                                "lifetime parameters must come before type and const parameters",
                            ),
                        ));
                    }
                }
                GenericParam::Type { .. } | GenericParam::Const { .. } => {
                    seen_type_or_const = true;
                }
            }
        }

        Ok(ImplGenerics {
            angle_token: Group {
                open: angle_token.open,
                slot: (),
                close: angle_token.close,
            },
            params: angle_token.slot.0,
            trailing_comma: angle_token.slot.1,
        })
    }
}

#[macro_derive(Clone, Debug, PartialEq, Eq, Hash, Unparse)]
pub struct TypeDefGenerics<S> {
    angle_token: GroupAngle<(), S>,
    #[group(self.angle_token)]
    params: Punctuated<GenericDefParam<S>, Token![S => ,]>,
    trailing_comma: Option<Token![S => ,]>,
}

impl<S, Atom, E> Parse<Atom> for TypeDefGenerics<S>
where
    S: Spanned,
    GroupAngle<
        (
            Punctuated<GenericDefParam<S>, Token![S => ,]>,
            Option<Token![S => ,]>,
        ),
        S,
    >: Parse<Atom, Error = E>,
    E: syan::error::UnionWith<ParseError>,
{
    type Error = <E as syan::error::UnionWith<ParseError>>::Output;

    fn parse(stream: impl IntoParseStream<Atom = Atom>) -> Result<Self, Self::Error> {
        let mut stream = stream.into_parse_stream();
        let angle_token: GroupAngle<
            (
                Punctuated<GenericDefParam<S>, Token![S => ,]>,
                Option<Token![S => ,]>,
            ),
            S,
        > = Parse::parse(&mut stream).map_err(<_ as syan::error::UnionWith<_>>::use_left)?;

        // Validate parameter order: lifetimes must come first, const generics or type parameters with defaults must be last
        let mut seen_type_or_const = false;
        let mut seen_default = false;

        for param in angle_token.slot.0.iter() {
            match param {
                GenericDefParam::Lifetime { .. } => {
                    if seen_type_or_const {
                        return Err(<E as syan::error::UnionWith<ParseError>>::use_right(
                            ParseError::new(
                                (),
                                "lifetime parameters must come before type and const parameters",
                            ),
                        ));
                    }
                }
                GenericDefParam::Type { default, .. } => {
                    seen_type_or_const = true;
                    if default.is_some() {
                        seen_default = true;
                    } else if seen_default {
                        return Err(<E as syan::error::UnionWith<ParseError>>::use_right(
                            ParseError::new(
                                (),
                                "type parameters with defaults must come after those without defaults",
                            ),
                        ));
                    }
                }
                GenericDefParam::Const { default, .. } => {
                    seen_type_or_const = true;
                    if default.is_some() {
                        seen_default = true;
                    } else if seen_default {
                        return Err(<E as syan::error::UnionWith<ParseError>>::use_right(
                            ParseError::new(
                                (),
                                "const parameters with defaults must come after those without defaults",
                            ),
                        ));
                    }
                }
            }
        }

        Ok(TypeDefGenerics {
            angle_token: Group {
                open: angle_token.open,
                slot: (),
                close: angle_token.close,
            },
            params: angle_token.slot.0,
            trailing_comma: angle_token.slot.1,
        })
    }
}

impl<S> core::iter::IntoIterator for TypeGenerics<S> {
    type Item = GenericArgument<S>;
    type IntoIter = <Punctuated<GenericArgument<S>, Token![S => ,]> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.params.into_iter()
    }
}

impl<S> ImplGenerics<S> {
    pub fn push(&mut self, item: impl Into<GenericParam<S>>)
    where
        Token![S => ,]: Default,
    {
        // We should consider the order. lifetimes must be the first. if the lifetime have some
        // bounds, they should be appear earlier.
        let param = item.into();

        match param {
            GenericParam::Lifetime { .. } => {
                // Insert lifetimes at the beginning, maintaining order with bounds first
                let mut insert_pos = 0;
                for (i, existing) in self.params.iter().enumerate() {
                    match existing {
                        GenericParam::Lifetime { bounds, .. } => {
                            // If existing lifetime has bounds and new one doesn't, insert after
                            if let GenericParam::Lifetime {
                                bounds: new_bounds, ..
                            } = &param
                            {
                                if bounds.is_some() && new_bounds.is_none() {
                                    insert_pos = i + 1;
                                }
                            }
                        }
                        _ => break, // Stop at first non-lifetime
                    }
                }
                self.params.insert(insert_pos, param);
            }
            _ => {
                // Type and const parameters go after all lifetimes
                self.params.push(param);
            }
        }
    }

    pub fn lifetimes(&self) -> impl Iterator<Item = &crate::Lifetime<S>> {
        self.params.iter().filter_map(|param| match param {
            GenericParam::Lifetime { lifetime, .. } => Some(lifetime),
            _ => None,
        })
    }

    pub fn consts(&self) -> impl Iterator<Item = (&Ident<S>, &Type<S>)> {
        self.params.iter().filter_map(|param| match param {
            GenericParam::Const { ident, ty, .. } => Some((ident, ty)),
            _ => None,
        })
    }

    pub fn tys(&self) -> impl Iterator<Item = &Ident<S>> {
        self.params.iter().filter_map(|param| match param {
            GenericParam::Type { ident, .. } => Some(ident),
            _ => None,
        })
    }

    pub fn lifetimes_mut(&mut self) -> impl Iterator<Item = &mut crate::Lifetime<S>> {
        self.params.iter_mut().filter_map(|param| match param {
            GenericParam::Lifetime { lifetime, .. } => Some(lifetime),
            _ => None,
        })
    }

    pub fn consts_mut(&mut self) -> impl Iterator<Item = (&mut Ident<S>, &mut Type<S>)> {
        self.params.iter_mut().filter_map(|param| match param {
            GenericParam::Const { ident, ty, .. } => Some((ident, ty)),
            _ => None,
        })
    }

    pub fn tys_mut(&mut self) -> impl Iterator<Item = &mut Ident<S>> {
        self.params.iter_mut().filter_map(|param| match param {
            GenericParam::Type { ident, .. } => Some(ident),
            _ => None,
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &GenericParam<S>> {
        self.params.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut GenericParam<S>> {
        self.params.iter_mut()
    }
}

impl<S: Default> TypeDefGenerics<S> {
    pub fn push(&mut self, item: impl Into<GenericDefParam<S>>)
    where
        Token![S => ,]: Default,
    {
        // We should consider the order. lifetimes must be the first. if the lifetime have some
        // bounds, they should be appear earlier.
        let param = item.into();

        match param {
            GenericDefParam::Lifetime { .. } => {
                // Insert lifetimes at the beginning, maintaining order with bounds first
                let mut insert_pos = 0;
                for (i, existing) in self.params.iter().enumerate() {
                    match existing {
                        GenericDefParam::Lifetime { bounds, .. } => {
                            // If existing lifetime has bounds and new one doesn't, insert after
                            if let GenericDefParam::Lifetime {
                                bounds: new_bounds, ..
                            } = &param
                            {
                                if bounds.is_some() && new_bounds.is_none() {
                                    insert_pos = i + 1;
                                }
                            }
                        }
                        _ => break, // Stop at first non-lifetime
                    }
                }
                self.params.insert(insert_pos, param);
            }
            _ => {
                // Type and const parameters go after all lifetimes
                self.params.push(param);
            }
        }
    }

    pub fn into_impl_generics_lossy(self) -> TypeGenerics<S> {
        TypeGenerics {
            angle_token: self.angle_token,
            params: self
                .params
                .into_iter()
                .map(|param| match param {
                    GenericDefParam::Type { ident, .. } => {
                        GenericArgument::Type(Type::Path(crate::TypePath {
                            path: crate::Path {
                                leading_colon: None,
                                segments: core::iter::once(crate::PathSegment {
                                    ident,
                                    arguments: None,
                                })
                                .collect(),
                            },
                        }))
                    }
                    GenericDefParam::Lifetime { lifetime, .. } => {
                        GenericArgument::Lifetime(lifetime)
                    }
                    GenericDefParam::Const { ident, .. } => {
                        GenericArgument::Const(Expr::Path(crate::ExprPath {
                            qself: None,
                            path: crate::Path {
                                leading_colon: None,
                                segments: core::iter::once(crate::PathSegment {
                                    ident,
                                    arguments: None,
                                })
                                .collect(),
                            },
                        }))
                    }
                })
                .collect(),
            trailing_comma: self.trailing_comma,
        }
    }

    pub fn lifetimes(&self) -> impl Iterator<Item = &crate::Lifetime<S>> {
        self.params.iter().filter_map(|param| match param {
            GenericDefParam::Lifetime { lifetime, .. } => Some(lifetime),
            _ => None,
        })
    }

    pub fn consts(&self) -> impl Iterator<Item = (&Ident<S>, &Type<S>)> {
        self.params.iter().filter_map(|param| match param {
            GenericDefParam::Const { ident, ty, .. } => Some((ident, ty)),
            _ => None,
        })
    }

    pub fn tys(&self) -> impl Iterator<Item = &Ident<S>> {
        self.params.iter().filter_map(|param| match param {
            GenericDefParam::Type { ident, .. } => Some(ident),
            _ => None,
        })
    }

    pub fn lifetimes_mut(&mut self) -> impl Iterator<Item = &mut crate::Lifetime<S>> {
        self.params.iter_mut().filter_map(|param| match param {
            GenericDefParam::Lifetime { lifetime, .. } => Some(lifetime),
            _ => None,
        })
    }

    pub fn consts_mut(&mut self) -> impl Iterator<Item = (&mut Ident<S>, &mut Type<S>)> {
        self.params.iter_mut().filter_map(|param| match param {
            GenericDefParam::Const { ident, ty, .. } => Some((ident, ty)),
            _ => None,
        })
    }

    pub fn tys_mut(&mut self) -> impl Iterator<Item = &mut Ident<S>> {
        self.params.iter_mut().filter_map(|param| match param {
            GenericDefParam::Type { ident, .. } => Some(ident),
            _ => None,
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &GenericDefParam<S>> {
        self.params.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut GenericDefParam<S>> {
        self.params.iter_mut()
    }
}

impl<S: Default> TypeGenerics<S> {
    pub fn push(&mut self, item: impl Into<GenericArgument<S>>) {
        // We should consider the order. lifetimes must be the first. if the lifetime have some
        // bounds, they should be appear earlier. const generics and type params with defaults must
        // be the last.
        let arg = item.into();

        match arg {
            GenericArgument::Lifetime(_) => {
                // Insert lifetimes at the beginning
                let mut insert_pos = 0;
                for (i, existing) in self.params.iter().enumerate() {
                    if !matches!(existing, GenericArgument::Lifetime(_)) {
                        break;
                    }
                    insert_pos = i + 1;
                }
                self.params.insert(insert_pos, arg);
            }
            GenericArgument::Const(_) => {
                // Insert const generics before type arguments but after lifetimes
                let mut insert_pos = self.params.len();
                for (i, existing) in self.params.iter().enumerate() {
                    match existing {
                        GenericArgument::Type(_)
                        | GenericArgument::Binding(_)
                        | GenericArgument::Constraint(_) => {
                            insert_pos = i;
                            break;
                        }
                        _ => {}
                    }
                }
                self.params.insert(insert_pos, arg);
            }
            _ => {
                // Type arguments, bindings, and constraints go last
                self.params.push(arg);
            }
        }
    }

    pub fn lifetimes(&self) -> impl Iterator<Item = &crate::Lifetime<S>> {
        self.params.iter().filter_map(|arg| match arg {
            GenericArgument::Lifetime(lifetime) => Some(lifetime),
            _ => None,
        })
    }

    pub fn consts(&self) -> impl Iterator<Item = &crate::Expr<S, std::convert::Infallible>> {
        self.params.iter().filter_map(|arg| match arg {
            GenericArgument::Const(expr) => Some(expr),
            _ => None,
        })
    }

    pub fn tys(&self) -> impl Iterator<Item = &crate::Type<S>> {
        self.params.iter().filter_map(|arg| match arg {
            GenericArgument::Type(ty) => Some(ty),
            _ => None,
        })
    }

    pub fn lifetimes_mut(&mut self) -> impl Iterator<Item = &mut crate::Lifetime<S>> {
        self.params.iter_mut().filter_map(|arg| match arg {
            GenericArgument::Lifetime(lifetime) => Some(lifetime),
            _ => None,
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &GenericArgument<S>> {
        self.params.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut GenericArgument<S>> {
        self.params.iter_mut()
    }
}

impl<S> core::iter::IntoIterator for ImplGenerics<S> {
    type Item = GenericParam<S>;
    type IntoIter = <Punctuated<GenericParam<S>, Token![S => ,]> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.params.into_iter()
    }
}

impl<S> core::iter::IntoIterator for TypeDefGenerics<S> {
    type Item = GenericDefParam<S>;
    type IntoIter = <Punctuated<GenericDefParam<S>, Token![S => ,]> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.params.into_iter()
    }
}

impl<'a, S> core::iter::IntoIterator for &'a TypeGenerics<S> {
    type Item = &'a GenericArgument<S>;
    type IntoIter = <&'a Punctuated<GenericArgument<S>, Token![S => ,]> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        (&self.params).into_iter()
    }
}

impl<'a, S> core::iter::IntoIterator for &'a ImplGenerics<S> {
    type Item = &'a GenericParam<S>;
    type IntoIter = <&'a Punctuated<GenericParam<S>, Token![S => ,]> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        (&self.params).into_iter()
    }
}

impl<'a, S> core::iter::IntoIterator for &'a TypeDefGenerics<S> {
    type Item = &'a GenericDefParam<S>;
    type IntoIter = <&'a Punctuated<GenericDefParam<S>, Token![S => ,]> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        (&self.params).into_iter()
    }
}

impl<S: Default> From<ImplGenerics<S>> for TypeDefGenerics<S> {
    fn from(value: ImplGenerics<S>) -> Self {
        TypeDefGenerics {
            angle_token: value.angle_token,
            params: value.params.into_iter().map(|param| param.into()).collect(),
            trailing_comma: value.trailing_comma,
        }
    }
}

impl<S: Default> From<ImplGenerics<S>> for TypeGenerics<S> {
    fn from(value: ImplGenerics<S>) -> Self {
        TypeGenerics {
            angle_token: value.angle_token,
            params: value
                .params
                .into_iter()
                .map(|param| match param {
                    GenericParam::Type { ident, .. } => {
                        GenericArgument::Type(Type::Path(crate::TypePath {
                            path: crate::Path {
                                leading_colon: None,
                                segments: core::iter::once(crate::PathSegment {
                                    ident,
                                    arguments: None,
                                })
                                .collect(),
                            },
                        }))
                    }
                    GenericParam::Lifetime { lifetime, .. } => GenericArgument::Lifetime(lifetime),
                    GenericParam::Const { ident, .. } => {
                        GenericArgument::Const(Expr::Path(crate::ExprPath {
                            qself: None,
                            path: crate::Path {
                                leading_colon: None,
                                segments: core::iter::once(crate::PathSegment {
                                    ident,
                                    arguments: None,
                                })
                                .collect(),
                            },
                        }))
                    }
                })
                .collect(),
            trailing_comma: value.trailing_comma,
        }
    }
}
