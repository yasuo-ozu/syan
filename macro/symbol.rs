use proc_macro2::{Span, TokenStream, TokenTree};
use proc_macro_error::abort;
use syn::parse::{Parse, ParseStream};
use syn::*;
use template_quote::quote;

#[derive(Debug)]
pub struct SymbolToken {
    slot: String,
    span: Span,
}

impl Parse for SymbolToken {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(LitInt) {
            let litint = input.parse::<LitInt>().unwrap();
            if !litint.suffix().is_empty() {
                return Err(Error::new(litint.span(), "cannot contain suffix"));
            }
            Ok(SymbolToken {
                slot: litint.base10_digits().to_owned(),
                span: litint.span(),
            })
        } else if input.peek(LitChar) {
            let litchar = input.parse::<LitChar>().unwrap();
            Ok(SymbolToken {
                slot: litchar.value().to_string(),
                span: litchar.span(),
            })
        } else {
            match input.parse::<TokenTree>()? {
                TokenTree::Ident(ident) => Ok(SymbolToken {
                    slot: ident.to_string(),
                    span: ident.span(),
                }),
                TokenTree::Punct(punct) => Ok(SymbolToken {
                    slot: punct.to_string(),
                    span: punct.span(),
                }),
                o => Err(Error::new(o.span(), "bad token")),
            }
        }
    }
}

pub struct SymbolArgs {
    syan_path: Ident,
    _comma: Token![,],
    tokens: Vec<SymbolToken>,
}

impl Parse for SymbolArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let syan_path = input.parse()?;
        let _comma = input.parse()?;

        let mut tokens = Vec::new();
        while !input.is_empty() {
            tokens.push(input.parse()?);
        }

        if tokens.is_empty() {
            return Err(syn::Error::new(input.span(), "Expected at least one token"));
        }

        Ok(SymbolArgs {
            syan_path,
            _comma,
            tokens,
        })
    }
}

/// Maps one character to its `chars::*` type path, or `None` for any character the symbol table does
/// not cover — so the caller can emit a clean spanned error instead of panicking the proc-macro.
fn char_to_type_path(c: char, syan_path: &Ident, span: proc_macro2::Span) -> Option<TokenStream> {
    let chars_path = quote! { #syan_path::symbol::chars };
    Some(match c {
        'a'..='z' | 'A'..='Z' | '0'..='9' => {
            let type_name = format!("_{}", c);
            let type_ident = Ident::new(&type_name, span);
            quote! { #chars_path::#type_ident }
        }
        '_' => quote! { #chars_path::__ },
        '!' => quote! { #chars_path::Not },
        '"' => quote! { #chars_path::Quot },
        '#' => quote! { #chars_path::Pound },
        '$' => quote! { #chars_path::Dollar },
        '%' => quote! { #chars_path::Percnt },
        '&' => quote! { #chars_path::And },
        '\'' => quote! { #chars_path::Apos },
        '(' => quote! { #chars_path::OpenParen },
        ')' => quote! { #chars_path::CloseParen },
        '*' => quote! { #chars_path::Star },
        '+' => quote! { #chars_path::Plus },
        ',' => quote! { #chars_path::Comma },
        '-' => quote! { #chars_path::Minus },
        '.' => quote! { #chars_path::Dot },
        '/' => quote! { #chars_path::Slash },
        ':' => quote! { #chars_path::Colon },
        ';' => quote! { #chars_path::Semi },
        '<' => quote! { #chars_path::Lt },
        '=' => quote! { #chars_path::Eq },
        '>' => quote! { #chars_path::Gt },
        '?' => quote! { #chars_path::Question },
        '@' => quote! { #chars_path::Commat },
        '[' => quote! { #chars_path::OpenBracket },
        ']' => quote! { #chars_path::CloseBracket },
        '\\' => quote! { #chars_path::Backslash },
        '^' => quote! { #chars_path::Caret },
        '`' => quote! { #chars_path::Grave },
        '{' => quote! { #chars_path::OpenBrace },
        '}' => quote! { #chars_path::CloseBrace },
        '|' => quote! { #chars_path::Or },
        '~' => quote! { #chars_path::Tilde },
        ' ' => {
            let space_ident = Ident::new("Space", span);
            quote! { #chars_path::#space_ident }
        }
        _ => return None,
    })
}

/// Creates a Joint type that can handle arbitrarily long character sequences.
fn create_joint_type(char_types: Vec<TokenStream>, syan_path: &Ident) -> TokenStream {
    const MAX_TUPLE_SIZE: usize = 12;

    if char_types.len() <= MAX_TUPLE_SIZE {
        quote! { #syan_path::nested::Joint<(#(#char_types,)*)> }
    } else {
        let chunks: Vec<Vec<TokenStream>> = char_types
            .chunks(MAX_TUPLE_SIZE)
            .map(|chunk| chunk.to_vec())
            .collect();

        let joint_types: Vec<TokenStream> = chunks
            .into_iter()
            .map(|chunk| create_joint_type(chunk, syan_path))
            .collect();

        if joint_types.len() == 1 {
            joint_types[0].clone()
        } else {
            quote! { #syan_path::nested::Joint<(#(#joint_types),*)> }
        }
    }
}

pub fn symbol(args: SymbolArgs) -> TokenStream {
    let syan_path = &args.syan_path;

    let mut char_types = Vec::new();
    for token in &args.tokens {
        for c in token.slot.chars() {
            match char_to_type_path(c, syan_path, token.span) {
                Some(ty) => char_types.push(ty),
                None => abort!(
                    token.span,
                    "symbol! does not support the character {:?} (U+{:04X})",
                    c,
                    c as u32
                ),
            }
        }
    }

    let joint_type = create_joint_type(char_types, syan_path);

    quote! { #syan_path::symbol::Symbol<#joint_type> }
}
