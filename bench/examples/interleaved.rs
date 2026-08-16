//! 負荷のある機械でも使える壁時計計測: 1 プロセス内で全バックエンドを**交互に**回し、
//! 各バックエンドの **min-of-N** を取る。
//!
//! criterion の平均は他プロセスの負荷で壊れるが、最小値は「最も邪魔されなかった実行」を
//! 拾うので、負荷の下でも比が保たれる。`perf-measurements.md` の §7c/§7d が同じ手法。
//!
//!     cargo run --release -p syan-bench --example interleaved

use std::hint::black_box;
use std::time::{Duration, Instant};
use syan_bench::{chumsky_impl, combine_impl, input, nom_impl, syan_char, syan_token};

const ROUNDS: usize = 40;

fn bench(f: &mut dyn FnMut(), inner: usize) -> Duration {
    let t = Instant::now();
    for _ in 0..inner {
        f();
    }
    t.elapsed() / inner as u32
}

fn main() {
    let cases: Vec<(String, String)> = [4usize, 16, 64, 256]
        .iter()
        .map(|n| (format!("flat/{n}ops"), input::flat(*n)))
        .chain(
            [1usize, 8, 32, 128]
                .iter()
                .map(|d| (format!("nested/depth{d}"), input::nested(*d))),
        )
        .chain(
            [2usize, 4, 6, 8]
                .iter()
                .map(|d| (format!("tree/depth{d}"), input::tree(*d))),
        )
        .chain(
            [4usize, 64]
                .iter()
                .map(|n| (format!("error/{n}ops"), input::bad_at(*n))),
        )
        .collect();

    let names = [
        "nom",
        "chumsky",
        "combine",
        "char/rk",
        "char/st",
        "token/rk",
        "token/st",
    ];
    println!(
        "min-of-{ROUNDS} interleaved, 単位 µs (トークン列は parse-only。字句解析は測定外)\n"
    );
    print!("| case |");
    for n in names {
        print!(" {n} |");
    }
    println!(" combine/nom | char st /chumsky |");
    print!("|---|");
    for _ in names {
        print!("---:|");
    }
    println!("---:|---:|");

    for (label, src) in &cases {
        let err = label.starts_with("error");
        // 反復回数: 小さい入力ほど多く回して計測分解能を確保
        let inner = (200_000 / (src.len() + 20)).clamp(20, 2000);
        let ts = syan_token::tokenise(src);
        let mut best = [Duration::MAX; 7];

        for _ in 0..ROUNDS {
            let mut fs: [Box<dyn FnMut()>; 7] = [
                Box::new(|| {
                    black_box(nom_impl::parse(black_box(src)).is_ok());
                }),
                Box::new(|| {
                    black_box(chumsky_impl::parse(black_box(src)).is_ok());
                }),
                Box::new(|| {
                    black_box(combine_impl::parse(black_box(src)).is_ok());
                }),
                Box::new(|| {
                    black_box(syan_char::ranked::parse(black_box(src)).is_ok());
                }),
                Box::new(|| {
                    black_box(syan_char::structural::parse(black_box(src)).is_ok());
                }),
                Box::new(|| {
                    black_box(syan_token::ranked::parse_pretokenised(ts.clone()).is_ok());
                }),
                Box::new(|| {
                    black_box(syan_token::structural::parse_pretokenised(ts.clone()).is_ok());
                }),
            ];
            for (i, f) in fs.iter_mut().enumerate() {
                let d = bench(&mut **f, inner);
                if d < best[i] {
                    best[i] = d;
                }
            }
        }

        let us = |d: Duration| d.as_secs_f64() * 1e6;
        print!("| {label}{} |", if err { " *" } else { "" });
        for b in best {
            print!(" {:.2} |", us(b));
        }
        println!(
            " {:.2}× | {:.2}× |",
            us(best[2]) / us(best[0]),
            us(best[4]) / us(best[1])
        );
    }
    println!("\n* error 行は失敗パス。token 列は lex+parse ではなく parse-only。");
    println!("`ts.clone()` は token 列の測定領域内に入る (proc-macro2 の deep copy) 点に注意。");
}
