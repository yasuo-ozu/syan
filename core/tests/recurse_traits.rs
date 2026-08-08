//! Group-free vs group-ful `#[recurse]` Unparse/Spanned (unbounded via re-entry) + the
//! `#[ignore_bounds]` primitive.
#![allow(dead_code)]

// Group-free `#[recurse]` cycles derive Unparse/Spanned DIRECTLY on the natural type (unbounded).
mod unparse_spanned {
    use core::marker::PhantomData;
    use syan::parse::{recurse, Parse, Unparse};
    use template_quote::quote;

    #[recurse]
    mod pu {
        use core::marker::PhantomData;
        use syan::parse::{Parse, Unparse};
        use syan::source::proc_macro2::literal::Integer;

        // A list `1 2 3` → Cons(1, Cons(2, Cons(3, Nil))); group-free, all-`Integer` leaves.
        #[derive(Parse, Unparse)]
        pub enum Expr<S> {
            Cons {
                head: Integer,
                tail: Box<Expr<S>>,
            },
            Nil(PhantomData<S>),
        }
    }

    #[test]
    fn unparse_roundtrip_with_type_param() {
        let toks = quote! { 1 2 3 };
        let e: pu::Expr<()> = Parse::parse(toks).unwrap();
        let mut out = Vec::<proc_macro2::TokenTree>::new();
        e.unparse(&mut (&mut out)).unwrap();
        assert_eq!(out.len(), 3, "the three integer literals, round-tripped");
    }

    #[test]
    fn parse_unbounded_depth() {
        use pu::Expr;
        // Recursion re-enters through decycle's un-ranked delegating impl at full height, so a stream of
        // 200 integers parses into a 200-deep `Cons` list with no truncation.
        let mut ts = proc_macro2::TokenStream::new();
        for _ in 0..200 {
            ts.extend(quote! { 1 });
        }
        let mut e: Expr<()> = Parse::parse(ts).unwrap();
        let mut depth = 0usize;
        while let Expr::Cons { tail, .. } = e {
            depth += 1;
            e = *tail;
        }
        assert_eq!(depth, 200, "all 200 levels parsed — `recurse_level` is not a depth ceiling");
    }

    #[test]
    fn unparse_unbounded_depth() {
        use pu::Expr;
        use syan::source::proc_macro2::literal::Integer;
        // `Unparse` has no backtracking and re-enters un-ranked, so a depth-5000 list unparses.
        let mut e: Expr<()> = Expr::Nil(PhantomData);
        for _ in 0..5000 {
            e = Expr::Cons {
                head: Integer { value: "1".into(), suffix: None },
                tail: Box::new(e),
            };
        }
        let mut out = Vec::<proc_macro2::TokenTree>::new();
        e.unparse(&mut (&mut out)).unwrap();
        assert_eq!(out.len(), 5000, "five thousand `1`s — depth far past the old limit");
    }

    #[recurse]
    mod sp {
        use syan::span::{Spanned, WithSpan};
        use syan::visit::Ast;

        #[derive(Ast, Spanned)]
        #[subast()]
        pub enum Expr<S: syan::span::Span> {
            Node {
                head: WithSpan<u32, S>,
                child: Box<Expr<S>>,
            },
            Leaf(WithSpan<u64, S>),
        }
    }

    #[test]
    fn spanned_with_type_param() {
        use sp::Expr;
        use syan::span::{Spanned, WithSpan};
        let tree: Expr<()> = Expr::Node {
            head: WithSpan { slot: 0, span: () },
            child: Box::new(Expr::Node {
                head: WithSpan { slot: 0, span: () },
                child: Box::new(Expr::Leaf(WithSpan { slot: 0, span: () })),
            }),
        };
        let _s: () = tree.span();
    }

    // MULTI-TYPE cycle: members' leaf bounds differ (`Expr` has an `Integer` leaf, `Stmt` does not), so
    // `#[recurse]` injects the *union* of all members' leaf bounds as `#[predicate_unparse(…)]` on every
    // member — so each impl can build/unparse its siblings, DIRECT on the natural type (unbounded).
    #[recurse]
    mod mt {
        use core::marker::PhantomData;
        use syan::parse::{Parse, Unparse};
        use syan::source::proc_macro2::literal::Integer;

        #[derive(Parse, Unparse)]
        pub enum Expr<S> {
            Wrap(Box<Stmt<S>>),
            Lit(Integer, PhantomData<S>),
        }

        #[derive(Parse, Unparse)]
        pub enum Stmt<S> {
            Wrap(Box<Expr<S>>),
            Nil(PhantomData<S>),
        }
    }

