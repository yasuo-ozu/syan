// AUDIT #2 (now a CLEAN error): symbol! used to *panic* (a raw proc-macro panic) on any character it
// has no mapping for — a non-ASCII XID identifier (`Symbol![café]`) or a char literal with a control
// char (`Symbol!['\n']`). char_to_type_path now returns `Option` and symbol() emits a clean spanned
// `abort!(token.span, "symbol! does not support the character …")`. It is still (correctly) a compile
// error — an unsupported character should be rejected — just no longer a panic.
use syan::symbol::Symbol;

fn main() {
    let _ = <Symbol![café]>::default();
}
