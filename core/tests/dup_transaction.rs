//! Direct tests for `ParseStream::dup` as a TRANSACTION primitive.
//!
//! Every other test in this suite reaches `dup` only through `#[derive(Parse)]`
//! — via an enum alternative, an `Option`, or a `Vec`. That means the suite only
//! ever constructs the shapes the derive constructs, and never calls the
//! primitive directly (`grep -c '\.dup(' core/tests/*.rs` == 0 before this
//! file). These tests drive it head-on.
//!
//! `dup_commit_preserves_pushback_order` FAILS on the current implementation.
//! That is deliberate: it pins a real ordering bug so a fix cannot regress and
//! a rewrite cannot reintroduce it.

use syan::parse::ParseStream;
use syan::source::string::{Span, Stream};
use syan::span::WithSpan;

type Atom = WithSpan<char, Span>;

fn stream_of(src: &str) -> Stream {
    Stream::new(src.to_string())
}

/// Drain what the stream will serve from here on, as a plain string so the
/// assertions read as the input does.
fn drain(s: &mut impl ParseStream<Atom = Atom>) -> String {
    std::iter::from_fn(|| s.next()).map(|a| a.slot).collect()
}

// ---------------------------------------------------------------------------
// Baseline: the two paths a transaction can take.
// ---------------------------------------------------------------------------

#[test]
fn dup_rollback_restores_everything_consumed() {
    let mut s = stream_of("abc");
    let r: Result<(), &str> = s.dup(|d| {
        d.next();
        d.next();
        Err("fail after consuming two")
    });
    assert!(r.is_err());
    assert_eq!(drain(&mut s), "abc", "a failed dup must consume nothing");
}

#[test]
fn dup_commit_keeps_consumption() {
    let mut s = stream_of("abc");
    let r: Result<(), &str> = s.dup(|d| {
        d.next();
        d.next();
        Ok(())
    });
    assert!(r.is_ok());
    assert_eq!(
        drain(&mut s),
        "c",
        "a successful dup must keep its consumption"
    );
}

#[test]
fn dup_that_consumes_nothing_and_fails_is_a_no_op() {
    let mut s = stream_of("abc");
    let _: Result<(), &str> = s.dup(|_d| Err("nothing consumed"));
    assert_eq!(drain(&mut s), "abc");
}

#[test]
fn dup_that_consumes_to_eof_and_fails_restores_all() {
    let mut s = stream_of("abc");
    let _: Result<(), &str> = s.dup(|d| {
        while d.next().is_some() {}
        Err("ate everything")
    });
    assert_eq!(drain(&mut s), "abc");
}

// ---------------------------------------------------------------------------
// Nesting. `dup` scopes are strictly LIFO; an inner failure must not disturb
// what an outer scope can still rewind.
// ---------------------------------------------------------------------------

#[test]
fn inner_failure_does_not_escape_a_successful_outer() {
    let mut s = stream_of("abcd");
    let r: Result<(), &str> = s.dup(|d| {
        d.next(); // outer consumes 1
        let _inner: Result<(), &str> = d.dup(|i| {
            i.next(); // inner consumes 2
            Err("inner fails")
        });
        Ok(()) // outer succeeds having consumed only 1
    });
    assert!(r.is_ok());
    assert_eq!(
        drain(&mut s),
        "bcd",
        "the inner attempt's consumption must have been rolled back, \
         and the outer's kept"
    );
}

#[test]
fn outer_failure_restores_past_a_committed_inner() {
    let mut s = stream_of("abcd");
    let r: Result<(), &str> = s.dup(|d| {
        let _inner: Result<(), &str> = d.dup(|i| {
            i.next(); // inner consumes 1 and COMMITS
            Ok(())
        });
        d.next(); // outer consumes 2
        Err("outer fails")
    });
    assert!(r.is_err());
    assert_eq!(
        drain(&mut s),
        "abcd",
        "an outer rollback must undo a committed inner scope too"
    );
}