    #[test]
    fn multi_type_unparse_direct_unbounded() {
        use core::marker::PhantomData;
        use mt::{Expr, Stmt};
        use syan::source::proc_macro2::literal::Integer;
        let tree: Expr<()> = Expr::Wrap(Box::new(Stmt::Wrap(Box::new(Expr::Lit(
            Integer { value: "7".into(), suffix: None },
            PhantomData,
        )))));
        let mut out = Vec::<proc_macro2::TokenTree>::new();
        tree.unparse(&mut (&mut out)).unwrap();
        assert_eq!(out.len(), 1, "the `7` literal (Stmt::Wrap/Nil emit nothing)");
        let _n: Stmt<()> = Stmt::Nil(PhantomData);

        // A depth-2000 alternating tree unparses.
        let mut e: Expr<()> = Expr::Lit(Integer { value: "1".into(), suffix: None }, PhantomData);
        for _ in 0..2000 {
            e = Expr::Wrap(Box::new(Stmt::Wrap(Box::new(e))));
        }
        let mut deep = Vec::<proc_macro2::TokenTree>::new();
        e.unparse(&mut (&mut deep)).unwrap();
        assert_eq!(deep.len(), 1, "deep multi-type tree round-trips (direct → unbounded)");
    }

    // MULTI-TYPE cycle: DIRECT Spanned via the leaf-bound union (`S: Span`).
    #[recurse]
    mod mts {
        use syan::span::{Span, Spanned, WithSpan};
        use syan::visit::Ast;

        #[derive(Ast, Spanned)]
        #[subast(crate::unparse_spanned::mts::Stmt)]
        pub enum Expr<S: Span> {
            Wrap(Box<Stmt<S>>),
            Leaf(WithSpan<u32, S>),
        }

        #[derive(Ast, Spanned)]
        #[subast(crate::unparse_spanned::mts::Expr)]
        pub enum Stmt<S: Span> {
            Wrap(Box<Expr<S>>),
            Tag(WithSpan<u8, S>),
        }
    }

    #[test]
    fn multi_type_spanned_direct_unbounded() {
        use mts::{Expr, Stmt};
        use syan::span::{Spanned, WithSpan};
        let tree: Expr<()> = Expr::Wrap(Box::new(Stmt::Wrap(Box::new(Expr::Leaf(WithSpan {
            slot: 0,
            span: (),
        })))));
        let _s: () = tree.span();
    }
}

// Group-ful `#[recurse]` cycles: the group is entered via `GroupShape`/`GroupUnparse`, whose content
// type is a METHOD generic, so the obligation is projection-free and the cycle breaks normally.
mod group_ful {
    use syan::parse::{recurse, Parse, Unparse};
    use template_quote::quote;

    #[recurse]
    mod up {
        use syan::nested::group::GroupBrace;
        use syan::parse::{Parse, Unparse};
        use syan::source::proc_macro2::literal::Integer;

        // A brace-delimited list of integer literals, recursive in `inner`.
        #[derive(Parse, Unparse)]
        pub enum Expr<S> {
            Lit(Integer),
            Block {
                brace: GroupBrace<(), S>,
                #[group(self.brace)]
                inner: Vec<Expr<S>>,
            },
        }
    }

    #[test]
    fn group_ful_unparse_round_trips_to_token_group() {
        // `{ 1 2 }` unparses back as ONE `TokenTree::Group` token (the brace group).
        let e: up::Expr<_> = Parse::parse(quote! { { 1 2 } }).unwrap();
        let mut out = Vec::<proc_macro2::TokenTree>::new();
        e.unparse(&mut (&mut out)).unwrap();
        assert_eq!(out.len(), 1, "the whole expression is a single brace `TokenTree::Group`");
        assert!(matches!(out[0], proc_macro2::TokenTree::Group(_)));
        assert_eq!(out[0].to_string(), "{ 1 2 }");
    }

