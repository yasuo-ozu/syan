//! Generated inputs. Deterministic (no rng crate), so numbers are reproducible run to run.

/// `n` integers joined by alternating operators, no parens: `1 + 22 * 333 - 4 …`.
/// Exercises the flat/iterative path.
pub fn flat(n: usize) -> String {
    let ops = ["+", "*", "-", "/"];
    let mut s = String::new();
    for i in 0..n {
        if i > 0 {
            s.push(' ');
            // avoid `/ 0` and keep the value small-ish; operator choice is still deterministic
            s.push_str(ops[i % ops.len()]);
            s.push(' ');
        }
        s.push_str(&((i % 9) + 1).to_string());
    }
    s
}

/// `depth` nested parens around a small expression: `((((1 + 2))))`.
/// Exercises the recursive path — the one `#[recurse]` exists for.
pub fn nested(depth: usize) -> String {
    let mut s = String::from("1 + 2");
    for _ in 0..depth {
        s = format!("( {s} )");
    }
    s
}

/// A balanced binary tree of parens, `2^depth` leaves. Recursion AND width.
pub fn tree(depth: usize) -> String {
    if depth == 0 {
        return "1".into();
    }
    let half = tree(depth - 1);
    format!("( {half} + {half} )")
}

/// Inputs that FAIL, at a known distance in. Error-path cost is its own axis.
pub fn bad_at(n: usize) -> String {
    let mut s = flat(n);
    s.push_str(" + &");
    s
}