#[test]
fn three_deep_inner_fail_middle_ok_outer_fail() {
    let mut s = stream_of("abcde");
    let r: Result<(), &str> = s.dup(|a| {
        a.next(); // 1
        let _mid: Result<(), &str> = a.dup(|b| {
            let _in: Result<(), &str> = b.dup(|c| {
                c.next(); // 2
                Err("innermost fails")
            });
            b.next(); // 2 again
            Ok(()) // middle SUCCEEDS
        });
        a.next(); // 3
        Err("outermost fails")
    });
    assert!(r.is_err());
    assert_eq!(drain(&mut s), "abcde");
}

// ---------------------------------------------------------------------------
// THE DETECTOR — currently FAILS.
//
// `parse_stream.rs`'s Ok/commit path pops `push_buf` (reverse insertion order)
// onto a LIFO parent, which INVERTS two or more leftover pushbacks. A leftover
// arises when an inner attempt fails after consuming, replaying its atoms into
// the enclosing scope, and the enclosing scope then commits without re-reading
// them.
//
// In the real grammar such an attempt almost always dies on its first token, so
// there is at most ONE leftover and inversion is invisible. With two it is not.
// ---------------------------------------------------------------------------

#[test]
fn dup_commit_preserves_pushback_order() {
    let mut s = stream_of("abcd");
    let r: Result<(), &str> = s.dup(|d| {
        let _inner: Result<(), &str> = d.dup(|i| {
            i.next(); // 1
            i.next(); // 2   -> both replayed into the outer scope on failure
            Err("inner fails after consuming two")
        });
        Ok(()) // outer commits WITHOUT re-consuming them
    });
    assert!(r.is_ok());
    assert_eq!(
        drain(&mut s),
        "abcd",
        "leftover pushbacks must keep their original order \
         (today this yields \"bacd\")"
    );
}

/// The corruption is a FULL REVERSAL applied once per committing level, so it
/// CANCELS at even depth. Measured on the current implementation, leftovers x
/// committing levels above the failing inner dup:
///
///     n=1 lv=1 -> "abc.." ok      n=1 lv=2 -> "abc.." ok
///     n=2 lv=1 -> "bac.." WRONG   n=2 lv=2 -> "abc.." ok  (cancels)
///     n=3 lv=1 -> "cba.." WRONG n=3 lv=2 -> "abc.." ok  (cancels)
///
/// So a test at even depth proves nothing — the first version of this test used
/// two levels and passed while the bug was live. Three committing levels.
#[test]
fn dup_commit_preserves_pushback_order_odd_depth() {
    let mut s = stream_of("abcde");
    let r: Result<(), &str> = s.dup(|a| {
        let _: Result<(), ()> = a.dup(|b| {
            let _: Result<(), ()> = b.dup(|c| {
                let _: Result<(), &str> = c.dup(|i| {
                    i.next();
                    i.next(); // two leftovers
                    Err("innermost fails")
                });
                Ok(())
            });
            Ok(())
        });
        Ok(())
    });
    assert!(r.is_ok());
    assert_eq!(
        drain(&mut s),
        "abcde",
        "three committing levels = odd = one net reversal"
    );
}

// ---------------------------------------------------------------------------
// `push` is the primitive `dup` is built from; the leaf idiom is
// next-then-push-back-on-mismatch. It must compose with transactions.
// ---------------------------------------------------------------------------

#[test]
fn leaf_style_pushback_inside_a_failing_dup() {
    let mut s = stream_of("abc");
    let r: Result<(), &str> = s.dup(|d| {
        let a = d.next().unwrap(); // 'a'
        d.push(a); // leaf rejects it and un-consumes
        d.next(); // 'a' again
        Err("fail after a push cycle")
    });
    assert!(r.is_err());
    assert_eq!(drain(&mut s), "abc");
}

