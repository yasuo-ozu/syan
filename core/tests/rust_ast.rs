#[syan::parse::recurse]
pub mod rust_ast {
    use syan::nested::group::{GroupBrace, GroupParen};
    use syan::parse::{Parse, Unparse};
    use syan::source::proc_macro2::literal::Integer;
    use syan::symbol::Token;

    // Simple identifier
    #[derive(Parse, Unparse)]
    pub struct Ident {
        pub name: Integer,
    }

    // Item enum with all the variants
    #[derive(Parse, Unparse)]
    pub enum Item<S> {
        Fn(ItemFn<S>),
        Mod(ItemMod<S>),
        Trait(ItemTrait<S>),
        Impl(ItemImpl<S>),
    }

    // Simple function with groups
    #[derive(Parse, Unparse)]
    pub struct ItemFn<S> {
        pub fn_token: Token![S => fn],
        pub name: Ident,
        pub paren_group: GroupParen<(), S>,
        pub body: Block<S>,
    }

    // Simple module
    #[derive(Parse, Unparse)]
    pub struct ItemMod<S> {
        pub mod_token: Token![S => mod],
        pub name: Ident,
        pub brace_group: GroupBrace<(), S>,
        #[group(self.brace_group)]
        pub content: Integer,
    }

    // Simple trait
    #[derive(Parse, Unparse)]
    pub struct ItemTrait<S> {
        pub trait_token: Token![S => trait],
        pub name: Ident,
        pub brace_group: GroupBrace<(), S>,
        #[group(self.brace_group)]
        pub content: Integer,
    }

    // Simple impl
    #[derive(Parse, Unparse)]
    pub struct ItemImpl<S> {
        pub impl_token: Token![S => impl],
        pub name: Ident,
        pub for_token: Token![S => for],
        pub target: Ident,
        pub brace_group: GroupBrace<(), S>,
        #[group(self.brace_group)]
        pub content: Integer,
    }

    // Simple expression enum
    #[derive(Parse, Unparse)]
    pub enum Expr<S> {
        Ident(Ident),
        Literal(Integer),
        Binary {
            left: Box<Expr<S>>,
            op: Token![S => +],
            right: Box<Expr<S>>,
        },
        Block(Block<S>),
    }

    // Simple statement enum
    #[derive(Parse, Unparse)]
    pub enum Stmt<S> {
        Let {
            let_token: Token![S => let],
            name: Ident,
            eq_token: Token![S => =],
            value: Box<Expr<S>>,
            semicolon: Token![S => ;],
        },
        Expr {
            expr: Box<Expr<S>>,
            semicolon: Option<Token![S => ;]>,
        },
    }

    // Simple block with statements
    #[derive(Parse, Unparse)]
    pub struct Block<S> {
        pub brace_group: GroupBrace<(), S>,
        #[group(self.brace_group)]
        pub stmts: Vec<Stmt<S>>,
    }
}

// Tests to verify the AST parsing works
#[cfg(test)]
mod tests {
    use super::rust_ast::*;
    use syan::parse::Parse;
    use template_quote::quote;

    #[test]
    fn test_simple_function() {
        let tokens = quote! {
            fn 42() {
                1;
            }
        };
        let func: ItemFn<_> = Parse::parse(tokens).unwrap();
        assert_eq!(func.name.name.value, "42");
    }

    #[test]
    fn test_simple_module() {
        let tokens = quote! {
            mod 100 {
                200
            }
        };
        let module: ItemMod<_> = Parse::parse(tokens).unwrap();
        assert_eq!(module.name.name.value, "100");
        assert_eq!(module.content.value, "200");
    }

    #[test]
    fn test_simple_block() {
        let tokens = quote! {
            {
                let 1 = 2;
            }
        };
        let block: Block<_> = Parse::parse(tokens).unwrap();
        assert_eq!(block.stmts.len(), 1);
    }

    #[test]
    fn test_trait_item() {
        let tokens = quote! {
            trait 123 {
                456
            }
        };
        let trait_item: ItemTrait<_> = Parse::parse(tokens).unwrap();
        assert_eq!(trait_item.name.name.value, "123");
        assert_eq!(trait_item.content.value, "456");
    }

    #[test]
    fn test_impl_item() {
        let tokens = quote! {
            impl 111 for 222 {
                333
            }
        };
        let impl_item: ItemImpl<_> = Parse::parse(tokens).unwrap();
        assert_eq!(impl_item.name.name.value, "111");
        assert_eq!(impl_item.target.name.value, "222");
        assert_eq!(impl_item.content.value, "333");
    }

    #[test]
    fn test_ident() {
        let tokens = quote! { 999 };
        let ident: Ident = Parse::parse(tokens).unwrap();
        assert_eq!(ident.name.value, "999");
    }

    #[test]
    fn test_item_function() {
        let tokens = quote! {
            fn 42() {
                1;
            }
        };
        let item: Item<_> = Parse::parse(tokens).unwrap();
        match item {
            Item::Fn(func) => assert_eq!(func.name.name.value, "42"),
            _ => panic!("Expected function item"),
        }
    }

    #[test]
    fn test_expression_literal() {
        let tokens = quote! { 123 };
        let expr: Expr<_> = Parse::parse(tokens).unwrap();
        match expr {
            Expr::Literal(lit) => assert_eq!(lit.value, "123"),
            _ => panic!("Expected literal expression"),
        }
    }

    #[test]
    fn test_expression_block() {
        let tokens = quote! {
            {
                let 1 = 2;
            }
        };
        let expr: Expr<_> = Parse::parse(tokens).unwrap();
        match expr {
            Expr::Block(block) => assert_eq!(block.stmts.len(), 1),
            _ => panic!("Expected block expression"),
        }
    }
}
