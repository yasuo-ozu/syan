use proc_macro2::{Punct, TokenStream};
use syn::parse::{Parse, ParseStream};
use syn::*;
use template_quote::quote;

#[derive(Debug)]
pub enum SymbolToken {
    Ident(Ident),
    Punct(Punct),
    LitInt(LitInt),
    LitChar(LitChar),
}

impl Parse for SymbolToken {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(Ident) {
            Ok(SymbolToken::Ident(input.parse()?))
        } else if input.peek(LitInt) {
            Ok(SymbolToken::LitInt(input.parse()?))
        } else if input.peek(LitChar) {
            Ok(SymbolToken::LitChar(input.parse()?))
        } else {
            // Try to parse as Punct - this handles various punctuation
            Ok(SymbolToken::Punct(input.parse()?))
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

fn char_to_type_path(c: char, syan_path: &Ident, span: proc_macro2::Span) -> TokenStream {
    let chars_path = quote! { #syan_path::symbol::chars };
    match c {
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
            // Space character - we'll represent it as a special Space type if it exists,
            // or skip it for now
            let space_ident = Ident::new("Space", span);
            quote! { #chars_path::#space_ident }
        }
        _ => panic!("Unsupported character: {} (code: {})", c, c as u32),
    }
}

/// Creates a Joint type that can handle arbitrarily long character sequences.
fn create_joint_type(char_types: Vec<TokenStream>, syan_path: &Ident) -> TokenStream {
    const MAX_TUPLE_SIZE: usize = 12;

    if char_types.len() <= MAX_TUPLE_SIZE {
        quote! { #syan_path::nested::Joint<(#(#char_types,)*)> }
    } else {
        // Recursive case: split into chunks
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

fn token_to_char_types(token: &SymbolToken, syan_path: &Ident) -> Vec<TokenStream> {
    match token {
        SymbolToken::Ident(ident) => {
            let ident_str = ident.to_string();
            ident_str
                .chars()
                .map(|c| char_to_type_path(c, syan_path, ident.span()))
                .collect()
        }
        SymbolToken::Punct(punct) => {
            let punct_char = punct.as_char();
            vec![char_to_type_path(punct_char, syan_path, punct.span())]
        }
        SymbolToken::LitInt(lit_int) => {
            // Convert to decimal string without suffixes
            match lit_int.base10_parse::<u64>() {
                Ok(value) => {
                    let decimal_str = value.to_string();
                    decimal_str
                        .chars()
                        .map(|c| char_to_type_path(c, syan_path, lit_int.span()))
                        .collect()
                }
                Err(_) => {
                    // Fallback: use the token string directly, stripping suffixes
                    let token_str = lit_int.to_string();
                    let clean_str =
                        token_str.trim_end_matches(|c: char| c.is_ascii_alphabetic() || c == '_');
                    clean_str
                        .chars()
                        .map(|c| char_to_type_path(c, syan_path, lit_int.span()))
                        .collect()
                }
            }
        }
        SymbolToken::LitChar(lit_char) => {
            let char_value = lit_char.value();
            vec![char_to_type_path(char_value, syan_path, lit_char.span())]
        }
    }
}

pub fn symbol(args: SymbolArgs) -> TokenStream {
    let syan_path = &args.syan_path;

    // Convert all tokens to character types
    let mut char_types = Vec::new();
    for token in &args.tokens {
        char_types.extend(token_to_char_types(token, syan_path));
    }

    // Generate the Joint type using recursive algorithm
    let joint_type = create_joint_type(char_types, syan_path);

    // Wrap in Symbol<T>
    quote! { #syan_path::symbol::Symbol<#joint_type> }
}
