// SPIKE for docs/recurse-unbounded-plan.md §9.1 — self-contained, no syan deps.
//
// Validates the unbounded-#[recurse] mechanism in isolation:
//   (a) compiles with FINITE type recursion (no infinite monomorphization / E0275),
//   (b) parses arbitrarily deep input,
//   (c) backtracks correctly THROUGH the terminator re-entry boundary.
//
// Grammar (deliberately backtracking + deeply recursive):
//   Expr = "(" Expr ")" "!"   |   "(" Expr ")"   |   Int
// The first two variants share the "(" Expr ")" prefix, so trying variant A and failing on the
// missing "!" forces a rewind of EVERYTHING consumed during the deep recursive descent — i.e. a
// backtrack across terminator re-entries.

use std::any::type_name;
use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};

// ───────────────────────── toy token stream (mirrors syan: next/peek/push + a nesting Dup) ──────────
#[derive(Clone, Copy, Debug, PartialEq)]
enum Tok {
    L,
    R,
    Bang,
    Int(i64),
}

// Object-safe: only next/peek/push (this is the §3 "make ParseStream object-safe" requirement).
trait Stream {
    fn next(&mut self) -> Option<Tok>;
    fn peek(&mut self) -> Option<Tok>;
    fn push(&mut self, t: Tok);
}

// `&mut T: Stream` blanket (so `&mut dyn Stream: Stream`, hence usable as the erased re-entry stream).
impl<T: Stream + ?Sized> Stream for &mut T {
    fn next(&mut self) -> Option<Tok> {
        (**self).next()
    }
    fn peek(&mut self) -> Option<Tok> {
        (**self).peek()
    }
    fn push(&mut self, t: Tok) {
        (**self).push(t)
    }
}

struct VecStream(VecDeque<Tok>);
impl Stream for VecStream {
    fn next(&mut self) -> Option<Tok> {
        self.0.pop_front()
    }
    fn peek(&mut self) -> Option<Tok> {
        self.0.front().copied()
    }
    fn push(&mut self, t: Tok) {
        self.0.push_front(t)
    }
}

// Backtracking wrapper — wraps `&mut S`, so each `dup` nests the TYPE (`Dup<&mut Dup<&mut …>>`); this
// is exactly the stream-type growth that causes E0275 in a derived recursive `Parse`.
struct Dup<S> {
    inner: S,
    taken: Vec<Tok>,
    pushed: Vec<Tok>,
}
impl<S: Stream> Stream for Dup<S> {
    fn next(&mut self) -> Option<Tok> {
        if let Some(t) = self.pushed.pop() {
            return Some(t);
        }
        let t = self.inner.next();
        if let Some(t) = t {
            self.taken.push(t);
        }
        t
    }
    fn peek(&mut self) -> Option<Tok> {
        if let Some(&t) = self.pushed.last() {
            return Some(t);
        }
        self.inner.peek()
    }
    fn push(&mut self, t: Tok) {
        self.pushed.push(t);
    }
}

// `dup` is the GENERIC method that would make the base trait non-object-safe. Here it's a separate
// `Sized`-bounded ext trait; the real design instead keeps it on the base trait with `where Self: Sized`
// (object-safe, no extra trait — see `core/tests/spike_real_parsestream.rs`). Closures get
// `&mut Dup<&mut Self>`; commit/rollback mirrors syan.
trait StreamExt: Stream + Sized {
    fn dup<T>(&mut self, f: impl FnOnce(&mut Dup<&mut Self>) -> Result<T, ()>) -> Result<T, ()> {
        let mut d = Dup { inner: self, taken: Vec::new(), pushed: Vec::new() };
        let r = f(&mut d);
        let Dup { inner, taken, pushed } = d;
        match r {
            Ok(v) => {
                for t in pushed.into_iter().rev() {
                    inner.push(t);
                }
                Ok(v)
            }
            Err(()) => {
                for t in taken.into_iter().rev() {
                    inner.push(t);
                }
                Err(())
            }
        }
    }
}
impl<S: Stream + Sized> StreamExt for S {}

