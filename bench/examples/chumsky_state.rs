//! chumsky の `ParserExtra::State` を使う実例。
//!
//!     cargo run -p syan-bench --example chumsky_state
//!
//! State の典型用途は「パース中に溜めるが、構文規則そのものには影響しないもの」。
//! ここでは 2 つ同時にやる:
//!   * 識別子のインターン    — 同じ名前を 1 つの `SymId` に潰す
//!   * AST のアリーナ確保    — `Box` を使わず `Vec<Node>` に詰めて `NodeId` を返す
//!
//! どちらも「出力側」であって、どのトークンを受理するかは一切変えない。
//! 文法自体を変えたいなら State ではなく `Context` (`then_with_ctx` / `configure`)。

use chumsky::inspector::{RollbackState, SimpleState};
use chumsky::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymId(u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeId(u32);

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Int(i64),
    Var(SymId),
    Add(NodeId, NodeId),
    Mul(NodeId, NodeId),
}

/// パース中に育てていく副作用の置き場。`Clone` は例 3 の `RollbackState` 用。
#[derive(Debug, Default, Clone)]
pub struct Arena {
    pub syms: Vec<String>,
    pub nodes: Vec<Node>,
}

impl Arena {
    fn intern(&mut self, s: &str) -> SymId {
        // インターンは冪等なので、投機的に実行されても害がない ← 後述の落とし穴を免れる理由
        if let Some(i) = self.syms.iter().position(|x| x == s) {
            return SymId(i as u32);
        }
        self.syms.push(s.to_string());
        SymId((self.syms.len() - 1) as u32)
    }
    fn alloc(&mut self, n: Node) -> NodeId {
        self.nodes.push(n);
        NodeId((self.nodes.len() - 1) as u32)
    }
}

// ---------------------------------------------------------------------------
// 例 1: SimpleState でインターン + アリーナ
// ---------------------------------------------------------------------------

/// `extra::Full<Error, State, Context>`。State を使うのでデフォルトの `()` から差し替える。
type E<'a> = extra::Full<Rich<'a, char>, SimpleState<Arena>, ()>;

fn expr<'a>() -> impl Parser<'a, &'a str, NodeId, E<'a>> + Clone {
    recursive(|expr| {
        let atom = choice((
            // `map_with` のクロージャ第 2 引数 `e: &mut MapExtra` から `e.state()` で触る。
            // `SimpleState<T>` は `DerefMut<Target = T>` なので Arena のメソッドが直接呼べる。
            text::int(10).map_with(|s: &str, e| {
                let st: &mut SimpleState<Arena> = e.state();
                st.alloc(Node::Int(s.parse().unwrap()))
            }),
            text::ident().map_with(|s: &str, e| {
                let st: &mut SimpleState<Arena> = e.state();
                let id = st.intern(s);
                st.alloc(Node::Var(id))
            }),
            expr.delimited_by(just('('), just(')')),
        ))
        .padded();

        // 左結合の畳み込み。ここでも `e.state()` が使えるので、
        // 中間ノードを Box せずアリーナに置いて NodeId だけ返せる。
        let product = atom.clone().foldl_with(
            just('*').padded().ignore_then(atom).repeated(),
            |l, r, e| {
                let st: &mut SimpleState<Arena> = e.state();
                st.alloc(Node::Mul(l, r))
            },
        );
        product.clone().foldl_with(
            just('+').padded().ignore_then(product).repeated(),
            |l, r, e| {
                let st: &mut SimpleState<Arena> = e.state();
                st.alloc(Node::Add(l, r))
            },
        )
    })
}

fn example1() {
    let mut arena = SimpleState(Arena::default());
    let root = expr()
        .then_ignore(end())
        .parse_with_state("x*2 + y*x", &mut arena)
        .into_result();

    println!("--- 1. インターン + アリーナ (SimpleState) ---");
    println!("root  = {root:?}");
    println!(
        "syms  = {:?}   <- x, y が 1 つずつ。3 回書いた x は SymId(0) に潰れている",
        arena.syms
    );
    println!("nodes = {:#?}", arena.nodes);
    println!();
}

// ---------------------------------------------------------------------------
// 例 2: 落とし穴 — バックトラックしても State は巻き戻らない
// ---------------------------------------------------------------------------

