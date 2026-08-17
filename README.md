<p align="center">
  <img src="https://raw.githubusercontent.com/yasuo-ozu/syan/main/syan.png" width="140" alt="syan logo: a token stream feeding a syntax tree, with arrows circling back to the tokens">
</p>

# syan crate [![Latest Version]][crates.io] [![Documentation]][docs.rs]

[Latest Version]: https://img.shields.io/crates/v/syan.svg
[crates.io]: https://crates.io/crates/syan
[Documentation]: https://img.shields.io/docsrs/syan
[docs.rs]: https://docs.rs/syan/latest/syan/

**Declare the syntax tree; the parser is derived from it.** `syan` is a general-purpose parser. It
is generic over the *atom* it reads, so the input can be a `&str`, a `&[u8]`, or a proc-macro
`TokenStream`. Trees carry spans, unparse back to atoms, and may be mutually recursive.

Nothing in the core needs `proc_macro2`. It is one optional feature and one source module. Writing a
proc macro? See [Proc macros](#proc-macros). Otherwise read on.

## Parsing text

A combinator library builds a parser as a *value*. Here is `combine` reading `x = 1`:

```rust,ignore
use combine::parser::char::{char, digit, spaces};
use combine::{many1, Parser};

let int = many1::<String, _, _>(digit()).map(|s| s.parse::<u32>().unwrap());
let mut assign = char('x')
    .skip(spaces())
    .with(char('=').skip(spaces()))
    .with(int);

assert_eq!(assign.parse("x = 1").unwrap().0, 1);
```

The grammar is only implicit, in the order of the `with` and `skip` calls. The result is whatever
the last combinator returned; a named tree means writing one out and mapping onto it. `syan` starts
from the tree instead. **The field order is the grammar:**

```rust
# use syan::parse::Parse;
# use syan::literal::Integer;
# use syan::symbol::Token;
#[derive(Parse)]
struct Assign<S, V> {
    name: Token![S => x],
    eq: Token![S => =],
    value: V,
}

let a: Assign<_, Integer> = Parse::parse("x = 1").unwrap();
assert_eq!(a.value.value, "1");

// Each `Token!` carries the span of what it matched, in the source's own coordinates.
assert_eq!((a.name.span.line, a.name.span.col, a.name.span.loc), (1, 1, 0));
assert_eq!((a.eq.span.line, a.eq.span.col, a.eq.span.loc), (1, 3, 2));
```

You get a named struct, not a tuple, and spans come with it. The span is a type parameter, so one
node serves both text and a `TokenStream`.

Separators between fields are skipped, so `"x=1"` and `"x  =  1"` both parse. When spacing matters,
`#[joint]` demands none and `#[alone]` demands one.

## Combinators

Sequencing is the field order. Everything else is a type you put in a field.

| type | parses |
|---|---|
| `Option<T>` | `T`, or nothing |
| `Vec<T>` | `T` repeated |
| `Punctuated<T, P>` | `T` separated by `P` |
| `GroupParen<T, S>` | `(` `T` `)` — also `GroupBrace`, `GroupBracket` |
| `Unordered<T, U>` | `T` then `U`, or `U` then `T` |
| `Joint<T>` | a sequence with no space between the parts |
| `Attempt<T>` | `T`, rewinding the stream if it fails |

`Token![Span => +]` names a punctuation or keyword token, carrying a span of your source's type.

### Groups

A group takes two fields: one for the delimiters, and one marked `#[group(..)]` that is parsed from
what is *inside* them.

```rust
# use syan::nested::group::GroupParen;
# use syan::nested::Punctuated;
# use syan::parse::Parse;
# use syan::literal::Integer;
# use syan::symbol::Token;
#[derive(Parse)]
struct Call<S> {
    name: Token![S => f],
    paren: GroupParen<(), S>,
    #[group(self.paren)]
    args: Punctuated<Integer, Token![S => ,]>,
}

let call: Call<_> = Parse::parse("f( 1, 2, 3 )").unwrap();
assert_eq!(call.args.len(), 3);
```

The `()` is the holder's own content type. It is empty because the content lives in `args` instead.
Keeping the two apart leaves the content type free.

## Recursive grammars

Types that refer to each other need `#[recurse]` on the enclosing module. Without it the derived
bounds are mutually dependent, so none can be proved and nothing compiles.

```rust
# use syan::parse::{recurse, Parse};
#[recurse]
mod ast {
    use syan::parse::Parse;
    use syan::literal::Integer;
    use syan::symbol::Token;

    #[derive(Parse)]
    pub enum Expr<S> {
        Neg {
            minus: Token![S => -],
            inner: Box<Expr<S>>,
        },
        Lit(Integer),
    }
}

let e: ast::Expr<_> = Parse::parse("- - 1").unwrap();
```

Alternatives are tried in order, and `Neg` recurses through the `Box`. Depth is bounded only by the
call stack. `#[recurse]` routes the cyclic obligations through the
[`decycle`](https://docs.rs/decycle) crate. `#[recurse(structural)]` picks its other engine, which is
faster but narrower.

A cycle can also run through a group. That is what the split form in [Groups](#groups) is for.

## Visitors

Walking a tree is a separate derive. `#[derive(Ast)]` marks a node, `#[subast(..)]` names the other
nodes it can reach, and `visitor!` generates the traversal.

```rust
mod ast {
    use syan::visit::Ast;

    #[derive(Ast)]
    #[subast(crate::ast::Expr)]
    pub enum Expr {
        Lit(u32),
        Neg(Box<Expr>),
    }

    pub mod visit {
        syan::visit::visitor!(super::Expr);
    }
}

fn main() {
    use ast::Expr;
    let mut e = Expr::Neg(Box::new(Expr::Lit(2)));

    let mut nodes = 0;
    e.visit(|_: &Expr| nodes += 1);
    assert_eq!(nodes, 2);

    e.visit_mut(|x: &mut Expr| {
        if let Expr::Lit(n) = x {
            *n *= 10;
        }
    });
    assert!(matches!(&e, Expr::Neg(b) if matches!(**b, Expr::Lit(20))));
}
```

A closure visits one node type. A tuple of closures visits several in one traversal. A struct
implementing the generated `Visit` or `VisitMut` trait gets a method per node type; call
`visit::visit_expr(self, i)` from it to keep descending. `visit_mut` edits in place.

## Proc macros

A `TokenStream` is just another source, behind the default `proc_macro2` feature. Its atom is a
`TokenTree` and its span is a `proc_macro2::Span`. The literal types (`Integer`, `Str`, `Float`, …)
come with it.

This is `syn`'s niche, and the contrast is the same as with combinators. `syn` has you write the
tree *and* the code that walks the input, kept in step by hand.

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

The `Assign` from the top of this file reads a `TokenStream` unchanged. Only the span and the
literal type follow the source. `#[derive(Unparse)]` adds the other direction, which is how a macro
emits its result: a `TokenStream` is an `Emitter`, so a tree writes straight into one.

```rust
# #[cfg(feature = "proc_macro2")] {
# use syan::parse::{Parse, Unparse};
# use syan::literal::Integer;
# use syan::symbol::Token;
#[derive(Parse, Unparse)]
struct Assign<S, V> {
    name: Token![S => x],
    eq: Token![S => =],
    value: V,
}

let ts: proc_macro2::TokenStream = "x = 1".parse().unwrap();
let a: Assign<_, Integer> = Parse::parse(ts).unwrap();
assert_eq!(a.value.value, "1");

let mut out = proc_macro2::TokenStream::new();
a.unparse(&mut out).unwrap();
assert_eq!(out.to_string(), "x = 1");
# }
```

A token source delivers a delimited group as one `TokenTree`, so `#[group(..)]` is handed its
contents directly. `Call` from [Groups](#groups) reads `f(1, 2, 3)` unchanged.

## Errors

A failed parse returns `ParseError<S>`. It is an enum over the *kind* of failure — `Expected`,
`Eof`, `Group`, `Literal`, and so on — carrying a span of your source's type. Nothing is formatted
until you print it.

## Features

- `proc_macro2` (default) — the `TokenStream` source and its literal types. Turn it off and the
  dependency goes away, leaving the text (`&str`, `String`) and byte (`&[u8]`) sources.

## License

MIT
