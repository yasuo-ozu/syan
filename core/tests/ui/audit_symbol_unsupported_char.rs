// AUDIT (panic): symbol! panics (a raw proc-macro panic, not a spanned error) on any character it
// has no mapping for. macro/symbol.rs char_to_type_path ends in
// `_ => panic!("Unsupported character: {} (code: {})", c, c as u32)`, reached from valid user input
// such as a non-ASCII XID identifier (`Symbol![café]`) or a char literal with a control char
// (`Symbol!['\n']`). The SymbolToken already carries a span, so the fix is abort!(span, ...).
use syan::symbol::Symbol;

fn main() {
    let _ = <Symbol![café]>::default();
}