fn example2() {
    // `let` 文と式文。`let` の枝が識別子を確保してから `=` で失敗し、式文の枝が通る。
    type X<'a> = extra::Full<Rich<'a, char>, SimpleState<Arena>, ()>;
    let stmt = choice((
        // `let x = ...` を狙う枝。ident を確保した後で `=` が無くて落ちる。
        text::ident::<_, X>()
            .padded()
            .map_with(|s: &str, e| e.state().intern(s))
            .then_ignore(just('=').padded())
            .labelled("let-binding"),
        // 式文の枝
        text::ident::<_, X>()
            .padded()
            .map_with(|s: &str, e| e.state().intern(s)),
    ));

    let mut arena = SimpleState(Arena::default());
    let r = stmt
        .then_ignore(end())
        .parse_with_state("foo", &mut arena)
        .into_result();
    println!("--- 2. 落とし穴: 巻き戻らない State (SimpleState) ---");
    println!("結果  = {r:?}");
    println!("syms  = {:?}", arena.syms);
    println!("→ intern は冪等なので、捨てられた枝が先に確保していても結果は同じ。");
    println!("  しかし『採用された分だけ』が要る操作 (カウント、ログ、非冪等な確保) では壊れる:");

    // カウントは非冪等なので壊れる
    let mut n = SimpleState(0u32);
    type C<'a> = extra::Full<Rich<'a, char>, SimpleState<u32>, ()>;
    let counted = just::<_, _, C>('a').map_with(|c, e| {
        **e.state() += 1;
        c
    });
    let p = choice((counted.then(just('b')), counted.then(just('c'))));
    let ok = p.parse_with_state("ac", &mut n).into_result().is_ok();
    println!(
        "  例: 'a' を数える  ok={ok}  counter={}  <- 実際に採用された 'a' は 1 つ",
        n.0
    );
    println!();
}

// ---------------------------------------------------------------------------
// 例 3: RollbackState で巻き戻す
// ---------------------------------------------------------------------------

fn example3() {
    let mut n = RollbackState(0u32);
    type R<'a> = extra::Full<Rich<'a, char>, RollbackState<u32>, ()>;
    let counted = just::<_, _, R>('a').map_with(|c, e| {
        **e.state() += 1;
        c
    });
    let p = choice((counted.then(just('b')), counted.then(just('c'))));
    let ok = p.parse_with_state("ac", &mut n).into_result().is_ok();

    println!("--- 3. RollbackState ---");
    println!("ok={ok}  counter={}  <- on_rewind で復元された", n.0);
    println!(
        "代償: on_save のたびに丸ごと clone する。Arena のような重い State では高くつくので、"
    );
    println!("      本当に必要なら自前 Inspector で差分だけ巻き戻す (nodes.truncate(len) 等)。");
    println!();
}

// ---------------------------------------------------------------------------
// 例 4: 自前 Inspector — アリーナを truncate だけで巻き戻す
// ---------------------------------------------------------------------------

mod undo {
    use super::*;
    use chumsky::input::{Checkpoint, Cursor};
    use chumsky::inspector::Inspector;

    /// `RollbackState<Arena>` の代わり。Checkpoint は 2 つの長さだけなので clone が O(1)。
    #[derive(Debug, Default)]
    pub struct UndoArena(pub Arena);

    impl<'src, I: Input<'src>> Inspector<'src, I> for UndoArena {
        /// 丸ごと clone せず、末尾を切り戻すための長さだけ持つ。
        type Checkpoint = (usize, usize);
        fn on_token(&mut self, _: &I::Token) {}
        fn on_save<'parse>(&self, _: &Cursor<'src, 'parse, I>) -> Self::Checkpoint {
            (self.0.syms.len(), self.0.nodes.len())
        }
        fn on_rewind<'parse>(&mut self, m: &Checkpoint<'src, 'parse, I, Self::Checkpoint>) {
            let (syms, nodes) = *m.inspector();
            self.0.syms.truncate(syms);
            self.0.nodes.truncate(nodes);
        }
    }
}

fn example4() {
    use undo::UndoArena;
    type U<'a> = extra::Full<Rich<'a, char>, UndoArena, ()>;

    let node = text::ident::<_, U>().padded().map_with(|s: &str, e| {
        let st = e.state();
        let id = st.0.intern(s);
        st.0.alloc(Node::Var(id))
    });
    // 第 1 枝は node を作ってから '=' で失敗する
    let p = choice((node.then_ignore(just('=')), node));

    let mut st = UndoArena::default();
    let r = p
        .then_ignore(end())
        .parse_with_state("foo", &mut st)
        .into_result();
    println!("--- 4. 自前 Inspector (O(1) チェックポイント) ---");
    println!("結果  = {r:?}");
    println!(
        "syms  = {:?}  nodes = {}  <- 捨てられた枝の確保は truncate で消えている",
        st.0.syms,
        st.0.nodes.len()
    );
}

fn main() {
    example1();
    example2();
    example3();
    example4();
}