#[test]
fn peek_agrees_with_next_after_a_rollback() {
    let mut s = stream_of("abc");
    let _: Result<(), &str> = s.dup(|d| {
        d.next();
        d.next();
        Err("fail")
    });
    assert_eq!(s.peek().map(|a| a.slot), Some('a'));
    assert_eq!(
        s.next().map(|a| a.slot),
        Some('a'),
        "peek must predict next after a rollback"
    );
}

// ---------------------------------------------------------------------------
// SYSTEMATIC: the corruption is a full reversal applied once per committing
// level, so it is PARITY-dependent. Two hand-picked cases would miss half the
// space — and did: the first version of this file tested even depth and passed
// while the bug was live. This sweeps the space instead.
// ---------------------------------------------------------------------------

/// Run: `levels` nested dups that all COMMIT, wrapping an innermost dup that
/// consumes `leftovers` atoms and FAILS. Returns what the stream serves after.
///
/// The nesting is written out EXPLICITLY rather than by a depth-generic helper.
/// It no longer has to be — see `dup_nests_to_arbitrary_depth_generically` — but
/// when this test was written a generic helper could not compile at all
/// (`Dup<&mut Dup<&mut ..>>`, E0275), so growth source (1) blocked its own
/// systematic test. Kept as-is: the explicit form covers the same space.
fn leftover_shape(leftovers: usize, levels: usize) -> String {
    fn innermost(s: &mut impl ParseStream<Atom = Atom>, leftovers: usize) {
        let _: Result<(), &str> = s.dup(|i| {
            for _ in 0..leftovers {
                i.next();
            }
            Err("innermost fails")
        });
    }
    let mut s = stream_of("abcdef");
    match levels {
        1 => {
            let _: Result<(), ()> = s.dup(|a| {
                innermost(a, leftovers);
                Ok(())
            });
        }
        2 => {
            let _: Result<(), ()> = s.dup(|a| {
                let _: Result<(), ()> = a.dup(|b| {
                    innermost(b, leftovers);
                    Ok(())
                });
                Ok(())
            });
        }
        3 => {
            let _: Result<(), ()> = s.dup(|a| {
                let _: Result<(), ()> = a.dup(|b| {
                    let _: Result<(), ()> = b.dup(|c| {
                        innermost(c, leftovers);
                        Ok(())
                    });
                    Ok(())
                });
                Ok(())
            });
        }
        _ => {
            let _: Result<(), ()> = s.dup(|a| {
                let _: Result<(), ()> = a.dup(|b| {
                    let _: Result<(), ()> = b.dup(|c| {
                        let _: Result<(), ()> = c.dup(|d| {
                            innermost(d, leftovers);
                            Ok(())
                        });
                        Ok(())
                    });
                    Ok(())
                });
                Ok(())
            });
        }
    }
    drain(&mut s)
}

