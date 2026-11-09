//! Every function in the design's table against a hand-computed reference (and the prototype's `t_derived` values).

use std::collections::HashMap;
use tlmbench_expr::{Env, Series, Val, eval_str, functions};

const NAN: f64 = f64::NAN;

#[derive(Default)]
struct M {
    ch: HashMap<String, Series>,
    labels: HashMap<(String, String), f64>,
    limits: HashMap<(String, bool), (Option<f64>, Option<f64>)>,
}
impl M {
    fn with(mut self, n: &str, v: &[f64], hz: f64) -> Self {
        self.ch.insert(n.into(), Series::rate(v.to_vec(), hz));
        self
    }
}
impl Env for M {
    fn series(&self, n: &str) -> Option<Series> {
        self.ch.get(n).cloned()
    }
    fn label(&self, c: &str, t: &str) -> Option<f64> {
        self.labels.get(&(c.into(), t.into())).copied()
    }
    fn limits(&self, c: &str, alarm: bool) -> Option<(Option<f64>, Option<f64>)> {
        self.limits.get(&(c.into(), alarm)).copied()
    }
}

fn same(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || (a - b).abs() < 1e-4
}
fn ser(src: &str, m: &M) -> Vec<f64> {
    match eval_str(src, m) {
        Ok(Val::Ser(s)) => s.v,
        other => panic!("{src}: {other:?}"),
    }
}
fn num(src: &str, m: &M) -> f64 {
    match eval_str(src, m) {
        Ok(v) => v.number().unwrap_or_else(|| panic!("{src}: not a number")),
        Err(e) => panic!("{src}: {e}"),
    }
}
fn is(src: &str, m: &M, want: &[f64]) {
    let got = ser(src, m);
    assert!(got.len() == want.len() && got.iter().zip(want).all(|(a, b)| same(*a, *b)), "{src}: {got:?} != {want:?}");
}
fn err(src: &str, m: &M) -> String {
    eval_str(src, m).unwrap_err().msg
}

fn x() -> M {
    M::default().with("x", &[10.0, 11.0, NAN, 12.1, 11.0], 1.0)
}

#[test]
fn prototype_values() {
    let m = x();
    is("delta(x)", &m, &[NAN, 1.0, NAN, 1.1, -1.1]);
    is("diff(x)", &m, &[NAN, 1.0, NAN, 1.1, -1.1]);
    is("pct_change(x) * 100", &m, &[NAN, 10.0, NAN, 10.0, -9.0909]);
    is("sma(x, 2)", &m, &[NAN, 10.5, NAN, 11.55, 11.55]);
    assert!(same(num("mean(x)", &m), 11.025));
    is("max(x, 11)", &m, &[11.0, 11.0, NAN, 12.1, 11.0]);
    is("\"Adj Close\" / 2", &M::default().with("Adj Close", &[4.0, 6.0], 1.0), &[2.0, 3.0]);
    is("x × 2 − 1", &M::default().with("x", &[1.0, 2.0], 1.0), &[1.0, 3.0]);
    is(
        "median(x, 3)",
        &M::default().with("x", &[1.0, 9.0, 2.0, 3.0, 100.0, 4.0], 1.0),
        &[NAN, NAN, 2.0, 3.0, 3.0, 4.0],
    );
    assert_eq!(err("x * (2 +", &m), "The expression ends too soon");
    assert_eq!(err("foo(x)", &m), "There's no function called foo");
}

