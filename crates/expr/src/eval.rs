//! Evaluation over whole series. Semantics (shared with the prototype): NaN is a gap that sample-to-sample and
//! rolling functions skip; series on different grids are held onto the longest one's; comparisons give 1 or 0.

use crate::ast::{BinOp, Expr, ExprError, UnOp};
use crate::filters;
use crate::series::Series;

/// What an expression reads from: channels by exact name, value labels and limits.
pub trait Env {
    fn series(&self, name: &str) -> Option<Series>;
    /// The stored number for value label `text` of `channel` (`state == 'flight'`).
    fn label(&self, _channel: &str, _text: &str) -> Option<f64> {
        None
    }
    /// A channel's (low, high) warn or alarm limits.
    fn limits(&self, _channel: &str, _alarm: bool) -> Option<(Option<f64>, Option<f64>)> {
        None
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Num(f64),
    Dur(f64),
    Ser(Series),
}

type R<T> = Result<T, ExprError>;

pub fn eval(e: &Expr, env: &dyn Env) -> R<Val> {
    match e {
        Expr::Num(v) => Ok(Val::Num(*v)),
        Expr::Dur { secs, .. } => Ok(Val::Dur(*secs)),
        Expr::Label { at, .. } => {
            Err(ExprError::new("A value name in quotes goes next to the channel it belongs to", *at))
        }
        Expr::Name { name, at } => {
            for alt in name.split('|').map(str::trim) {
                if let Some(mut s) = env.series(alt) {
                    s.ch = Some(alt.to_string());
                    return Ok(Val::Ser(s));
                }
            }
            Err(ExprError::new(format!("There's no channel called {name}"), *at))
        }
        Expr::Un { op, a } => {
            let a = eval(a, env)?;
            let f: fn(f64) -> f64 = match op {
                UnOp::Neg => |x| -x,
                UnOp::Not => |x| (x == 0.0) as i32 as f64,
            };
            elementwise(&[a], 0, |x| f(x[0]))
        }
        Expr::Bin { op, a, b } => {
            let (a, b) = labelled(*op, a, b, env)?;
            let f: fn(f64, f64) -> f64 = match op {
                BinOp::Add => |x, y| x + y,
                BinOp::Sub => |x, y| x - y,
                BinOp::Mul => |x, y| x * y,
                BinOp::Div => |x, y| x / y,
                BinOp::Rem => |x, y| x % y,
                BinOp::Pow => f64::powf,
                BinOp::Lt => |x, y| (x < y) as i32 as f64,
                BinOp::Le => |x, y| (x <= y) as i32 as f64,
                BinOp::Gt => |x, y| (x > y) as i32 as f64,
                BinOp::Ge => |x, y| (x >= y) as i32 as f64,
                BinOp::Eq => |x, y| (x == y) as i32 as f64,
                BinOp::Ne => |x, y| (x != y) as i32 as f64,
                BinOp::And => |x, y| (x != 0.0 && y != 0.0) as i32 as f64,
                BinOp::Or => |x, y| (x != 0.0 || y != 0.0) as i32 as f64,
            };
            elementwise(&[a, b], 0, |x| f(x[0], x[1]))
        }
        Expr::Call { f, args, at } => call(f, args, *at, env),
    }
}

/// Evaluates both sides of a binary operator, reading a `'label'` against the channel on its other side.
fn labelled(op: BinOp, a: &Expr, b: &Expr, env: &dyn Env) -> R<(Val, Val)> {
    let label_of = |lab: &Expr, other: &Expr| -> R<Option<Val>> {
        let Expr::Label { text, at } = lab else { return Ok(None) };
        let Some(&name) = other.names().first() else {
            return Err(ExprError::new("A value name in quotes needs a channel to compare with", *at));
        };
        let Val::Ser(s) = eval(&Expr::Name { name: name.to_string(), at: *at }, env)? else { unreachable!() };
        let ch = s.ch.as_deref().unwrap_or(name);
        match env.label(ch, text) {
            Some(v) => Ok(Some(Val::Num(v))),
            None => Err(ExprError::new(format!("{ch} has no value called '{text}'"), *at)),
        }
    };
    let cmp = matches!(op, BinOp::Eq | BinOp::Ne);
    if cmp {
        if let Some(v) = label_of(b, a)? {
            return Ok((eval(a, env)?, v));
        }
        if let Some(v) = label_of(a, b)? {
            return Ok((v, eval(b, env)?));
        }
    }
    Ok((eval(a, env)?, eval(b, env)?))
}

fn num_of(v: &Val) -> Option<f64> {
    match v {
        Val::Num(x) | Val::Dur(x) => Some(*x),
        Val::Ser(_) => None,
    }
}

/// Applies `f` over the arguments element by element. Series are held (or interpolated) onto the longest one's
/// grid; numbers stay numbers; any NaN in gives NaN out. `skip` arguments from the end are ignored for NaN tests.
pub fn elementwise(args: &[Val], _skip: usize, f: impl Fn(&[f64]) -> f64) -> R<Val> {
    let lead = args.iter().filter_map(|a| if let Val::Ser(s) = a { Some(s) } else { None }).max_by_key(|s| s.len());
    let Some(lead) = lead else {
        let xs: Vec<f64> = args.iter().map(|a| num_of(a).unwrap()).collect();
        return Ok(Val::Num(if xs.iter().any(|x| x.is_nan()) { f64::NAN } else { f(&xs) }));
    };
    let (grid, len) = (lead.grid.clone(), lead.len());
    let cols: Vec<Result<Vec<f64>, f64>> = args
        .iter()
        .map(|a| match a {
            Val::Ser(s) => Ok(s.onto(&grid, len)),
            v => Err(num_of(v).unwrap()),
        })
        .collect();
    let mut xs = vec![0.0; args.len()];
    let mut out = Vec::with_capacity(len);
    'row: for i in 0..len {
        for (j, c) in cols.iter().enumerate() {
            let x = match c {
                Ok(v) => v[i],
                Err(k) => *k,
            };
            if x.is_nan() {
                out.push(f64::NAN);
                continue 'row;
            }
            xs[j] = x;
        }
        out.push(f(&xs));
    }
    Ok(Val::Ser(Series::new(out, grid)))
}