#[test]
fn leftover_order_is_preserved_at_every_depth_and_width() {
    let mut wrong = Vec::new();
    for leftovers in 1..=4 {
        for levels in 1..=4 {
            let got = leftover_shape(leftovers, levels);
            if got != "abcdef" {
                wrong.push(format!("leftovers={leftovers} levels={levels} -> {got:?}"));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "a committing dup must never reorder leftover pushbacks; \
         today each commit REVERSES them, so odd nesting corrupts and even \
         cancels:\n  {}",
        wrong.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// INVARIANT: a failed dup is a no-op on the stream, whatever the closure did.
// Driven over many pseudo-random bodies rather than hand-picked ones.
// ---------------------------------------------------------------------------

#[test]
fn a_failed_dup_is_always_a_no_op() {
    let src = "abcdefghij";
    let mut seed: u64 = 0x9E3779B97F4A7C15;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };

    for case in 0..500 {
        let mut s = stream_of(src);
        // random prefix consumed OUTSIDE the transaction, which must survive
        let pre = (rnd() % 4) as usize;
        for _ in 0..pre {
            s.next();
        }
        let expect: String = src.chars().skip(pre).collect();

        let script: Vec<u64> = (0..(rnd() % 8 + 1)).map(|_| rnd() % 4).collect();
        let r: Result<(), &str> = s.dup(|d| {
            for op in &script {
                match op {
                    0 => {
                        d.next();
                    }
                    1 => {
                        d.peek();
                    }
                    2 => {
                        if let Some(a) = d.next() {
                            d.push(a); // leaf idiom: un-consume
                        }
                    }
                    _ => {
                        let _: Result<(), &str> = d.dup(|i| {
                            i.next();
                            i.next();
                            Err("nested failure")
                        });
                    }
                }
            }
            Err("outer fails")
        });
        assert!(r.is_err());
        assert_eq!(
            drain(&mut s),
            expect,
            "case {case}: a failed dup must restore exactly \
             (pre={pre}, script={script:?})"
        );
    }
}

// ---------------------------------------------------------------------------
// Interleaving and lookahead across scope boundaries.
// ---------------------------------------------------------------------------

#[test]
fn outer_keeps_consuming_after_an_inner_failure_and_commits() {
    let mut s = stream_of("abcde");
    let r: Result<(), &str> = s.dup(|d| {
        d.next(); // 'a'
        let _: Result<(), &str> = d.dup(|i| {
            i.next(); // 'b'
            i.next(); // 'c'
            Err("inner fails, both replayed")
        });
        d.next(); // must be 'b' again
        Ok(())
    });
    assert!(r.is_ok());
    assert_eq!(
        drain(&mut s),
        "cde",
        "the outer re-consumed 'b' after the inner rolled back, so only 'a' \
         and 'b' are gone"
    );
}

#[test]
fn peek_inside_a_nested_scope_agrees_with_next() {
    let mut s = stream_of("abc");
    let _: Result<(), &str> = s.dup(|d| {
        d.next(); // 'a'
        let _: Result<(), &str> = d.dup(|i| {
            i.next(); // 'b'
            Err("roll back to just after 'a'")
        });
        let p = d.peek().map(|a| a.slot);
        let n = d.next().map(|a| a.slot);
        assert_eq!(p, n, "peek must predict next inside an outer scope");
        assert_eq!(n, Some('b'));
        Err("outer fails too")
    });
    assert_eq!(drain(&mut s), "abc");
}

#[test]
fn dup_error_type_is_fully_generic() {
    // The derive turbofishes `dup::<_, ParseError, _>`; E must stay unbounded.
    #[derive(Debug, PartialEq)]
    struct Custom(u32);
    let mut s = stream_of("abc");
    let r: Result<char, Custom> = s.dup(|d| {
        d.next();
        Err(Custom(7))
    });
    assert_eq!(r.unwrap_err(), Custom(7));
    assert_eq!(drain(&mut s), "abc");
}

// ---------------------------------------------------------------------------
// A FOURTH defect, adjacent to dup and found while auditing it: the wrapper
// types did not forward every trait method, so a caller reaching one through a
// wrapper hit the trait's `todo!()` default and PANICKED. It affected all three
// wrappers of the time — `Dup`, the `&mut T` blanket, and `SubStream`.
//
// `get_error`/`skip_sep` are now REQUIRED, like the checkpoint trio, so a
// forgotten forward is a compile error and this can no longer regress silently.
// The test is kept as the behavioural half of that: required-ness only stops an
// omission, not a forward that reaches the wrong stream.
// ---------------------------------------------------------------------------

#[test]
fn wrappers_forward_every_method() {
    let mut base = stream_of("ab");
    let mut through_ref = &mut base;
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ParseStream::skip_sep(&mut through_ref)
    }));
    assert!(
        r.is_ok(),
        "skip_sep through the `&mut T` blanket must reach the base stream"
    );
}

// ===========================================================================
// TYPE-GROWTH detectors.
//
// Both growth sources are fixed, so these are regression tests rather than probes for a pending
// defect. They record `type_name::<S>()` across a descent and assert the set of stream types stays
// BOUNDED — the property the two fixes established: `dup` hands the closure `&mut Self` (no `Dup`
// wrapper), and recursion reborrows `&mut *stream` (no `&mut` flood, no erasure tower).
//
// Probing at shallow depth keeps them ordinary `#[test]`s. The unbounded case is a COMPILE-time
// failure, covered by `dup_nests_to_arbitrary_depth_generically` below — that it compiles is the
// assertion.
// ===========================================================================