#[test]
fn math_family() {
    let m = M::default().with("a", &[-4.0, 0.0, 9.0], 1.0).with("b", &[2.0, 3.0, 4.0], 1.0);
    is("abs(a)", &m, &[4.0, 0.0, 9.0]);
    is("sqrt(b * b)", &m, &[2.0, 3.0, 4.0]);
    is("ln(exp(b))", &m, &[2.0, 3.0, 4.0]);
    is("log(exp(b))", &m, &[2.0, 3.0, 4.0]);
    is("log10(100 * b)", &m, &[2.30103, 2.47712, 2.60206]);
    is("pow(b, 2)", &m, &[4.0, 9.0, 16.0]);
    is("b ^ 2 ^ 1", &m, &[4.0, 9.0, 16.0]);
    is("min(a, b)", &m, &[-4.0, 0.0, 4.0]);
    is("clamp(a, -1, 5)", &m, &[-1.0, 0.0, 5.0]);
    is("round(b / 3, 1)", &m, &[0.7, 1.0, 1.3]);
    is("floor(b / 3) + ceil(b / 3)", &m, &[1.0, 2.0, 3.0]);
    is("sign(a)", &m, &[-1.0, 0.0, 1.0]);
    is("deg(rad(b))", &m, &[2.0, 3.0, 4.0]);
    is("atan2(b, b) * 4 / pi", &m, &[1.0, 1.0, 1.0]);
    is("sin(0 * b) + cos(0 * b) + tan(0 * b)", &m, &[1.0, 1.0, 1.0]);
    is("asin(sin(b / 10)) + acos(cos(b / 10)) + atan(tan(b / 10))", &m, &[0.6, 0.9, 1.2]);
    is("if(a > 0, 1, -1)", &m, &[-1.0, -1.0, 1.0]);
    is("a % 3", &m, &[-1.0, 0.0, 0.0]);
    is("!(a > 0) && b < 4", &m, &[1.0, 1.0, 0.0]);
    is("a < 0 || b == 4", &m, &[1.0, 0.0, 1.0]);
    assert!(same(num("sqrt(16) + pi - pi", &m), 4.0));
    assert!(ser("sqrt(a)", &m)[0].is_nan());
}

#[test]
#[allow(clippy::approx_constant)]
fn sample_to_sample_and_rolling() {
    let m = M::default().with("v", &[1.0, 3.0, 2.0, 6.0], 2.0);
    is("prev(v)", &m, &[NAN, 1.0, 3.0, 2.0]);
    is("prev(v, 2)", &m, &[NAN, NAN, 1.0, 3.0]);
    is("rate(v)", &m, &[NAN, 4.0, -2.0, 8.0]);
    is("cumsum(v)", &m, &[1.0, 4.0, 6.0, 12.0]);
    is("integral(v)", &m, &[0.0, 1.0, 2.25, 4.25]);
    is("running_max(v)", &m, &[1.0, 3.0, 3.0, 6.0]);
    is("running_min(6 - v)", &m, &[5.0, 3.0, 3.0, 0.0]);
    is("rolling_mean(v, 2)", &m, &[NAN, 2.0, 2.5, 4.0]);
    is("rolling_sum(v, 2)", &m, &[NAN, 4.0, 5.0, 8.0]);
    is("rolling_min(v, 2)", &m, &[NAN, 1.0, 2.0, 2.0]);
    is("rolling_max(v, 2)", &m, &[NAN, 3.0, 3.0, 6.0]);
    is("rolling_std(v, 2)", &m, &[NAN, 1.41421, 0.70711, 2.82843]);
    // a time window: 1 s at 2 Hz holds samples within 1 s of each other (3 samples)
    is("rolling_sum(v, 1s)", &m, &[1.0, 4.0, 6.0, 11.0]);
    // ema over 3 samples: alpha 0.5
    is("ema(v, 3)", &m, &[1.0, 2.0, 2.0, 4.0]);
    // gaps are skipped: prev() of Monday is Friday
    let g = M::default().with("v", &[1.0, NAN, 4.0], 1.0);
    is("prev(v)", &g, &[NAN, NAN, 1.0]);
    is("integral(v)", &g, &[0.0, NAN, 5.0]);
    assert!(err("sma(v, 0.5)", &m).contains("the window is a number of samples"));
}

#[test]
fn one_number_family() {
    let m = M::default().with("v", &[1.0, 3.0, 2.0, 6.0], 2.0).with("c", &[0.0, 1.0, 1.0, 0.0, 1.0, NAN], 2.0);
    assert!(same(num("mean(v)", &m), 3.0));
    assert!(same(num("std(v)", &m), 2.16025));
    assert!(same(num("min(v)", &m), 1.0) && same(num("max(v)", &m), 6.0));
    assert!(same(num("first(v)", &m), 1.0) && same(num("last(v)", &m), 6.0));
    assert!(same(num("count(v)", &m), 4.0) && same(num("sum(v)", &m), 12.0));
    assert!(same(num("pct(c)", &m), 60.0));
    assert!(same(num("duration(c)", &m), 1.5));
    assert!(same(num("occurrences(c)", &m), 2.0));
    is("since(c)", &m, &[NAN, 0.0, 0.0, 0.5, 0.0, NAN]);
    assert!(same(num("mean(v) * 2", &m), 6.0));
    // a one-number result combines with a series as a flat line
    is("v - mean(v)", &m, &[-2.0, 0.0, -1.0, 3.0]);
}

