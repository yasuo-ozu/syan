// AUDIT (compile error): #[derive(Unparse)] on a zero-variant enum emits `match self {}`, which
// rustc rejects with E0004 (non-exhaustive — `&NoVariants` is a reference, always inhabited). The
// Parse path folds over zero variants and returns Err, and Spanned aborts cleanly ("no field
// exists"); only Unparse mis-compiles. Fix: special-case zero variants (e.g. `match *self {}` on the
// uninhabited place, or an `Ok(())`/`unreachable!()` body), or abort like Spanned does.
use syan::parse::Unparse;

#[derive(Unparse)]
pub enum NoVariants {}

fn main() {}