use std::any::type_name;
use std::sync::Mutex;

static SEEN: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn note<S: ?Sized>() {
    SEEN.lock().unwrap().push(type_name::<S>().to_string());
}

fn taken() -> Vec<String> {
    let mut v = std::mem::take(&mut *SEEN.lock().unwrap());
    v.sort();
    v.dedup();
    v
}

fn report(what: &str, seen: &[String]) -> String {
    format!(
        "{what}: {} distinct stream types, longest {} chars:\n  {}",
        seen.len(),
        seen.iter().map(String::len).max().unwrap_or(0),
        seen.join("\n  ")
    )
}

/// (2) `dup` wraps `Dup<&mut Self>`, and `Dup` is itself a `ParseStream`, so
/// each nesting level is a NEW stream type.
#[test]
fn dup_nesting_does_not_grow_the_stream_type() {
    let mut s = stream_of("abcd");
    note::<Stream>();
    let _: Result<(), ()> = s.dup(|a| {
        fn lvl<S: ParseStream<Atom = Atom>>(s: &mut S, depth: u32) {
            note::<S>();
            if depth > 0 {
                let _: Result<(), ()> = s.dup(|_d| Ok(()));
            }
        }
        lvl(a, 1);
        Ok(())
    });
    let seen = taken();
    let seen: Vec<String> = seen.into_iter().filter(|t| t.contains("Stream")).collect();
    assert_eq!(
        seen.len(),
        1,
        "{}",
        report("nesting dup must not change the stream type", &seen)
    );
}

/// The COMPILE-time half of the above, and the stronger statement: a
/// DEPTH-GENERIC recursive nest, which only type-checks if `dup`'s closure
/// parameter is a fixed point. This could not be a `#[test]` at all before the
/// checkpoint trio landed, because `dup` handed the closure `Dup<&mut Self>`:
///
///     error[E0275]: overflow evaluating the requirement
///       = note: required for `Dup<&mut Dup<&mut Dup<&mut ...>>>` to implement
///               `ParseStream`
///
/// The assertion at the end is almost beside the point — that this function
/// compiles is the test.
#[test]
fn dup_nests_to_arbitrary_depth_generically() {
    fn nest<S: ParseStream<Atom = Atom>>(s: &mut S, depth: u32) {
        if depth == 0 {
            return;
        }
        let _: Result<(), ()> = s.dup(|d| {
            nest(d, depth - 1);
            Ok(())
        });
    }
    let mut s = stream_of("abcd");
    nest(&mut s, 40);
    assert_eq!(
        drain(&mut s),
        "abcd",
        "40 nested dups that consume nothing and all commit are a no-op"
    );
}

