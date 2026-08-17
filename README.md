<p align="center">
  <img src="https://raw.githubusercontent.com/yasuo-ozu/syan/main/syan.png" width="140" alt="syan logo: a token stream feeding a syntax tree, with arrows circling back to the tokens">
</p>

# syan crate [![Latest Version]][crates.io] [![Documentation]][docs.rs]

[Latest Version]: https://img.shields.io/crates/v/syan.svg
[crates.io]: https://crates.io/crates/syan
[Documentation]: https://img.shields.io/docsrs/syan
[docs.rs]: https://docs.rs/syan/latest/syan/

**Declare the syntax tree; the parser is derived from it.** `syan` is a parsing toolkit in the shape
of `syn`, with two differences: the grammar is the type definition rather than hand-written parsing
code, and it is generic over the *atom* it consumes, so one grammar reads `char`s from a `String` or
`TokenTree`s from a `TokenStream`. Trees round-trip back to atoms, carry spans, and may be mutually
recursive.

Writing a parser with `syn` means writing the syntax tree *and* the code that walks the input to
build it. The two have to be kept in step by hand, the parsing code is imperative even though the
grammar it encodes is declarative, and the whole thing only ever reads a `proc_macro2::TokenStream`.

```rust,ignore
// syn: the tree, and then a parser for it, written separately.
syn::custom_keyword!(x);

struct Assign { name: x, eq: Token![=], value: LitInt }

impl Parse for Assign {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Assign {
            name: input.parse()?,
            eq: input.parse()?,
            value: input.parse()?,
        })
    }
}
```

`syan` removes the second half: you declare the tree and derive the parser from it. The field order
*is* the grammar.

```rust
# #[cfg(feature = "proc_macro2")] {
# use syan::parse::Parse;
# use syan::source::proc_macro2::literal::Integer;
# use syan::source::proc_macro2::Span;
# use syan::symbol::{Symbol, Token};
#[derive(Parse)]
struct Assign {
    name: Symbol!(x),
    eq: Token![Span => =],
    value: Integer,
}

let ts: proc_macro2::TokenStream = "x = 1".parse().unwrap();
let a: Assign = Parse::parse(ts).unwrap();
assert_eq!(a.value.value, "1");
# }
```

Both blocks parse `x = 1`. `syan` is also generic over the *atom* it consumes, so the same grammar
reads `char`s from a `String` as readily as `TokenTree`s from a `TokenStream`. `#[derive(Unparse)]`
gives you the reverse direction — the tree back to atoms — and `#[derive(Spanned)]` reports where a
node came from.

## Combinators

Sequencing is the field order; everything else is a type you put in a field.

| type | parses |
|---|---|
| `Option<T>` | `T`, or nothing |
| `Vec<T>` | `T` repeated |
| `Punctuated<T, P>` | `T` separated by `P` |
| `GroupParen<T, S>` | `(` `T` `)` — also `GroupBrace`, `GroupBracket` |
| `Unordered<T, U>` | `T` then `U`, or `U` then `T` |
| `Joint<T>` | a sequence with no space between the parts |
| `Attempt<T>` | `T`, rewinding the stream if it fails |

`Token![Span => +]` names a punctuation or keyword token carrying a span; `Symbol!(name)` names one
without.

### Groups

A group takes two fields: one holding the delimiters, and one — marked `#[group(..)]` — parsed from
what is *inside* them. Splitting it this way is what lets the content type stay free, so a group can
sit on a recursion cycle.

```rust
# #[cfg(feature = "proc_macro2")] {
# use syan::nested::group::GroupParen;
# use syan::nested::Punctuated;
# use syan::parse::Parse;
# use syan::source::proc_macro2::literal::Integer;
# use syan::source::proc_macro2::Span;
# use syan::symbol::{Symbol, Token};
#[derive(Parse)]
struct Call {
    name: Symbol!(f),
    paren: GroupParen<(), Span>,
    #[group(self.paren)]
    args: Punctuated<Integer, Token![Span => ,]>,
}

let ts: proc_macro2::TokenStream = "f(1, 2, 3)".parse().unwrap();
let call: Call = Parse::parse(ts).unwrap();
assert_eq!(call.args.len(), 3);
# }
```

The `()` in `GroupParen<(), Span>` is the holder's own content type — empty, because the content is
the `args` field instead.

## Recursive grammars

A grammar whose types refer to each other needs `#[recurse]` on the enclosing module. Without it the
derived bounds are mutually dependent, so none of them can be proved and nothing compiles.

```rust
# #[cfg(feature = "proc_macro2")] {
# use syan::parse::{recurse, Parse};
#[recurse]
mod ast {
    use syan::nested::group::GroupBrace;
    use syan::parse::Parse;
    use syan::source::proc_macro2::literal::Integer;
    use syan::source::proc_macro2::Span;

    #[derive(Parse)]
    pub enum Expr {
        Lit(Integer),
        Block {
            brace: GroupBrace<(), Span>,
            #[group(self.brace)]
            inner: Vec<Expr>,
        },
    }
}

let ts: proc_macro2::TokenStream = "{ 1 }".parse().unwrap();
let e: ast::Expr = Parse::parse(ts).unwrap();
# }
```

Depth is bounded only by the call stack. `#[recurse]` routes the cyclic obligations through the
[`decycle`](https://docs.rs/decycle) crate; `#[recurse(structural)]` selects its other engine, which
is faster but narrower in scope.

The `#[group(self.brace)]` here is the same pattern as above, now on a cycle: `inner` recurses back
into `Expr`, and the group boundary is what stops it running away.

## Errors

A failed parse returns `ParseError<S>`, an enum over the *kind* of failure — `Expected`, `Eof`,
`Group`, `Literal`, and so on — carrying a span of your source's own type. Nothing is formatted until
you print it.

## Features

- `proc_macro2` (default) — the `TokenStream` source and its literal types. Turn it off to parse
  only from `String`.

## License

MIT