fn series_arg<'a>(v: &'a Val, f: &str, at: usize) -> R<&'a Series> {
    match v {
        Val::Ser(s) => Ok(s),
        _ => Err(ExprError::new(format!("{f}() needs a channel, not a number"), at)),
    }
}

/// A window: samples (20) or seconds (30d).
#[derive(Clone, Copy, Debug)]
pub enum Window {
    Samples(usize),
    Secs(f64),
}

fn window(v: Option<&Val>, f: &str, at: usize) -> R<Window> {
    match v {
        None => Ok(Window::Samples(20)),
        Some(Val::Dur(s)) => Ok(Window::Secs(*s)),
        Some(Val::Num(n)) if *n >= 1.0 => Ok(Window::Samples(n.round() as usize)),
        _ => Err(ExprError::new(format!("{f}: the window is a number of samples (20) or a time (30d, 5s)"), at)),
    }
}

/// The non-gap samples of `s`: their indices, values and times.
fn compact(s: &Series) -> (Vec<usize>, Vec<f64>, Vec<f64>) {
    let idx: Vec<usize> = (0..s.len()).filter(|&i| !s.v[i].is_nan()).collect();
    let v = idx.iter().map(|&i| s.v[i]).collect();
    let t = idx.iter().map(|&i| s.time(i)).collect();
    (idx, v, t)
}

/// Runs `f(values, times)` over the non-gap samples; gaps stay gaps.
pub fn sample_op(s: &Series, f: impl FnOnce(&[f64], &[f64]) -> Vec<f64>) -> Val {
    let (idx, v, t) = compact(s);
    let res = f(&v, &t);
    let mut out = vec![f64::NAN; s.len()];
    for (j, &i) in idx.iter().enumerate() {
        out[i] = res[j];
    }
    Val::Ser(Series::new(out, s.grid.clone()))
}