    #[test]
    fn group_ful_unparse_is_unbounded() {
        // A 60-deep `{ { … 1 … } }` round-trips in full.
        let mut src = quote! { 1 };
        for _ in 0..60 {
            src = quote! { { #src } };
        }
        let e: up::Expr<_> = Parse::parse(src.clone()).unwrap();
        let mut out = Vec::<proc_macro2::TokenTree>::new();
        e.unparse(&mut (&mut out)).unwrap();
        assert_eq!(
            out.into_iter().collect::<proc_macro2::TokenStream>().to_string(),
            src.to_string(),
            "deep group-ful tree round-trips (Unparse past the old depth limit)",
        );
    }

    #[recurse]
    mod sp {
        use syan::nested::group::GroupBrace;
        use syan::span::{Span, Spanned, WithSpan};
        use syan::visit::Ast;

        #[derive(Ast, Spanned)]
        #[subast()]
        pub enum Expr<S: Span> {
            Atom(WithSpan<u32, S>),
            Block {
                brace: GroupBrace<(), S>,
                #[group(self.brace)]
                inner: Vec<Expr<S>>,
            },
        }
    }

    #[test]
    fn group_ful_spanned_folds_delimiters() {
        use sp::Expr;
        use syan::nested::group::Group;
        use syan::span::{Spanned, WithSpan};
        // `.span()` folds the group's delimiter spans + the leaf span — the empty `()` slot needs no
        // `Spanned` impl (its span comes from the delimiters).
        let brace = Group { open: Default::default(), slot: (), close: Default::default() };
        let tree: Expr<()> = Expr::Block {
            brace,
            inner: vec![Expr::Atom(WithSpan { slot: 7, span: () })],
        };
        let _s: () = tree.span();

        // Unbounded: a depth-2000 hand-built tree still folds its span (re-entry is through the
        // per level; no `Root: Clone`).
        let mut deep: Expr<()> = Expr::Atom(WithSpan { slot: 7, span: () });
        for _ in 0..2000 {
            let brace = Group { open: Default::default(), slot: (), close: Default::default() };
            deep = Expr::Block { brace, inner: vec![deep] };
        }
        let _s: () = deep.span();
    }

    // Backtracking must rewind a DEEP recursive parse, not just recurse forward. Because every level
    // re-enters through the delegating impl, a late failure has many nested `dup()` frames to unwind.
    // Two outer variants share the identical deep `( … )` spine and differ only in a trailing token;
    // the input is
    // crafted so the first variant's spine parses in full and only THEN fails on the trailing token,
    // forcing the outer `dup()` to rewind the entire re-entered parse before the second variant retries.
    #[recurse]
    mod df {
        use syan::nested::group::GroupParen;
        use syan::parse::{Parse, Unparse};
        use syan::source::proc_macro2::literal::Integer;

        #[derive(Parse, Unparse)]
        pub enum Expr<S> {
            Paren {
                paren: GroupParen<(), S>,
                #[group(self.paren)]
                inner: Box<Expr<S>>,
            },
            Lit(Integer),
        }
    }

    #[type_macro_derive_tricks::macro_derive(Parse, Unparse)]
    enum Top<S> {
        // Tried first (declaration order) — matches the whole spine, then fails on the trailing token.
        Bang {
            e: df::Expr<S>,
            bang: syan::symbol::Token![S => !],
        },
        // Field names differ from `Bang` (no shared-prefix dedup), so this re-parses the spine from
        // scratch on retry — exactly the rewound backtrack under test.
        Question {
            e2: df::Expr<S>,
            quest: syan::symbol::Token![S => ?],
        },
    }

    #[test]
    fn deep_backtrack_rewinds_past_reentry_boundaries() {
        // Far past `DEFAULT_RECURSION_DEPTH` (4).
        const N: usize = 120;
        let mut spine = quote! { 1 };
        for _ in 0..N {
            spine = quote! { ( #spine ) };
        }
        let full = quote! { #spine ? };
        let top: Top<_> = Parse::parse(full.clone())
            .expect("Bang fails on the trailing `?`; Question must succeed after the rewind");
        assert!(
            matches!(top, Top::Question { .. }),
            "Bang's spine must fully parse then fail on the trailing token, backtracking to Question"
        );
        let mut out = Vec::<proc_macro2::TokenTree>::new();
        top.unparse(&mut (&mut out)).unwrap();
        assert_eq!(
            out.into_iter().collect::<proc_macro2::TokenStream>().to_string(),
            full.to_string(),
            "round-trips after the deep backtrack",
        );
    }
}

// `#[ignore_bounds]` drops the synthesized `field_ty: Trait` where-bound so a mutually-recursive pair's
// Unparse derive carries only leaf bounds (no E0275 where-cycle); child calls resolve coinductively.
mod ignore_bounds {
    use core::marker::PhantomData;
    use syan::parse::Unparse;

    #[derive(Unparse)]
    pub enum Expr<S> {
        Lit(::syan::source::proc_macro2::literal::Integer, PhantomData<S>),
        Nest {
            #[ignore_bounds]
            inner: Box<Stmt<S>>,
        },
    }

    #[derive(Unparse)]
    pub enum Stmt<S> {
        One(::syan::source::proc_macro2::literal::Integer, PhantomData<S>),
        Two {
            #[ignore_bounds]
            e: Box<Expr<S>>,
        },
    }

    #[test]
    fn recursive_unparse_compiles_with_leaf_only_bounds() {
        use syan::source::proc_macro2::literal::Integer;
        // A tree deeper than any fixed bound — natural recursion, no depth limit.
        let deep: Expr<proc_macro2::TokenTree> = Expr::Nest {
            inner: Box::new(Stmt::Two {
                e: Box::new(Expr::Nest {
                    inner: Box::new(Stmt::One(
                        Integer { value: "7".into(), suffix: None },
                        PhantomData,
                    )),
                }),
            }),
        };
        let mut out = Vec::<proc_macro2::TokenTree>::new();
        deep.unparse(&mut (&mut out)).unwrap();
        assert_eq!(out.len(), 1, "the single `7` literal at the bottom of the tree");
    }
}
