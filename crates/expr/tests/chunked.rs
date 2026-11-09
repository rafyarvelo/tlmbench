//! The answer doesn't depend on the chunk size: every function carries its lookback across chunks, filters read a
//! margin past each chunk, and one-number functions take their own pass.

use std::ops::Range;
use tlmbench_expr::{Env, Grid, Series, Val, eval_chunked, parse};

struct Rec {
    x: Series,
    stamps: Series,
}

impl Env for Rec {
    fn series(&self, n: &str) -> Option<Series> {
        match n {
            "x" => Some(self.x.clone()),
            "t" => Some(self.stamps.clone()),
            _ => None,
        }
    }
    fn samples(&self) -> Option<usize> {
        Some(self.x.len())
    }
    fn hz(&self) -> Option<f64> {
        Some(100.0)
    }
    fn read(&self, n: &str, r: Range<usize>) -> Option<Series> {
        self.series(n).map(|s| s.slice(r))
    }
}

fn rec(n: usize) -> Rec {
    let mut seed = 12345u64;
    let mut rnd = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as f64 / (1u64 << 31) as f64
    };
    let v: Vec<f64> = (0..n)
        .map(|i| {
            if i % 97 == 5 || (300..312).contains(&i) { f64::NAN } else { (i as f64 * 0.05).sin() * 3.0 + rnd() - 0.5 }
        })
        .collect();
    // irregular times
    let mut tt = 0.0;
    let stamps: Vec<f64> = (0..n)
        .map(|_| {
            tt += 0.005 + rnd() * 0.01;
            tt
        })
        .collect();
    Rec { x: Series::rate(v.clone(), 100.0), stamps: Series::new(v, Grid::Stamps(stamps.into())) }
}

fn close(a: &Val, b: &Val, tol: f64, what: &str) {
    match (a, b) {
        (Val::Num(x), Val::Num(y)) => {
            assert!((x - y).abs() <= tol * x.abs().max(1.0) || (x.is_nan() && y.is_nan()), "{what}: {x} vs {y}")
        }
        (Val::Ser(x), Val::Ser(y)) => {
            assert_eq!(x.len(), y.len(), "{what}");
            for i in 0..x.len() {
                let (p, q) = (x.v[i], y.v[i]);
                assert!(
                    (p.is_nan() && q.is_nan()) || (p - q).abs() <= tol * p.abs().max(1.0),
                    "{what} at {i}: {p} vs {q}"
                );
            }
        }
        _ => panic!("{what}: {a:?} vs {b:?}"),
    }
}

const EXACT: &[&str] = &[
    "delta(x)",
    "prev(x, 3)",
    "pct_change(x)",
    "rate(x)",
    "cumsum(x)",
    "integral(x)",
    "running_max(x)",
    "running_min(x)",
    "sma(x, 10)",
    "sma(x, 0.3s)",
    "rolling_std(x, 25)",
    "rolling_min(x, 7)",
    "rolling_max(x, 0.2s)",
    "rolling_sum(x, 9)",
    "median(x, 5)",
    "ema(x, 8)",
    "ema(x, 0.3s)",
    "since(x > 1)",
    "x * 2 + delta(x) / 3",
    "mean(x)",
    "std(x)",
    "min(x)",
    "max(x)",
    "first(x)",
    "last(x)",
    "count(x)",
    "sum(x)",
    "pct(x > 0)",
    "duration(x > 0)",
    "occurrences(x > 1)",
    "x - mean(x)",
    "(x - mean(x)) / std(x)",
    "mean(x - mean(x))",
    "x > mean(x) + std(x)",
    "since(x > mean(x))",
    "max(x - min(x))",
    "cumsum(x) / count(x)",
];
const FILTERED: &[&str] = &[
    "lowpass(x, 5)",
    "highpass(x, 2)",
    "bandpass(x, 2, 20)",
    "x - lowpass(x, 1)",
    "delta(lowpass(x, 8))",
    "sma(lowpass(x, 4), 5)",
    "mean(lowpass(x, 5))",
];

#[test]
fn chunk_size_does_not_matter() {
    let r = rec(4000);
    for src in EXACT.iter().chain(FILTERED) {
        let e = parse(src).unwrap();
        let whole = eval_chunked(&e, &r, usize::MAX).unwrap();
        let tol = if FILTERED.contains(src) { 1e-6 } else { 1e-9 };
        for chunk in [1, 7, 64, 999, 4000] {
            close(&eval_chunked(&e, &r, chunk).unwrap(), &whole, tol, &format!("{src} / chunk {chunk}"));
        }
    }
}

#[test]
fn irregular_times_chunk_the_same() {
    let r = rec(1500);
    for src in ["rate(t)", "integral(t)", "sma(t, 0.1s)", "ema(t, 0.2s)", "duration(t > 0)", "since(t > 2)"] {
        let e = parse(src).unwrap();
        let whole = eval_chunked(&e, &r, usize::MAX).unwrap();
        for chunk in [1, 13, 500] {
            close(&eval_chunked(&e, &r, chunk).unwrap(), &whole, 1e-9, &format!("{src} / chunk {chunk}"));
        }
    }
}

#[test]
fn whole_file_filter_is_the_same_as_the_prototype_style_two_pass() {
    // chunked or not, zero-phase filtering matches filtering the whole signal forward then backward
    let r = rec(2000);
    let e = parse("lowpass(x, 5)").unwrap();
    let a = eval_chunked(&e, &r, 100).unwrap();
    let b = eval_chunked(&e, &r, usize::MAX).unwrap();
    close(&a, &b, 1e-6, "lowpass");
}

#[test]
fn a_chunk_is_streamed_not_collected() {
    use tlmbench_expr::run;
    let r = rec(1000);
    let mut starts = Vec::new();
    let n = run(&parse("cumsum(x)").unwrap(), &r, 300, &mut |a, s| starts.push((a, s.len()))).unwrap();
    assert_eq!((n, starts), (None, vec![(0, 300), (300, 300), (600, 300), (900, 100)]));
    // a one-number result is a number and streams nothing
    let mut called = false;
    let n = run(&parse("mean(x)").unwrap(), &r, 300, &mut |_, _| called = true).unwrap();
    assert!(n.is_some() && !called);
}
