pub mod group_paren_test {
    use syan::parse::{Parse, Unparse};
    use syan::symbol::Token;
    use type_macro_derive_tricks::macro_derive;
    #[doc(hidden)]
    type __TypeMacroAlias8EHgcCuyQsAW<S> = ::syan::span::WithSpan<
        ::syan::symbol::Symbol<
            ::syan::nested::Joint<(
                ::syan::symbol::chars::_i,
                ::syan::symbol::chars::_n,
                ::syan::symbol::chars::_n,
                ::syan::symbol::chars::_e,
                ::syan::symbol::chars::_r,
            )>,
        >,
        S,
    >;
    // #[fundamental_tys()]
    // #[predicate($syan::symbol::chars::OpenParen:$syan::parse::Parse<$atom>)]
    // #[predicate($syan::symbol::chars::CloseParen:$syan::parse::Parse<$atom>)]
    // #[predicate_parse($atom:::core::clone::Clone)]
    pub struct GroupParenExample<S> {
        pub paren_token: syan::nested::group::GroupParen<(), S>,
        // #[group(self.paren_token)]
        pub inner_value: __TypeMacroAlias8EHgcCuyQsAW<S>,
        // #[ignore_bounds]
        pub next: Option<Box<GroupParenExample<S>>>,
    }
    // #[syan(::syan)]
    #[allow(non_camel_case_types)]
    struct __SyanSubstructOf_paren_token_GroupParenExample_9701004729239255308<S> {
        inner_value: __TypeMacroAlias8EHgcCuyQsAW<S>,
        _syan_phantom: ::core::marker::PhantomData<(S,)>,
    }
    #[automatically_derived]
    impl<S, __SyanMacro_Atom> ::syan::parse::parse::Parse<__SyanMacro_Atom>
        for __SyanSubstructOf_paren_token_GroupParenExample_9701004729239255308<S>
    where
        __SyanMacro_Atom: ::syan::span::Spanned,
        __TypeMacroAlias8EHgcCuyQsAW<S>: ::syan::parse::parse::Parse<__SyanMacro_Atom>,
        ::core::marker::PhantomData<(S,)>: ::syan::parse::parse::Parse<__SyanMacro_Atom>,
    {
        type Error = ::syan::error::ParseError;
        fn parse(
            __syan_stream: impl ::syan::parse::into_parse_stream::IntoParseStream<
                Atom = __SyanMacro_Atom,
            >,
        ) -> ::core::result::Result<Self, Self::Error> {
            let mut __syan_stream = __syan_stream.into_parse_stream();
            let inner_value = ::core::result::Result::map_err(
                <__TypeMacroAlias8EHgcCuyQsAW<S> as ::syan::parse::parse::Parse<
                    __SyanMacro_Atom,
                >>::parse(&mut __syan_stream),
                |err| <_ as ::syan::error::Error>::into_parse_error(err),
            )?;
            let _syan_phantom = ::core::result::Result::map_err(
                <::core::marker::PhantomData<(S,)> as ::syan::parse::parse::Parse<
                    __SyanMacro_Atom,
                >>::parse(&mut __syan_stream),
                |err| <_ as ::syan::error::Error>::into_parse_error(err),
            )?;
            ::core::result::Result::Ok(
                __SyanSubstructOf_paren_token_GroupParenExample_9701004729239255308 {
                    inner_value,
                    _syan_phantom,
                },
            )
        }
    }
    #[automatically_derived]
    impl<S, __SyanMacro_Atom> ::syan::parse::parse::Parse<__SyanMacro_Atom> for GroupParenExample<S>
    where
        __SyanMacro_Atom: ::syan::span::Spanned,
        ::syan::symbol::chars::OpenParen: ::syan::parse::Parse<__SyanMacro_Atom>,
        ::syan::symbol::chars::CloseParen: ::syan::parse::Parse<__SyanMacro_Atom>,
        __SyanMacro_Atom: ::core::clone::Clone,
        syan::nested::group::GroupParen<(), S>: ::syan::parse::parse::Parse<__SyanMacro_Atom>,
        syan::nested::group::GroupParen<(), S>: ::syan::nested::group::EmptyGroup,
        __SyanSubstructOf_paren_token_GroupParenExample_9701004729239255308<S>:
            Parse<__SyanMacro_Atom>,
    {
        type Error = ::syan::error::ParseError;
        fn parse(
            __syan_stream: impl ::syan::parse::into_parse_stream::IntoParseStream<
                Atom = __SyanMacro_Atom,
            >,
        ) -> ::core::result::Result<Self, Self::Error> {
            let mut __syan_stream = __syan_stream.into_parse_stream();
            let paren_token: <syan::nested::group::GroupParen<
                (),
                S,
            > as ::syan::nested::group::EmptyGroup>::Fill<
                __SyanSubstructOf_paren_token_GroupParenExample_9701004729239255308<S>,
            > = ::core::result::Result::map_err(
                <<syan::nested::group::GroupParen<
                    (),
                    S,
                > as ::syan::nested::group::EmptyGroup>::Fill<
                    __SyanSubstructOf_paren_token_GroupParenExample_9701004729239255308<
                        S,
                    >,
                > as ::syan::parse::parse::Parse<
                    __SyanMacro_Atom,
                >>::parse(&mut __syan_stream),
                |err| <_ as ::syan::error::Error>::into_parse_error(err),
            )?;
            let (
                __SyanSubstructOf_paren_token_GroupParenExample_9701004729239255308 {
                    inner_value,
                    _syan_phantom: _,
                },
                paren_token,
            ) = ::syan::nested::group::EmptyGroup::unfill(paren_token);
            let next = ::core::result::Result::map_err(
                <Option<Box<GroupParenExample<S>>> as ::syan::parse::parse::Parse<
                    __SyanMacro_Atom,
                >>::parse(&mut __syan_stream),
                |err| <_ as ::syan::error::Error>::into_parse_error(err),
            )?;
            ::core::result::Result::Ok(GroupParenExample {
                paren_token,
                inner_value,
                next,
            })
        }
    }
    // #[syan(::syan)]
    #[allow(non_camel_case_types)]
    struct __SyanSubstructOf_paren_token_GroupParenExample_7569091820825175015<
        'syan_substruct_ref,
        S,
    > {
        inner_value: &'syan_substruct_ref __TypeMacroAlias8EHgcCuyQsAW<S>,
        _syan_phantom: ::core::marker::PhantomData<(S,)>,
    }
    #[automatically_derived]
    impl<'syan_substruct_ref, S, __SyanMacro_Atom> ::syan::parse::unparse::Unparse<__SyanMacro_Atom>
        for __SyanSubstructOf_paren_token_GroupParenExample_7569091820825175015<
            'syan_substruct_ref,
            S,
        >
    where
        &'syan_substruct_ref __TypeMacroAlias8EHgcCuyQsAW<S>:
            ::syan::parse::unparse::Unparse<__SyanMacro_Atom>,
        ::core::marker::PhantomData<(S,)>: ::syan::parse::unparse::Unparse<__SyanMacro_Atom>,
    {
        fn unparse<__Syan_Emitter: ::syan::parse::unparse::Emitter<__SyanMacro_Atom>>(
            &self,
            __syan_sink: &mut __Syan_Emitter,
        ) -> ::core::result::Result<(), __Syan_Emitter::Error> {
            let __SyanSubstructOf_paren_token_GroupParenExample_7569091820825175015 {
                inner_value,
                _syan_phantom,
            } = self;
            ::syan::parse::unparse::Unparse::unparse(&inner_value, __syan_sink)?;
            ::syan::parse::unparse::Unparse::unparse(&_syan_phantom, __syan_sink)?;
            ::core::result::Result::Ok(())
        }
    }
    #[automatically_derived]
    impl<S, __SyanMacro_Atom> ::syan::parse::unparse::Unparse<__SyanMacro_Atom> for GroupParenExample<S>
    where
        ::syan::symbol::chars::OpenParen: ::syan::parse::Parse<__SyanMacro_Atom>,
        ::syan::symbol::chars::CloseParen: ::syan::parse::Parse<__SyanMacro_Atom>,
        syan::nested::group::GroupParen<(), S>: ::syan::parse::unparse::Unparse<__SyanMacro_Atom>,
        syan::nested::group::GroupParen<(), S>:
            ::syan::nested::group::EmptyGroup + ::core::clone::Clone,
        for<'syan_substruct_ref> <syan::nested::group::GroupParen<(), S> as ::syan::nested::group::EmptyGroup>::Fill<
            __SyanSubstructOf_paren_token_GroupParenExample_7569091820825175015<
                'syan_substruct_ref,
                S,
            >,
        >: ::syan::parse::unparse::Unparse<__SyanMacro_Atom>,
    {
        fn unparse<__Syan_Emitter: ::syan::parse::unparse::Emitter<__SyanMacro_Atom>>(
            &self,
            __syan_sink: &mut __Syan_Emitter,
        ) -> ::core::result::Result<(), __Syan_Emitter::Error> {
            let GroupParenExample {
                paren_token,
                inner_value,
                next,
            } = self;
            use ::syan::nested::group::EmptyGroup as _;
            let paren_token =
                <syan::nested::group::GroupParen<(), S> as ::syan::nested::group::EmptyGroup>::fill(
                    ::core::clone::Clone::clone(paren_token),
                    __SyanSubstructOf_paren_token_GroupParenExample_7569091820825175015 {
                        inner_value,
                        _syan_phantom: ::core::marker::PhantomData,
                    },
                );
            ::syan::parse::unparse::Unparse::unparse(&paren_token, __syan_sink)?;
            ::syan::parse::unparse::Unparse::unparse(&next, __syan_sink)?;
            ::core::result::Result::Ok(())
        }
    }
}