/// (3) A recursive descent must instantiate **one** stream type, however deep it goes.
///
/// `Parse::parse` used to take `impl IntoParseStream` **by value**, so each level asked for
/// `parse::<&mut &mut …>` — an infinite monomorphisation chain. `#[recurse]` worked around it by
/// rewriting every recursive call's argument to `erase(…)`, pinning the callee to
/// `&mut dyn ParseStream`; the old version of this test asserted that the resulting type set was
/// merely *bounded* (two: the entry stream and the erased one).
///
/// `parse_stream` takes `&mut S` and recursion reborrows, so `S` is a genuine fixed point and the
/// answer is now **one**, not two — and there is no `dyn` in it. Delete the reborrow (pass
/// `&mut stream` where `stream: &mut S`) and this fails to compile with E0275, exactly as the
/// by-value version did.
#[test]
fn descent_instantiates_exactly_one_stream_type() {
    fn note_stream<S: ParseStream<Atom = Atom>>(_s: &mut S) {
        note::<S>();
    }
    // Depth-GENERIC. Under the old design this could not compile at all.
    fn descend<S: ParseStream<Atom = Atom>>(stream: &mut S, depth: u32) {
        note_stream(stream);
        let _ = stream.next();
        if depth > 0 {
            descend(&mut *stream, depth - 1); // reborrow: the callee is the SAME `S`
        }
    }

    let mut s = stream_of("abcdefghij");
    descend(&mut s, 3);
    let shallow = taken();
    let mut s = stream_of("abcdefghij");
    descend(&mut s, 30);
    let deep = taken();

    assert_eq!(
        shallow,
        deep,
        "the stream type must not depend on descent depth\n{}\n{}",
        report("depth 3", &shallow),
        report("depth 30", &deep)
    );
    assert_eq!(
        deep.len(),
        1,
        "{}",
        report("expected ONE stream type", &deep)
    );
    assert!(
        !deep[0].contains("dyn"),
        "the descent should be fully monomorphised, got {:?}",
        deep[0]
    );
}

/// (5) `WithSpan::parse` wraps the stream in a function-local `SubStream`
/// (span.rs:113), a THIRD wrapper alongside `Dup` and `&mut T`. Nesting
/// `WithSpan` inside `WithSpan` should not compound it.
#[test]
fn substream_does_not_nest() {
    struct Probe;
    impl syan::parse::Parse<Atom> for Probe {
        type Error = syan::error::ParseError<Span>;
        fn parse_stream<__S: syan::parse::ParseStream<Atom = Atom>>(
            stream: &mut __S,
        ) -> Result<Self, Self::Error> {
            note::<Atom>();
            let _ = stream.next();
            Ok(Probe)
        }
    }
    let _ = <WithSpan<Probe, Span> as syan::parse::Parse<Atom>>::parse(stream_of("ab"));
    let _ =
        <WithSpan<WithSpan<Probe, Span>, Span> as syan::parse::Parse<Atom>>::parse(stream_of("ab"));
    let seen = taken();
    assert!(
        seen.len() <= 1,
        "{}",
        report("SubStream must not compound when WithSpan nests", &seen)
    );
}

/// (6) The COST, not the mechanism: at realistic grammar depth the type names
/// themselves used to become enormous (measured in rustyfi-syntax: up to 5,129
/// chars, `.debug_str` 1.145 GB of a 2.4 GB rlib), because each `dup` level added
/// a `Dup<…>` wrapper. It was `#[ignore]`d as too slow to compile for that reason.
/// Since `dup` hands the closure `&mut Self`, all eight levels below are ONE type
/// and the monomorphization is trivial, so it runs by default.
#[test]
fn deep_nesting_keeps_type_names_bounded() {
    fn note_stream<S: ParseStream<Atom = Atom>>(_s: &mut S) {
        note::<S>();
    }
    // explicit 8-level nest (a depth-generic helper cannot compile — E0275)
    let mut s = stream_of("abcdefgh");
    note_stream(&mut s);
    let _: Result<(), ()> = s.dup(|a| {
        note_stream(a);
        let _: Result<(), ()> = a.dup(|b| {
            note_stream(b);
            let _: Result<(), ()> = b.dup(|c| {
                note_stream(c);
                let _: Result<(), ()> = c.dup(|d| {
                    note_stream(d);
                    let _: Result<(), ()> = d.dup(|e| {
                        note_stream(e);
                        let _: Result<(), ()> = e.dup(|f| {
                            note_stream(f);
                            Ok(())
                        });
                        Ok(())
                    });
                    Ok(())
                });
                Ok(())
            });
            Ok(())
        });
        Ok(())
    });
    let seen = taken();
    let longest = seen.iter().map(String::len).max().unwrap_or(0);
    assert!(
        longest < 120 && seen.len() <= 2,
        "{}\n(rustyfi-syntax measures 5,129-char names at real grammar depth)",
        report("deep nesting must keep type names bounded", &seen)
    );
}