// ───────────────────────── the natural AST ──────────────────────────────────────────────────────
#[derive(Debug, PartialEq)]
enum Expr {
    Banged(Box<Expr>),
    Paren(Box<Expr>),
    Int(i64),
}

// ───────────────────────── the registry (§4: type_name-pointer key, erased fn ptr) ──────────────────
type ParseFn = fn(&mut dyn Stream) -> Result<Expr, ()>;
static REG: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();

// ICF-safe per-type key: the data pointer of `type_name::<T>()` (content differs per type, so the
// statics are never merged; same type → same interned &'static str → same pointer).
fn key<T: ?Sized>() -> usize {
    type_name::<T>().as_ptr() as *const core::ffi::c_void as usize
}
fn register<T: ?Sized>(f: ParseFn) {
    let m = REG.get_or_init(|| Mutex::new(HashMap::new()));
    m.lock().unwrap().entry(key::<T>()).or_insert(f as usize);
}
fn lookup<T: ?Sized>() -> ParseFn {
    let m = REG.get_or_init(|| Mutex::new(HashMap::new()));
    let raw = *m.lock().unwrap().get(&key::<T>()).expect("not registered");
    unsafe { core::mem::transmute::<usize, ParseFn>(raw) }
}

// A stable marker type the registry is keyed on (stands in for `__ExprTerm<S>`).
struct ExprKey;

fn expect<S: Stream>(s: &mut S, want: Tok) -> Result<(), ()> {
    match s.next() {
        Some(t) if t == want => Ok(()),
        Some(t) => {
            s.push(t);
            Err(())
        }
        None => Err(()),
    }
}

// ───────────────────────── the engine (depth-1) + Term re-entry ────────────────────────────────────
// `term_parse` is the recursive-child re-entry: it ERASES its stream to `&mut dyn Stream` and calls the
// registered top-level fn. Crucially it does NOT recurse generically into itself, so only finitely many
// monomorphizations of `expr_parse`/`term_parse` are ever instantiated (the compile *is* the proof).
fn term_parse<S: Stream>(s: &mut S) -> Result<Expr, ()> {
    let f = lookup::<ExprKey>();
    let dyns: &mut dyn Stream = s; // erase — cuts the Dup<…> type growth here
    f(dyns)
}

// The grammar:  Expr = "(" Expr ")"  |  Int.  Two variants, each tried with `dup` (so the stream TYPE
// nests `Dup<&mut Dup<…>>` exactly as a derived recursive `Parse` would — the growth the erasure cuts).
// LL(1): the wrong variant fails on its first token, so the rewind is O(1) and deep input is linear.
// Recursion goes through `term_parse` (the terminator re-entry), never directly into `expr_parse`.
fn expr_parse<S: Stream>(s: &mut S) -> Result<Expr, ()> {
    if let Ok(v) = s.dup(|d| {
        expect(d, Tok::L)?;
        let inner = term_parse(d)?;
        expect(d, Tok::R)?;
        Ok(Expr::Paren(Box::new(inner)))
    }) {
        return Ok(v);
    }
    s.dup(|d| match d.next() {
        Some(Tok::Int(n)) => Ok(Expr::Int(n)),
        Some(t) => {
            d.push(t);
            Err(())
        }
        None => Err(()),
    })
}

// Top-level backtrack THROUGH the terminator boundary:  Top = Expr "!"  |  Expr.  Trying the first
// alternative parses a full (deep) `Expr` — descending through many term re-entries — and on a missing
// trailing "!" rewinds the ENTIRE Expr (every term boundary) in one O(D) backtrack, then parses plain
// `Expr`. This proves the `dup` snapshot/restore propagates across the erased re-entry.
fn top_bt<S: Stream>(s: &mut S) -> Result<Expr, ()> {
    if let Ok(v) = s.dup(|d| {
        let e = expr_parse(d)?;
        expect(d, Tok::Bang)?;
        Ok(Expr::Banged(Box::new(e)))
    }) {
        return Ok(v);
    }
    expr_parse(s)
}

// The registered erased entry: `fn(&mut dyn Stream) -> Result<Expr,()>` (the top-level natural parse,
// monomorphized at the erased stream type — re-entry always restarts from `Dup<&mut dyn Stream>`).
fn expr_parse_dyn(mut s: &mut dyn Stream) -> Result<Expr, ()> {
    expr_parse(&mut s)
}