#[test]
fn names_labels_limits() {
    let mut m = M::default().with("Adj Close", &[5.0], 1.0).with("state", &[0.0, 2.0, 2.0], 1.0).with(
        "t",
        &[10.0, 90.0, 50.0],
        1.0,
    );
    m.labels.insert(("state".into(), "flight".into()), 2.0);
    m.limits.insert(("t".into(), false), (None, Some(80.0)));
    is("\"Close|Adj Close\" + 1", &m, &[6.0]);
    is("state == 'flight'", &m, &[0.0, 1.0, 1.0]);
    is("'flight' != state", &m, &[1.0, 0.0, 0.0]);
    is("past_warn(t)", &m, &[0.0, 1.0, 0.0]);
    assert_eq!(err("state == 'nope'", &m), "state has no value called 'nope'");
    assert_eq!(err("past_alarm(t)", &m), "t has no alarm limits");
    assert_eq!(err("missing + 1", &m), "There's no channel called missing");
    assert_eq!(err("past_warn(t + 1)", &m), "past_warn() takes a channel, like past_warn(esc_temp_1)");
    assert_eq!(err("sma(3)", &m), "sma() needs a channel, not a number");
    assert_eq!(err("pow(1)", &m), "pow() takes 2 arguments");
}

#[test]
fn mixed_rates_hold_or_interpolate() {
    let m = M::default().with("slow", &[0.0, 10.0], 1.0).with("fast", &[0.0; 10], 10.0);
    // held: 0 for the first second, then 10
    let held = ser("slow + fast", &m);
    assert_eq!((held.len(), held[5], held[9]), (10, 0.0, 0.0));
    let lin = ser("interp(slow) + fast", &m);
    assert!(same(lin[5], 5.0) && same(lin[0], 0.0));
    // past the last slow sample interpolation has nothing to say; holding does
    let m = M::default().with("slow", &[0.0, 10.0], 1.0).with("fast", &[0.0; 21], 10.0);
    assert_eq!(ser("slow + fast", &m)[15], 10.0);
    assert!(ser("interp(slow) + fast", &m)[15].is_nan());
}

#[test]
fn filters() {
    let n = 400;
    let sig = |f1: f64, f2: f64| -> Vec<f64> {
        (0..n)
            .map(|i| {
                (2.0 * std::f64::consts::PI * f1 * i as f64 / 100.0).sin()
                    + (2.0 * std::f64::consts::PI * f2 * i as f64 / 100.0).sin()
            })
            .collect()
    };
    let m = M::default().with("x", &sig(1.0, 30.0), 100.0);
    let amp = |v: Vec<f64>| v[100..300].iter().fold(0.0f64, |a, b| a.max(b.abs()));
    let lo = amp(ser("lowpass(x, 5)", &m));
    let hi = amp(ser("highpass(x, 10)", &m));
    let bp = amp(ser("bandpass(x, 10, 40)", &m));
    assert!(lo > 0.8 && lo < 1.2, "{lo}");
    assert!(hi > 0.8 && hi < 1.2, "{hi}");
    assert!(bp > 0.7 && bp < 1.2, "{bp}");
    // zero phase: a slow sine passes the low-pass with no delay
    let slow = M::default().with("x", &sig(1.0, 1.0), 100.0);
    let out = ser("lowpass(x, 5)", &slow);
    let want = sig(1.0, 1.0);
    assert!((150..250).all(|i| (out[i] - want[i]).abs() < 0.1));
    assert!(err("lowpass(x, 80)", &m).contains("below half the sample rate"));
    assert!(err("lowpass(x, 0)", &m).contains("the cutoff is a frequency in Hz"));
    assert!(err("bandpass(x, 20, 10)", &m).contains("the second cutoff must be above the first"));
    is("movavg(x, 1)", &M::default().with("x", &[1.0, 2.0], 1.0), &[1.0, 2.0]);
}

#[test]
fn every_function_in_the_table_is_tested_here() {
    let src = include_str!("functions.rs");
    for (name, ..) in functions::TABLE {
        assert!(src.contains(&format!("{name}(")), "{name} has no test");
    }
}
