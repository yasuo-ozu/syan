//! chumsky の `ParserExtra::Context` を使う実例。
//!
//!     cargo run -p syan-bench --example chumsky_context
//!
//! State との違い (extra.rs:29-37):
//!   * State   … 出力側。パースの進行には影響しない。
//!   * Context … 入力側。**文法そのものを変える**。ただし常に左辺依存で、文脈はストリーム上で
//!     それを使う地点より前から来なければならない。
//!
//! API は 4 つ:
//! ```text
//! a.ignore_with_ctx(b)        a の出力を b の Context にする (a の出力は捨てる)
//! a.then_with_ctx(b)          同上だが出力が (Context, b の出力)
//! p.with_ctx(v)               固定値を Context として与える
//! p.configure(|cfg, ctx| ..)  ctx から just/repeated 等の設定を作る
//! ```

use chumsky::prelude::*;

// ===========================================================================
// 例 1: 長さ前置 — 先頭の数字が後続要素数を決める
// ===========================================================================

fn example1() {
    let p = any::<_, extra::Err<Rich<char>>>()
        .try_map(|c: char, s| {
            c.to_digit(10)
                .map(|d| d as usize)
                .ok_or_else(|| Rich::custom(s, "数字が必要"))
        })
        // ここで Context が usize に変わる。repeated の回数を ctx から設定。
        .ignore_with_ctx(
            just('x')
                .repeated()
                .configure(|cfg, ctx: &usize| cfg.exactly(*ctx)),
        )
        .then_ignore(end());

    println!("--- 1. 長さ前置 (Context = usize) ---");
    for s in ["2xx", "2x", "2xxx", "0", "3xxx"] {
        println!("  {s:6} -> {}", p.parse(s).into_result().is_ok());
    }
    println!();
}

// ===========================================================================
// 例 2: Rust の raw string — 開き `#` の個数が閉じ方を決める
//       Context 無しでは書けない典型例 (正規表現でも書けない)
// ===========================================================================

fn example2() {
    // Context = usize (`#` の個数)。閉じ側もこの ctx を読む。
    let closing = just::<_, _, extra::Full<Rich<char>, (), usize>>('"').ignore_then(
        just('#')
            .repeated()
            .configure(|cfg, ctx: &usize| cfg.exactly(*ctx)),
    );

    let body = any()
        .and_is(closing.not())
        .repeated()
        .to_slice()
        .then_ignore(closing);

    let raw = just::<_, _, extra::Err<Rich<char>>>('r')
        .ignore_then(just('#').repeated().count()) // -> usize、これが Context になる
        .ignore_with_ctx(just('"').ignore_then(body))
        .then_ignore(end());

    println!("--- 2. raw string (Context = `#` の個数) ---");
    for s in [
        r####"r"abc""####,
        r####"r#"a"b"#"####,    // 内側の `"` は閉じない
        r####"r##"a"#b"##"####, // 内側の `"#` も閉じない
        r####"r#"abc""####,     // 閉じ `#` が足りない -> エラー
    ] {
        match raw.parse(s).into_result() {
            Ok(body) => println!("  {s:20} -> Ok({body:?})"),
            Err(e) => println!("  {s:20} -> Err({})", e[0]),
        }
    }
    println!();
}

// ===========================================================================
// 例 3: 結合順序 — Context に最小結合力 (min binding power) を載せた
//       precedence climbing
// ===========================================================================

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    Num(i64),
    Bin(Box<Expr>, char, Box<Expr>),
}

impl Expr {
    /// 括弧つきで表示して、どう結合したかを見えるようにする
    fn show(&self) -> String {
        match self {
            Expr::Num(n) => n.to_string(),
            Expr::Bin(l, op, r) => format!("({} {} {})", l.show(), op, r.show()),
        }
    }
}

/// (左結合力, 右結合力)。左 < 右 なら左結合、左 > 右 なら右結合。
fn bp(op: char) -> (u8, u8) {
    match op {
        '+' | '-' => (1, 2), // 左結合
        '*' | '/' => (3, 4), // 左結合
        '^' => (6, 5),       // 右結合 ← 左右を逆転させるだけで切り替わる
        _ => (0, 0),
    }
}

/// Context = (直前の演算子, 最小結合力)。演算子も載せるのは
/// `then_with_ctx` が `(Context, 出力)` を返すのを利用して、親がどの演算子で
/// 繋いだかを取り戻すため。
type Ctx = (char, u8);
type P<'a> = extra::Full<Rich<'a, char>, (), Ctx>;

fn expr<'a>() -> impl Parser<'a, &'a str, Expr, P<'a>> + Clone {
    recursive(|expr| {
        let atom = choice((
            text::int(10).map(|s: &str| Expr::Num(s.parse().unwrap())),
            // 括弧の中は結合力をリセットする。これが `with_ctx` の出番。
            expr.clone()
                .with_ctx(('\0', 0))
                .delimited_by(just('('), just(')')),
        ))
        .padded();

        // 演算子を読み、周囲の min_bp と比べる。低ければ失敗して repeated を止める
        // = precedence climbing のループ脱出条件そのもの。
        let op = one_of("+-*/^")
            .padded()
            .try_map_with(|c: char, e| {
                let ctx: &Ctx = e.ctx(); // 周囲の min_bp
                let (l, r) = bp(c);
                if l < ctx.1 {
                    Err(Rich::custom(e.span(), "低優先度なので親に譲る"))
                } else {
                    Ok((c, r)) // これが再帰呼び出しの新しい Context
                }
            })
            // 右辺を「新しい min_bp」で再帰的に読む。出力は ((op, r_bp), rhs)。
            .then_with_ctx(expr);

        atom.foldl(op.repeated(), |lhs, ((c, _), rhs)| {
            Expr::Bin(Box::new(lhs), c, Box::new(rhs))
        })
    })
}

fn example3() {
    let p = expr().then_ignore(end());
    println!("--- 3. 結合順序 (Context = 最小結合力) ---");
    for s in [
        "1+2+3",   // 左結合
        "1-2-3",   // 左結合
        "1+2*3",   // * が強い
        "1*2+3",   // * が強い
        "2^3^2",   // 右結合
        "1+2^3*4", // ^ > * > +
        "(1+2)*3", // 括弧で結合力リセット
    ] {
        match p.parse(s).into_result() {
            Ok(e) => println!("  {s:10} -> {}", e.show()),
            Err(e) => println!("  {s:10} -> Err({})", e[0]),
        }
    }
    println!();
}

// ===========================================================================
// 例 4: 比較 — chumsky 標準の pratt モジュールは Context を使わない
// ===========================================================================

fn example4() {
    println!("--- 4. 比較: chumsky 標準の pratt は Context を使わない ---");
    println!("  優先順位登攀は chumsky にも `pratt` モジュールがあるが (feature = \"pratt\")、");
    println!("  そちらは min_power を Context ではなく内部の関数引数として渡している:");
    println!("    pratt.rs:119/149/182  invoke_pratt_op_{{prefix,postfix,infix}}(.., min_power: i32, ..)");
    println!("  実際 pratt.rs に `Context` の出現は 0 件。つまり結合順序は Context の");
    println!("  「想定用途」ではなく、例 3 は機構の説明として書いたもの。");
    println!("  Context が本当に要るのは例 2 のような、文法自体が入力に依存する場合。");
}

fn main() {
    example1();
    example2();
    example3();
    example4();
}
