// Parses `proc_macro2` tokens throughout; the whole suite is skipped without the optional
// dependency that provides them.
#![cfg(feature = "proc_macro2")]

//! `#[seq]`/`#[opt]` structural-edit views over fields that are ALSO `#[group(...)]` parse groups: the
//! grouped `Vec`/`Option` is edited in place through its `SeqView`/`OptView`, and the group delimiters
//! survive an `Unparse` round-trip. Exercises `#[group]` + `#[seq]`/`#[opt]` together.
#![allow(dead_code)]

use syan::nested::group::{GroupBrace, GroupParen};
use syan::parse::{Parse, Unparse};
use syan::source::proc_macro2::literal::Integer;
use syan::symbol::Token;
use syan::visit::{Ast, OptView, SeqView};
use template_quote::quote;
use type_macro_derive_tricks::macro_derive;

/// The span type when parsing from a `proc_macro2` token stream.
type Sp = syan::source::proc_macro2::Span;

/// A leaf statement `N ;`.
#[macro_derive(Ast, Parse, Unparse, Debug)]
pub struct Stmt<S> {
    pub n: Integer,
    pub semi: Token![S => ;],
}

/// `{ stmts… } ( tail? )` — a brace group holding a `#[seq]` `Vec` and a paren group holding a `#[opt]`
/// `Option`. Each grouped field is *both* a parse group (`#[group(...)]`) and a structural-edit view.
#[macro_derive(Ast, Parse, Unparse, Debug)]
#[subast(crate::Stmt)]
pub struct Doc<S> {
    pub brace: GroupBrace<(), S>,
    #[group(self.brace)]
    #[seq]
    pub stmts: Vec<Stmt<S>>,
    pub paren: GroupParen<(), S>,
    #[group(self.paren)]
    #[opt]
    pub tail: Option<Stmt<S>>,
}

mod v {
    syan::visit::visitor!(crate::Stmt, crate::Doc);
}

/// Drop `0 ;` statements from the grouped `Vec`; clear the grouped `Option` tail if it is `7 ;`.
struct Editor;
impl<S> v::VisitMut<S> for Editor {
    fn visit_stmt_seq<V: SeqView<Stmt<S>>>(&mut self, v: &mut V) {
        v.retain_mut(|s| s.n.value != "0");
    }
    fn visit_stmt_opt<O: OptView<Stmt<S>>>(&mut self, v: &mut O) {
        if matches!(v.get(), Some(s) if s.n.value == "7") {
            v.take();
        }
    }
}

fn round_trip(doc: &Doc<Sp>) -> String {
    let mut out = Vec::<proc_macro2::TokenTree>::new();
    doc.unparse(&mut (&mut out)).unwrap();
    out.into_iter().collect::<proc_macro2::TokenStream>().to_string()
}

fn vals(doc: &Doc<Sp>) -> Vec<&str> {
    doc.stmts.iter().map(|s| s.n.value.as_str()).collect()
}

#[test]
fn edit_grouped_seq_and_opt() {
    let mut doc: Doc<Sp> = Parse::parse(quote!( { 1 ; 0 ; 2 ; 0 ; } ( 7 ; ) )).unwrap();
    assert_eq!(doc.stmts.len(), 4);
    assert!(doc.tail.is_some());

    doc.visit_mut(&mut Editor);

    assert_eq!(vals(&doc), vec!["1", "2"], "`0 ;` dropped from the #[group] #[seq] Vec");
    assert!(doc.tail.is_none(), "the #[group] #[opt] tail (`7 ;`) was cleared via OptView");

    // The brace/paren group delimiters survive the edit + Unparse.
    let s = round_trip(&doc);
    assert!(s.contains('{') && s.contains('}'), "brace group preserved: {s}");
    assert!(s.contains('(') && s.contains(')'), "paren group preserved: {s}");
    assert!(s.contains('1') && s.contains('2'), "kept statements present: {s}");
    assert!(!s.contains('0') && !s.contains('7'), "dropped/cleared values gone: {s}");
}

#[test]
fn tail_kept_when_not_seven() {
    let mut doc: Doc<Sp> = Parse::parse(quote!( { 5 ; } ( 3 ; ) )).unwrap();
    doc.visit_mut(&mut Editor);
    assert_eq!(vals(&doc), vec!["5"]);
    assert_eq!(doc.tail.as_ref().map(|s| s.n.value.as_str()), Some("3"), "a non-`7` tail is kept");
}

#[test]
fn empty_groups_edit_is_noop() {
    let mut doc: Doc<Sp> = Parse::parse(quote!( { } ( ) )).unwrap();
    assert!(doc.stmts.is_empty() && doc.tail.is_none());
    doc.visit_mut(&mut Editor);
    assert!(doc.stmts.is_empty() && doc.tail.is_none());
    let s = round_trip(&doc);
    assert!(s.contains('{') && s.contains('}') && s.contains('(') && s.contains(')'), "{s}");
}

// ── group + `#[seq]` inside a `#[recurse]` cycle: a `{ … }` block holds a `#[seq]` Vec of the cycle
//    type; the grouped seq is edited (recursively) through its `SeqView`. ─────────────────────────────
mod rec {
    use super::Sp;
    use syan::parse::{recurse, Parse};
    use syan::visit::SeqView;
    use template_quote::quote;

    #[recurse]
    mod ast {
        use syan::nested::group::GroupBrace;
        use syan::parse::{Parse, Unparse};
        use syan::source::proc_macro2::literal::Integer;
        use syan::visit::Ast;
        use type_macro_derive_tricks::macro_derive;

        #[macro_derive(Ast, Parse, Unparse, Debug)]
        #[subast(crate::rec::ast::Expr)]
        pub enum Expr<S> {
            Block {
                brace: GroupBrace<(), S>,
                #[group(self.brace)]
                #[seq] // grouped, self-recursive Vec-like slot -> visit_expr_seq
                items: Vec<Expr<S>>,
            },
            Lit(Integer),
        }
    }

    mod v {
        syan::visit::visitor!(crate::rec::ast::Expr);
    }

    // Descend every grouped block, then drop `0` literals at each depth.
    struct Editor;
    impl<S> v::VisitMut<S> for Editor {
        fn visit_expr_seq<V: SeqView<ast::Expr<S>>>(&mut self, v: &mut V) {
            for e in v.view_iter_mut() {
                v::visit_expr_mut(self, e); // recurse into nested blocks
            }
            v.retain_mut(|e| !matches!(e, ast::Expr::Lit(i) if i.value == "0"));
        }
    }

    fn lits(e: &ast::Expr<Sp>) -> Vec<String> {
        match e {
            ast::Expr::Block { items, .. } => items.iter().flat_map(lits).collect(),
            ast::Expr::Lit(i) => vec![i.value.clone()],
        }
    }

    #[test]
    fn seq_edit_in_group_recurse_cycle() {
        // `{ 1 0 { 2 0 } 3 }` — drop the `0`s in the grouped blocks at both depths.
        let mut e: ast::Expr<Sp> = Parse::parse(quote!( { 1 0 { 2 0 } 3 } )).unwrap();
        e.visit_mut(&mut Editor);
        assert_eq!(lits(&e), vec!["1", "2", "3"], "0s dropped from grouped blocks at both depths");
    }
}