fn metric(s: &Series, f: impl FnOnce(&[f64]) -> f64) -> Val {
    let v: Vec<f64> = s.v.iter().copied().filter(|x| !x.is_nan()).collect();
    Val::Num(if v.is_empty() { f64::NAN } else { f(&v) })
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

fn rolling(v: &[f64], t: &[f64], w: Window, kind: Roll) -> Vec<f64> {
    let mut out = vec![f64::NAN; v.len()];
    let (mut lo, mut s, mut s2) = (0usize, 0.0, 0.0);
    for j in 0..v.len() {
        s += v[j];
        s2 += v[j] * v[j];
        match w {
            Window::Samples(n) => {
                if j >= n {
                    s -= v[j - n];
                    s2 -= v[j - n] * v[j - n];
                    lo = j - n + 1;
                }
            }
            Window::Secs(sec) => {
                while t[j] - t[lo] > sec {
                    s -= v[lo];
                    s2 -= v[lo] * v[lo];
                    lo += 1;
                }
            }
        }
        let n = j - lo + 1;
        if matches!(w, Window::Samples(k) if n < k) {
            continue;
        }
        out[j] = match kind {
            Roll::Mean => s / n as f64,
            Roll::Sum => s,
            Roll::Std if n > 1 => ((s2 - s * s / n as f64) / (n as f64 - 1.0)).max(0.0).sqrt(),
            Roll::Std => f64::NAN,
            Roll::Min => v[lo..=j].iter().copied().fold(f64::INFINITY, f64::min),
            Roll::Max => v[lo..=j].iter().copied().fold(f64::NEG_INFINITY, f64::max),
            Roll::Median => {
                let mut win = v[lo..=j].to_vec();
                win.sort_by(f64::total_cmp);
                let m = win.len() / 2;
                if win.len() % 2 == 1 { win[m] } else { (win[m - 1] + win[m]) / 2.0 }
            }
        };
    }
    out
}

#[derive(Clone, Copy)]
enum Roll {
    Mean,
    Sum,
    Std,
    Min,
    Max,
    Median,
}

fn call(f: &str, args: &[Expr], at: usize, env: &dyn Env) -> R<Val> {
    let (lo, hi) = crate::functions::arity(f).expect("the parser checks function names");
    if args.len() < lo || args.len() > hi {
        let want = if lo == hi { format!("{lo}") } else { format!("{lo} to {hi}") };
        return Err(ExprError::new(format!("{f}() takes {want} argument{}", if hi == 1 { "" } else { "s" }), at));
    }
    let a: Vec<Val> = args.iter().map(|e| eval(e, env)).collect::<R<_>>()?;
    let ew = |g: &dyn Fn(&[f64]) -> f64| elementwise(&a, 0, g);
    let ser = |i: usize| series_arg(&a[i], f, at);
    let win = |i: usize| window(a.get(i), f, at);
    let roll = |kind: Roll| -> R<Val> {
        let w = win(1)?;
        Ok(sample_op(ser(0)?, |v, t| rolling(v, t, w, kind)))
    };
    use std::f64::consts::PI;
    Ok(match f {
        "abs" => ew(&|x| x[0].abs())?,
        "sqrt" => ew(&|x| x[0].sqrt())?,
        "ln" | "log" => ew(&|x| x[0].ln())?,
        "log10" => ew(&|x| x[0].log10())?,
        "exp" => ew(&|x| x[0].exp())?,
        "pow" => ew(&|x| x[0].powf(x[1]))?,
        "floor" => ew(&|x| x[0].floor())?,
        "ceil" => ew(&|x| x[0].ceil())?,
        "sign" => ew(&|x| if x[0] == 0.0 { 0.0 } else { x[0].signum() })?,
        "round" => {
            let q = 10f64.powf(a.get(1).and_then(num_of).unwrap_or(0.0));
            elementwise(&a[..1], 0, |x| (x[0] * q).round() / q)?
        }
        "clamp" => ew(&|x| x[0].max(x[1]).min(x[2]))?,
        "sin" => ew(&|x| x[0].sin())?,
        "cos" => ew(&|x| x[0].cos())?,
        "tan" => ew(&|x| x[0].tan())?,
        "asin" => ew(&|x| x[0].asin())?,
        "acos" => ew(&|x| x[0].acos())?,
        "atan" => ew(&|x| x[0].atan())?,
        "atan2" => ew(&|x| x[0].atan2(x[1]))?,
        "deg" => ew(&|x| x[0] * 180.0 / PI)?,
        "rad" => ew(&|x| x[0] * PI / 180.0)?,
        "if" => ew(&|x| if x[0] != 0.0 { x[1] } else { x[2] })?,
        // two or more arguments: element by element; one channel: the whole file's min or max
        "min" | "max" => {
            let is_min = f == "min";
            if a.len() == 1 {
                match &a[0] {
                    Val::Ser(s) => metric(s, |v| {
                        v.iter().copied().fold(
                            if is_min { f64::INFINITY } else { f64::NEG_INFINITY },
                            if is_min { f64::min } else { f64::max },
                        )
                    }),
                    n => n.clone(),
                }
            } else if is_min {
                ew(&|x| x.iter().copied().fold(f64::INFINITY, f64::min))?
            } else {
                ew(&|x| x.iter().copied().fold(f64::NEG_INFINITY, f64::max))?
            }
        }
        "delta" | "diff" => {
            sample_op(ser(0)?, |v, _| (0..v.len()).map(|j| if j > 0 { v[j] - v[j - 1] } else { f64::NAN }).collect())
        }
        "prev" => {
            let n = a.get(1).and_then(num_of).unwrap_or(1.0).max(0.0) as usize;
            sample_op(ser(0)?, |v, _| (0..v.len()).map(|j| if j >= n { v[j - n] } else { f64::NAN }).collect())
        }
        "pct_change" => sample_op(ser(0)?, |v, _| {
            (0..v.len()).map(|j| if j > 0 && v[j - 1] != 0.0 { v[j] / v[j - 1] - 1.0 } else { f64::NAN }).collect()
        }),
        "rate" => sample_op(ser(0)?, |v, t| {
            (0..v.len())
                .map(|j| if j > 0 && t[j] > t[j - 1] { (v[j] - v[j - 1]) / (t[j] - t[j - 1]) } else { f64::NAN })
                .collect()
        }),
        "cumsum" => sample_op(ser(0)?, |v, _| {
            let mut s = 0.0;
            v.iter()
                .map(|x| {
                    s += x;
                    s
                })
                .collect()
        }),
        "integral" => sample_op(ser(0)?, |v, t| {
            let mut s = 0.0;
            (0..v.len())
                .map(|j| {
                    if j > 0 {
                        s += (v[j] + v[j - 1]) / 2.0 * (t[j] - t[j - 1]);
                    }
                    s
                })
                .collect()
        }),
        "running_max" => sample_op(ser(0)?, |v, _| {
            let mut m = f64::NEG_INFINITY;
            v.iter()
                .map(|&x| {
                    m = m.max(x);
                    m
                })
                .collect()
        }),
        "running_min" => sample_op(ser(0)?, |v, _| {
            let mut m = f64::INFINITY;
            v.iter()
                .map(|&x| {
                    m = m.min(x);
                    m
                })
                .collect()
        }),
        "sma" | "rolling_mean" | "movavg" => roll(Roll::Mean)?,
        "rolling_std" => roll(Roll::Std)?,
        "rolling_min" => roll(Roll::Min)?,
        "rolling_max" => roll(Roll::Max)?,
        "rolling_sum" => roll(Roll::Sum)?,
        "median" => roll(Roll::Median)?,
        "ema" => {
            let w = win(1)?;
            sample_op(ser(0)?, |v, t| {
                let mut e = f64::NAN;
                (0..v.len())
                    .map(|j| {
                        let al = match w {
                            Window::Samples(n) => 2.0 / (n as f64 + 1.0),
                            Window::Secs(s) if j > 0 => 1.0 - (-(t[j] - t[j - 1]) / s).exp(),
                            Window::Secs(_) => 1.0,
                        };
                        e = if e.is_nan() { v[j] } else { e + al * (v[j] - e) };
                        e
                    })
                    .collect()
            })
        }
        "lowpass" | "highpass" => filters::pass(ser(0)?, f == "lowpass", num_of(&a[1]), f, at)?,
        "bandpass" => {
            let (f1, f2) = (num_of(&a[1]), num_of(&a[2]));
            if !matches!((f1, f2), (Some(x), Some(y)) if y > x) {
                return Err(ExprError::new(
                    "bandpass(x, low Hz, high Hz): the second cutoff must be above the first",
                    at,
                ));
            }
            let Val::Ser(hi) = filters::pass(ser(0)?, false, f1, f, at)? else { unreachable!() };
            filters::pass(&hi, true, f2, f, at)?
        }
        "interp" => {
            let mut s = ser(0)?.clone();
            s.linear = true;
            Val::Ser(s)
        }
        "mean" => one(&a[0], mean),
        "sum" => one(&a[0], |v| v.iter().sum()),
        "std" => one(&a[0], |v| {
            let m = mean(v);
            if v.len() > 1 {
                (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (v.len() as f64 - 1.0)).sqrt()
            } else {
                f64::NAN
            }
        }),
        "first" => one(&a[0], |v| v[0]),
        "last" => one(&a[0], |v| v[v.len() - 1]),
        "count" => one(&a[0], |v| v.len() as f64),
        "pct" => one(&a[0], |v| 100.0 * v.iter().filter(|x| **x != 0.0).count() as f64 / v.len() as f64),
        "duration" => {
            let s = ser(0)?;
            let (mut secs, mut seen) = (0.0, false);
            for (i, x) in s.v.iter().enumerate() {
                if !x.is_nan() {
                    seen = true;
                    if *x != 0.0 {
                        secs += s.grid.step(i, s.len());
                    }
                }
            }
            Val::Num(if seen { secs } else { f64::NAN })
        }
        "occurrences" => {
            let (mut n, mut seen, mut was) = (0, false, false);
            for x in ser(0)?.v.iter().filter(|x| !x.is_nan()) {
                seen = true;
                n += (*x != 0.0 && !was) as i32;
                was = *x != 0.0;
            }
            Val::Num(if seen { n as f64 } else { f64::NAN })
        }
        // seconds since the condition last held; a gap in the condition stays a gap, before the first hit it is a gap
        "since" => {
            let s = ser(0)?;
            let mut last = f64::NAN;
            let v = (0..s.len())
                .map(|i| {
                    let x = s.v[i];
                    if x.is_nan() {
                        return f64::NAN;
                    }
                    if x != 0.0 {
                        last = s.time(i);
                    }
                    s.time(i) - last
                })
                .collect();
            Val::Ser(Series::new(v, s.grid.clone()))
        }
        "past_warn" | "past_alarm" => {
            let s = ser(0)?;
            let alarm = f == "past_alarm";
            let kind = if alarm { "alarm" } else { "warn" };
            let Some(ch) = s.ch.as_deref() else {
                return Err(ExprError::new(format!("{f}() takes a channel, like {f}(esc_temp_1)"), at));
            };
            let (lo, hi) = env
                .limits(ch, alarm)
                .filter(|(l, h)| l.is_some() || h.is_some())
                .ok_or_else(|| ExprError::new(format!("{ch} has no {kind} limits"), at))?;
            elementwise(&a, 0, |x| (lo.is_some_and(|l| x[0] <= l) || hi.is_some_and(|h| x[0] >= h)) as i32 as f64)?
        }
        _ => unreachable!("{f} is in the table but not evaluated"),
    })
}

fn one(v: &Val, f: impl FnOnce(&[f64]) -> f64) -> Val {
    match v {
        Val::Ser(s) => metric(s, f),
        n => n.clone(),
    }
}

/// Convenience: parse and evaluate.
pub fn eval_str(src: &str, env: &dyn Env) -> R<Val> {
    eval(&crate::parse(src)?, env)
}

impl Val {
    pub fn series(&self) -> Option<&Series> {
        if let Val::Ser(s) = self { Some(s) } else { None }
    }
    pub fn number(&self) -> Option<f64> {
        num_of(self)
    }
}