fn parse_top(toks: Vec<Tok>) -> Result<Expr, ()> {
    register::<ExprKey>(expr_parse_dyn);
    let mut s = VecStream(toks.into());
    let e = expr_parse(&mut s)?;
    if s.next().is_some() {
        return Err(()); // trailing tokens
    }
    Ok(e)
}

// Entry that uses the top-level `Expr "!" | Expr` backtrack (for the through-terminator checks).
fn parse_top_bt(toks: Vec<Tok>) -> Result<Expr, ()> {
    register::<ExprKey>(expr_parse_dyn);
    let mut s = VecStream(toks.into());
    let e = top_bt(&mut s)?;
    if s.next().is_some() {
        return Err(());
    }
    Ok(e)
}

fn main() {
    // (b) arbitrarily deep: N nested parens around an int. (Far past the old type `limit` of ~4–12.)
    // The only ceiling is the OS call stack — runtime recursion, exactly like any recursive-descent
    // parser — not a type-level depth cap. 2000 is comfortable on the default stack.
    for depth in [0usize, 1, 5, 50, 500, 2000] {
        let mut toks = Vec::new();
        for _ in 0..depth {
            toks.push(Tok::L);
        }
        toks.push(Tok::Int(depth as i64));
        for _ in 0..depth {
            toks.push(Tok::R);
        }
        let e = parse_top(toks).expect("deep parse");
        // count Paren layers
        let mut n = 0usize;
        let mut cur = &e;
        while let Expr::Paren(inner) = cur {
            n += 1;
            cur = inner;
        }
        assert_eq!(n, depth, "depth {depth}: got {n} paren layers");
        assert_eq!(*cur, Expr::Int(depth as i64));
        println!("(b) depth {depth:>5}: OK  ({n} Paren layers)");
    }

    // (c) backtracking THROUGH terminators. `((1))!` → Banged(Paren(Paren(Int 1))): the top-level
    // `Expr "!"` alternative parses the whole `((1))` (through term re-entries) and the "!" succeeds.
    let banged = parse_top_bt(vec![Tok::L, Tok::L, Tok::Int(1), Tok::R, Tok::R, Tok::Bang])
        .expect("((1))!");
    assert_eq!(
        banged,
        Expr::Banged(Box::new(Expr::Paren(Box::new(Expr::Paren(Box::new(Expr::Int(1))))))),
    );
    println!("(c) ((1))! → {banged:?}  OK");

    // `((2))` (no "!"): the `Expr "!"` alternative parses all of `((2))` then fails on the missing "!",
    // rewinding EVERY term boundary, before the plain `Expr` alternative re-parses it. The single
    // backtrack spans all the re-entries.
    let paren = parse_top_bt(vec![Tok::L, Tok::L, Tok::Int(2), Tok::R, Tok::R]).expect("((2))");
    assert_eq!(paren, Expr::Paren(Box::new(Expr::Paren(Box::new(Expr::Int(2))))));
    println!("(c) ((2))  → {paren:?}  OK  (rewound 2 term boundaries, then re-parsed)");

    // Deep backtrack: D=200 nested parens, no "!", so the top alternative descends 200 term boundaries,
    // fails, and rewinds all of them in one O(D) backtrack. (Linear, not exponential — only ONE
    // top-level alternative backtracks; `expr_parse` itself is LL(1).)
    let d = 200usize;
    let mut t = vec![Tok::L; d];
    t.push(Tok::Int(7));
    t.extend(std::iter::repeat(Tok::R).take(d));
    let deep = parse_top_bt(t).expect("deep ((…7…))");
    let mut layers = 0;
    let mut cur = &deep;
    while let Expr::Paren(i) = cur {
        layers += 1;
        cur = i;
    }
    assert_eq!(layers, d);
    assert_eq!(*cur, Expr::Int(7));
    println!("(c) deep backtrack (D={d}): OK  (rewound {layers} term boundaries, then re-parsed)");

    println!("\nALL SPIKE CHECKS PASSED");
}
