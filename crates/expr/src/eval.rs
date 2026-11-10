//! Chunked evaluation. An expression is evaluated one chunk of samples at a time. Each function carries what it needs
//! from earlier chunks (ring buffers, running sums, filter state), so the answer does not depend on the chunk size.
//!
//! - Zero-phase filters need samples after the chunk: the chunk is read with a margin ([`filters::margin`]); every
//!   function runs over the margin too, but only the committed part changes its carried state.
//! - One-number functions (`mean`, `pct`, …) take a pass over the data each, one pass per level of nesting; the last
//!   pass produces the output.
//!
//! Semantics shared with the prototype: NaN is a gap that sample-to-sample and rolling functions skip; series on
//! different grids are held onto the longest one's; comparisons give 1 or 0.

use std::collections::VecDeque;
use std::ops::Range;

use crate::ast::{BinOp, Expr, ExprError, UnOp};
use crate::filters::{self, Biquad, Carry};
use crate::series::{Grid, Series};

/// What an expression reads from: channels by exact name, value labels and limits.
pub trait Env {
    /// The whole channel.
    fn series(&self, name: &str) -> Option<Series>;
    /// Samples `r` of a channel (a recording reads just those).
    fn read(&self, name: &str, r: Range<usize>) -> Option<Series> {
        self.series(name).map(|s| s.slice(r))
    }
    /// Samples per channel, when known: chunked evaluation needs it. With `None` everything is one chunk.
    fn samples(&self) -> Option<usize> {
        None
    }
    /// The channels' sample rate in Hz (about, for irregular times): sizes a filter's margin.
    fn hz(&self) -> Option<f64> {
        None
    }
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

impl Val {
    pub fn series(&self) -> Option<&Series> {
        if let Val::Ser(s) = self { Some(s) } else { None }
    }
    pub fn number(&self) -> Option<f64> {
        num_of(self)
    }
}

type R<T> = Result<T, ExprError>;

/// Samples per chunk unless told otherwise.
pub const CHUNK: usize = 1 << 20;

/// Parse and evaluate in one chunk.
pub fn eval_str(src: &str, env: &dyn Env) -> R<Val> {
    eval(&crate::parse(src)?, env)
}

/// Evaluates over the whole recording, collecting the output.
pub fn eval(e: &Expr, env: &dyn Env) -> R<Val> {
    eval_chunked(e, env, usize::MAX)
}

/// [`eval`] with chunks of `chunk` samples (the result is the same for every chunk size).
pub fn eval_chunked(e: &Expr, env: &dyn Env, chunk: usize) -> R<Val> {
    let (mut v, mut grid, mut stamps): (Vec<f64>, Option<Grid>, Vec<f64>) = (Vec::new(), None, Vec::new());
    let mut rate = true;
    let number = run(e, env, chunk, &mut |_, s| {
        v.extend_from_slice(&s.v);
        match &s.grid {
            Grid::Rate { .. } => {
                grid.get_or_insert_with(|| s.grid.clone());
                stamps.extend((0..s.len()).map(|i| s.time(i)));
            }
            Grid::Stamps(t) => {
                rate = false;
                stamps.extend(t.iter());
            }
        }
    })?;
    if let Some(x) = number {
        return Ok(Val::Num(x));
    }
    let grid = match grid {
        Some(g) if rate => g,
        _ => Grid::Stamps(stamps.into()),
    };
    Ok(Val::Ser(Series::new(v, grid)))
}

/// Whether every sample of the result depends only on the inputs at that sample (no rolling, filter, running or
/// one-number function): then any range can be computed alone, in any order, and in parallel with others.
pub fn is_stateless(e: &Expr) -> bool {
    analyze(e, None).is_ok_and(|p| p.stateless)
}

/// Samples `r` of the result of a [stateless](is_stateless) expression, computed alone. A number when the expression
/// is one.
pub fn eval_range(e: &Expr, env: &dyn Env, r: Range<usize>) -> R<Val> {
    let plan = analyze(e, env.hz())?;
    if !plan.stateless {
        return Err(ExprError::new(
            "This expression carries values from one sample to the next: it can't be computed in pieces",
            0,
        ));
    }
    let mut st = vec![St::default(); plan.nodes.len()];
    let mut cx =
        Ctx { env, commit: r.len(), range: r, pass: 0, cursor: 0, st: &mut st, plan: &plan, resolved: &[], warm: 0 };
    ev(e, &mut cx)
}

/// The number of samples to replay before a chunk of a recipe whose only state is windows of samples, so that the
/// chunk can be computed on its own (and in parallel with the others). `None` for any other recipe.
pub fn warm_rows(e: &Expr) -> Option<usize> {
    analyze(e, None).ok().filter(|p| p.warmable && !p.stateless && p.depth == 0).map(|p| p.warm)
}

/// [`warm_rows`] for a recording sampled at about `hz`, which also settles recipes with time windows (`rolling_mean(x,
/// 50ms)`): a live trigger replays that many samples before the new ones.
pub fn warm_rows_at(e: &Expr, hz: f64) -> Option<usize> {
    analyze(e, Some(hz)).ok().filter(|p| p.warmable && !p.stateless && p.depth == 0).map(|p| p.warm)
}

/// Samples `r` of a recipe with a [warm-up](warm_rows), computed alone from `warm` samples before it. `None` when a
/// window needed more real samples than the replay held (gaps), and the recipe must then run in one piece.
pub fn eval_warm(e: &Expr, env: &dyn Env, r: Range<usize>, warm: usize) -> R<Option<Val>> {
    let plan = analyze(e, env.hz())?;
    let from = r.start.saturating_sub(warm);
    let mut st = vec![St::default(); plan.nodes.len()];
    let n = r.len();
    let mut cx = Ctx {
        env,
        range: from..r.end,
        commit: r.end - from,
        pass: 0,
        cursor: 0,
        st: &mut st,
        plan: &plan,
        resolved: &[],
        warm: r.start - from,
    };
    let v = ev(e, &mut cx)?;
    if st.iter().any(|s| s.short) {
        return Ok(None);
    }
    Ok(Some(match v {
        Val::Ser(s) => Val::Ser(s.slice(r.start - from..r.start - from + n)),
        other => other,
    }))
}

/// Streams the expression's output chunk by chunk: `sink(start, chunk)`. Returns the number when the expression's
/// result is one (a flat line), and then never calls `sink`.
pub fn run(e: &Expr, env: &dyn Env, chunk: usize, sink: &mut dyn FnMut(usize, &Series)) -> R<Option<f64>> {
    let plan = analyze(e, env.hz())?;
    let mut resolved: Vec<Option<f64>> = vec![None; plan.nodes.len()];
    for pass in 0..=plan.depth {
        let last = pass == plan.depth;
        let mut st = vec![St::default(); plan.nodes.len()];
        let mut number = None;
        let len = env.samples();
        let mut a = 0usize;
        loop {
            let (b, end) = match len {
                Some(l) => {
                    let b = a.saturating_add(chunk).min(l);
                    (b, b.saturating_add(plan.ahead).min(l))
                }
                None => (usize::MAX, usize::MAX),
            };
            let mut cx = Ctx {
                env,
                range: a..end,
                commit: b.saturating_sub(a),
                pass,
                cursor: 0,
                st: &mut st,
                plan: &plan,
                resolved: &resolved,
                warm: 0,
            };
            let val = ev(e, &mut cx)?;
            if last {
                match val {
                    Val::Num(x) | Val::Dur(x) => number = Some(x),
                    Val::Ser(s) => sink(a, &s.slice(0..cx.commit)),
                }
            }
            match len {
                Some(l) if b < l => a = b,
                _ => break,
            }
        }
        if last {
            return Ok(number);
        }
        for (id, n) in plan.nodes.iter().enumerate() {
            if n.metric.is_some_and(|(d, _)| d == pass + 1) {
                resolved[id] = Some(finish(n.metric.unwrap().1, &st[id]));
            }
        }
    }
    unreachable!()
}

// ---------------------------------------------------------------------------------------------------------------
// The plan: which calls are one-number functions (and how deeply nested), and how far past a chunk to read.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Metric {
    Mean,
    Sum,
    Std,
    Min,
    Max,
    First,
    Last,
    Count,
    Pct,
    Duration,
    Occurrences,
}

fn metric_kind(f: &str, nargs: usize) -> Option<Metric> {
    Some(match f {
        "mean" => Metric::Mean,
        "sum" => Metric::Sum,
        "std" => Metric::Std,
        "min" if nargs == 1 => Metric::Min,
        "max" if nargs == 1 => Metric::Max,
        "first" => Metric::First,
        "last" => Metric::Last,
        "count" => Metric::Count,
        "pct" => Metric::Pct,
        "duration" => Metric::Duration,
        "occurrences" => Metric::Occurrences,
        _ => return None,
    })
}

#[derive(Default)]
struct Node {
    /// For a one-number function: its nesting depth (1 = no one-number function inside) and kind.
    metric: Option<(usize, Metric)>,
    /// Calls inside this one.
    inner: usize,
}

/// Functions whose output at a sample depends only on the inputs at that sample.
fn is_elementwise(f: &str) -> bool {
    matches!(
        f,
        "abs"
            | "sqrt"
            | "ln"
            | "log"
            | "log10"
            | "exp"
            | "pow"
            | "floor"
            | "ceil"
            | "sign"
            | "round"
            | "clamp"
            | "sin"
            | "cos"
            | "tan"
            | "asin"
            | "acos"
            | "atan"
            | "atan2"
            | "deg"
            | "rad"
            | "if"
            | "interp"
            | "past_warn"
            | "past_alarm"
    ) || (matches!(f, "min" | "max"))
}

#[derive(Default)]
struct Plan {
    /// Every call is element by element, and there is no one-number function: any chunk can be computed alone.
    stateless: bool,
    /// One per call, in the order evaluation meets them.
    nodes: Vec<Node>,
    depth: usize,
    /// Samples read past the end of a chunk.
    ahead: usize,
    /// Whether the only state is windows of samples (rolling min, max, mean, sum, std and median): a chunk can then
    /// be computed alone from `warm` samples before it.
    warmable: bool,
    warm: usize,
}

struct NoEnv;
impl Env for NoEnv {
    fn series(&self, _: &str) -> Option<Series> {
        None
    }
}

fn analyze(e: &Expr, hz: Option<f64>) -> R<Plan> {
    fn walk(e: &Expr, plan: &mut Plan, hz: Option<f64>) -> R<(usize, usize)> {
        match e {
            Expr::Un { a, .. } => walk(a, plan, hz),
            Expr::Bin { a, b, .. } => {
                let (x, y) = (walk(a, plan, hz)?, walk(b, plan, hz)?);
                Ok((x.0.max(y.0), x.1.max(y.1)))
            }
            Expr::Call { f, args, .. } => {
                let id = plan.nodes.len();
                plan.nodes.push(Node::default());
                let (mut depth, mut ahead) = (0, 0);
                for a in args {
                    let (d, h) = walk(a, plan, hz)?;
                    (depth, ahead) = (depth.max(d), ahead.max(h));
                }
                let kind = metric_kind(f, args.len());
                if kind.is_some() || !is_elementwise(f) {
                    plan.stateless = false;
                    let rolling = matches!(
                        f.as_str(),
                        "sma"
                            | "rolling_mean"
                            | "movavg"
                            | "rolling_std"
                            | "rolling_min"
                            | "rolling_max"
                            | "rolling_sum"
                            | "median"
                    );
                    let w = match args.get(1) {
                        Some(Expr::Dur { secs, .. }) if rolling && kind.is_none() => Some(Window::Secs(*secs)),
                        Some(a) => match eval(a, &NoEnv) {
                            Ok(v) if rolling && kind.is_none() => window(Some(&v), f, 0).ok(),
                            _ => None,
                        },
                        None => None,
                    };
                    match (w, hz) {
                        (Some(Window::Samples(n)), _) => plan.warm += n,
                        // a time window is that long at the recording's rate
                        (Some(Window::Secs(t)), Some(fs)) if fs > 0.0 => plan.warm += (t * fs).ceil() as usize + 1,
                        _ => plan.warmable = false,
                    }
                }
                plan.nodes[id].inner = plan.nodes.len() - id - 1;
                if let Some(k) = kind {
                    depth += 1;
                    plan.nodes[id].metric = Some((depth, k));
                }
                // cutoffs are constants; a bad one is reported when the call runs
                let cut = |i: usize| match args.get(i).map(|a| eval(a, &NoEnv)) {
                    Some(Ok(v)) => v.number(),
                    _ => None,
                };
                let own = match (f.as_str(), hz) {
                    ("lowpass" | "highpass", Some(fs)) => {
                        cut(1).filter(|c| *c > 0.0 && *c < fs / 2.0).map_or(0, |c| filters::margin(c, fs))
                    }
                    ("bandpass", Some(fs)) => [cut(1), cut(2)]
                        .iter()
                        .flatten()
                        .filter(|c| **c > 0.0 && **c < fs / 2.0)
                        .map(|c| filters::margin(*c, fs))
                        .sum(),
                    _ => 0,
                };
                Ok((depth, ahead + own))
            }
            _ => Ok((0, 0)),
        }
    }
    let mut plan = Plan { stateless: true, warmable: true, ..Plan::default() };
    (plan.depth, plan.ahead) = walk(e, &mut plan, hz)?;
    Ok(plan)
}

// ---------------------------------------------------------------------------------------------------------------
// What a call remembers between chunks.

#[derive(Clone, Default)]
struct St {
    v: VecDeque<f64>,
    /// The window in order, kept for `median` only.
    sorted: Vec<f64>,
    t: VecDeque<f64>,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    n: u64,
    flag: bool,
    flag2: bool,
    /// A constant argument (a number where a channel was expected): the one-number result is that number.
    constant: Option<f64>,
    carry: [Carry; 2],
    /// A window began with fewer samples than it needs: the replay before a chunk was too short.
    short: bool,
}

struct Ctx<'a> {
    env: &'a dyn Env,
    /// The samples read: the chunk and its margin.
    range: Range<usize>,
    /// How many of them are the chunk (the rest is the margin).
    commit: usize,
    pass: usize,
    cursor: usize,
    st: &'a mut Vec<St>,
    plan: &'a Plan,
    resolved: &'a [Option<f64>],
    /// Of the samples read, how many come before the chunk and only bring the windows up to date.
    warm: usize,
}

fn num_of(v: &Val) -> Option<f64> {
    match v {
        Val::Num(x) | Val::Dur(x) => Some(*x),
        Val::Ser(_) => None,
    }
}

fn ev(e: &Expr, cx: &mut Ctx) -> R<Val> {
    match e {
        Expr::Num(v) => Ok(Val::Num(*v)),
        Expr::Dur { secs, .. } => Ok(Val::Dur(*secs)),
        Expr::Label { at, .. } => {
            Err(ExprError::new("A value name in quotes goes next to the channel it belongs to", *at))
        }
        Expr::Name { name, at } => name_series(cx, name, *at).map(|s| Val::Ser(s.1)),
        Expr::Un { op, a } => {
            let a = ev(a, cx)?;
            let f: fn(f64) -> f64 = match op {
                UnOp::Neg => |x| -x,
                UnOp::Not => |x| (x == 0.0) as i32 as f64,
            };
            elementwise(&[a], |x| f(x[0]))
        }
        Expr::Bin { op, a, b } => {
            let (a, b) = operands(*op, a, b, cx)?;
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
            elementwise(&[a, b], |x| f(x[0], x[1]))
        }
        Expr::Call { f, args, at } => call(f, args, *at, cx),
    }
}

/// The channel a (possibly `a|b`) name stands for, with the name that matched.
fn name_series(cx: &Ctx, name: &str, at: usize) -> R<(String, Series)> {
    for alt in name.split('|').map(str::trim) {
        if let Some(mut s) = cx.env.read(alt, cx.range.clone()) {
            s.ch = Some(alt.to_string());
            return Ok((alt.to_string(), s));
        }
    }
    Err(ExprError::new(format!("There's no channel called {name}"), at))
}

/// Both sides of a binary operator, reading a `'label'` against the channel on its other side.
fn operands(op: BinOp, a: &Expr, b: &Expr, cx: &mut Ctx) -> R<(Val, Val)> {
    if matches!(op, BinOp::Eq | BinOp::Ne) {
        for (lab, other, label_first) in [(b, a, false), (a, b, true)] {
            let Expr::Label { text, at } = lab else { continue };
            let Some(&name) = other.names().first() else {
                return Err(ExprError::new("A value name in quotes needs a channel to compare with", *at));
            };
            let (ch, _) = name_series(cx, name, *at)?;
            let Some(v) = cx.env.label(&ch, text) else {
                return Err(ExprError::new(format!("{ch} has no value called '{text}'"), *at));
            };
            let o = ev(other, cx)?;
            return Ok(if label_first { (Val::Num(v), o) } else { (o, Val::Num(v)) });
        }
    }
    let x = ev(a, cx)?;
    Ok((x, ev(b, cx)?))
}

/// Applies `f` over the arguments element by element. Series are held (or interpolated) onto the longest one's
/// grid; numbers stay numbers; any NaN in gives NaN out.
fn elementwise(args: &[Val], f: impl Fn(&[f64]) -> f64) -> R<Val> {
    let lead = args.iter().filter_map(|a| if let Val::Ser(s) = a { Some(s) } else { None }).max_by_key(|s| s.len());
    let Some(lead) = lead else {
        let xs: Vec<f64> = args.iter().map(|a| num_of(a).unwrap()).collect();
        return Ok(Val::Num(if xs.iter().any(|x| x.is_nan()) { f64::NAN } else { f(&xs) }));
    };
    let (grid, len) = (lead.grid.clone(), lead.len());
    // the usual case, borrowed: one or two series on one grid (and numbers)
    let same = |a: &Val| matches!(a, Val::Ser(s) if s.grid == grid && s.len() == len) || num_of(a).is_some();
    if args.len() <= 2 && args.iter().all(same) {
        fn col(a: &Val) -> Result<&[f64], f64> {
            match a {
                Val::Ser(s) => Ok(&s.v[..]),
                v => Err(num_of(v).unwrap()),
            }
        }
        let mut out = Vec::with_capacity(len);
        match (col(&args[0]), args.get(1).map(|a| col(a))) {
            (Ok(x), None) => out.extend(x.iter().map(|&x| if x.is_nan() { f64::NAN } else { f(&[x]) })),
            (Ok(x), Some(Ok(y))) => {
                out.extend(x.iter().zip(y).map(|(&x, &y)| if x.is_nan() || y.is_nan() { f64::NAN } else { f(&[x, y]) }))
            }
            (Ok(x), Some(Err(k))) => {
                out.extend(x.iter().map(|&x| if x.is_nan() || k.is_nan() { f64::NAN } else { f(&[x, k]) }))
            }
            (Err(k), Some(Ok(y))) => {
                out.extend(y.iter().map(|&y| if y.is_nan() || k.is_nan() { f64::NAN } else { f(&[k, y]) }))
            }
            _ => unreachable!("a series leads"),
        }
        return Ok(Val::Ser(Series::new(out, grid)));
    }
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
enum Window {
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

/// The non-gap samples of `s`: their indices (`None`: all of them, in order), values and, when `times`, their times.
fn compact(s: &Series, times: bool) -> (Option<Vec<usize>>, std::borrow::Cow<'_, [f64]>, Vec<f64>) {
    use std::borrow::Cow;
    if !s.v.iter().any(|x| x.is_nan()) {
        let t = if times { (0..s.len()).map(|i| s.time(i)).collect() } else { Vec::new() };
        return (None, Cow::Borrowed(&s.v[..]), t);
    }
    let idx: Vec<usize> = (0..s.len()).filter(|&i| !s.v[i].is_nan()).collect();
    let v = idx.iter().map(|&i| s.v[i]).collect();
    let t = if times { idx.iter().map(|&i| s.time(i)).collect() } else { Vec::new() };
    (Some(idx), Cow::Owned(v), t)
}

fn scatter(s: &Series, idx: Option<&[usize]>, res: Vec<f64>) -> Val {
    let Some(idx) = idx else { return Val::Ser(Series::new(res, s.grid.clone())) };
    let mut out = vec![f64::NAN; s.len()];
    for (j, &i) in idx.iter().enumerate() {
        out[i] = res[j];
    }
    Val::Ser(Series::new(out, s.grid.clone()))
}

/// Runs `f(state, values, times)` over the non-gap samples; gaps stay gaps. The state advances over the committed
/// samples only; the margin is computed from a copy. `times` says whether `f` reads the times (otherwise empty).
fn sample_op(cx: &mut Ctx, id: usize, s: &Series, times: bool, f: impl Fn(&mut St, &[f64], &[f64]) -> Vec<f64>) -> Val {
    let (idx, v, t) = compact(s, times);
    let k = idx.as_ref().map_or(cx.commit.min(v.len()), |idx| idx.partition_point(|&i| i < cx.commit));
    let tk = |r: Range<usize>| if times { &t[r] } else { &t[..0] };
    let st = &mut cx.st[id];
    let mut res = f(st, &v[..k], tk(0..k));
    if k < v.len() {
        let mut tmp = st.clone();
        res.extend(f(&mut tmp, &v[k..], tk(k..v.len())));
    }
    scatter(s, idx.as_deref(), res)
}

/// The values `n` samples back (NaN before there are `n`), and their times.
fn lag(st: &mut St, v: &[f64], t: &[f64], n: usize) -> (Vec<f64>, Vec<f64>) {
    let with_t = !t.is_empty();
    let hist = st.v.len();
    let all_v: Vec<f64> = st.v.iter().chain(v).copied().collect();
    let all_t: Vec<f64> = st.t.iter().chain(t).copied().collect();
    let at = |all: &[f64], j: usize| if hist + j >= n { all[hist + j - n] } else { f64::NAN };
    let out = (
        (0..v.len()).map(|j| at(&all_v, j)).collect(),
        if with_t { (0..v.len()).map(|j| at(&all_t, j)).collect() } else { Vec::new() },
    );
    let keep = all_v.len().saturating_sub(n);
    st.v = all_v[keep..].iter().copied().collect();
    if with_t {
        st.t = all_t[keep..].iter().copied().collect();
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

/// The median over a window of samples: the window is kept in order and the oldest value is replaced in place.
fn median_samples(st: &mut St, v: &[f64], n: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(v.len());
    for &x in v {
        if st.v.len() == n {
            let old = st.v.pop_front().unwrap();
            let i = st.sorted.partition_point(|y| y.total_cmp(&old).is_lt());
            let j = st.sorted.partition_point(|y| y.total_cmp(&x).is_lt());
            if j > i {
                st.sorted.copy_within(i + 1..j, i);
                st.sorted[j - 1] = x;
            } else {
                st.sorted.copy_within(j..i, j + 1);
                st.sorted[j] = x;
            }
        } else {
            let j = st.sorted.partition_point(|y| y.total_cmp(&x).is_lt());
            st.sorted.insert(j, x);
        }
        st.v.push_back(x);
        out.push(if st.v.len() < n {
            f64::NAN
        } else {
            let m = n / 2;
            if n % 2 == 1 { st.sorted[m] } else { (st.sorted[m - 1] + st.sorted[m]) / 2.0 }
        });
    }
    out
}

fn rolling(st: &mut St, v: &[f64], t: &[f64], w: Window, kind: Roll) -> Vec<f64> {
    if let (Roll::Median, Window::Samples(n)) = (kind, w) {
        return median_samples(st, v, n);
    }
    let mut out = Vec::with_capacity(v.len());
    for j in 0..v.len() {
        st.v.push_back(v[j]);
        if let Roll::Median = kind {
            let at = st.sorted.partition_point(|y| y.total_cmp(&v[j]).is_lt());
            st.sorted.insert(at, v[j]);
        }
        if let Window::Secs(_) = w {
            st.t.push_back(t[j]);
        }
        st.a += v[j];
        st.b += v[j] * v[j];
        let evict = |st: &mut St| {
            let x = st.v.pop_front().unwrap();
            if matches!(w, Window::Secs(_)) {
                st.t.pop_front();
            }
            st.a -= x;
            st.b -= x * x;
            if let Roll::Median = kind {
                let at = st.sorted.partition_point(|y| y.total_cmp(&x).is_lt());
                st.sorted.remove(at);
            }
        };
        match w {
            Window::Samples(n) => {
                if st.v.len() > n {
                    evict(st);
                }
            }
            Window::Secs(sec) => {
                while t[j] - st.t[0] > sec {
                    evict(st);
                }
            }
        }
        let n = st.v.len();
        if matches!(w, Window::Samples(k) if n < k) {
            out.push(f64::NAN);
            continue;
        }
        out.push(match kind {
            Roll::Mean => st.a / n as f64,
            Roll::Sum => st.a,
            Roll::Std if n > 1 => ((st.b - st.a * st.a / n as f64) / (n as f64 - 1.0)).max(0.0).sqrt(),
            Roll::Std => f64::NAN,
            Roll::Min => st.v.iter().copied().fold(f64::INFINITY, f64::min),
            Roll::Max => st.v.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            Roll::Median => {
                let win = &st.sorted;
                let m = win.len() / 2;
                if win.len() % 2 == 1 { win[m] } else { (win[m - 1] + win[m]) / 2.0 }
            }
        });
    }
    out
}

// One-number functions: accumulated over a pass (committed samples only), finished at its end.

fn accumulate(kind: Metric, st: &mut St, arg: &Val, commit: usize) {
    let s = match arg {
        Val::Ser(s) => s,
        n => {
            st.constant = num_of(n);
            return;
        }
    };
    let commit = commit.min(s.len());
    for i in 0..commit {
        let x = s.v[i];
        if kind == Metric::Duration {
            let ti = s.time(i);
            if st.flag2 {
                st.d = ti - st.c; // the interval just ended
                if st.flag {
                    st.a += st.d;
                }
            }
            (st.c, st.flag2) = (ti, true);
            st.flag = !x.is_nan() && x != 0.0;
        }
        if x.is_nan() {
            continue;
        }
        st.n += 1;
        let first = st.n == 1;
        match kind {
            Metric::Mean | Metric::Sum => st.a += x,
            Metric::Std => {
                let dx = x - st.b;
                st.b += dx / st.n as f64;
                st.c += dx * (x - st.b);
            }
            Metric::Min => st.a = if first { x } else { st.a.min(x) },
            Metric::Max => st.a = if first { x } else { st.a.max(x) },
            Metric::First => {
                if first {
                    st.a = x
                }
            }
            Metric::Last => st.a = x,
            Metric::Count | Metric::Duration => {}
            Metric::Pct => st.a += (x != 0.0) as u8 as f64,
            Metric::Occurrences => {
                if x != 0.0 && !st.flag {
                    st.a += 1.0;
                }
                st.flag = x != 0.0;
            }
        }
    }
}

fn finish(kind: Metric, st: &St) -> f64 {
    if let Some(c) = st.constant {
        return c;
    }
    if st.n == 0 {
        return f64::NAN;
    }
    let n = st.n as f64;
    match kind {
        Metric::Mean => st.a / n,
        Metric::Sum | Metric::Min | Metric::Max | Metric::First | Metric::Last | Metric::Occurrences => st.a,
        Metric::Std => {
            if st.n > 1 {
                (st.c / (n - 1.0)).sqrt()
            } else {
                f64::NAN
            }
        }
        Metric::Count => n,
        Metric::Pct => 100.0 * st.a / n,
        // the last sample's interval is taken to be the one before it
        Metric::Duration => st.a + if st.flag { st.d } else { 0.0 },
    }
}

fn call(f: &str, args: &[Expr], at: usize, cx: &mut Ctx) -> R<Val> {
    let id = cx.cursor;
    cx.cursor += 1;
    let (lo, hi) = crate::functions::arity(f).expect("the parser checks function names");
    if args.len() < lo || args.len() > hi {
        let want = if lo == hi { format!("{lo}") } else { format!("{lo} to {hi}") };
        return Err(ExprError::new(format!("{f}() takes {want} argument{}", if hi == 1 { "" } else { "s" }), at));
    }
    let metric = cx.plan.nodes[id].metric;
    if let Some((d, _)) = metric.filter(|(d, _)| *d <= cx.pass) {
        let _ = d;
        cx.cursor += cx.plan.nodes[id].inner;
        return Ok(Val::Num(cx.resolved[id].unwrap_or(f64::NAN)));
    }
    let a: Vec<Val> = args.iter().map(|e| ev(e, cx)).collect::<R<_>>()?;
    if let Some((d, kind)) = metric {
        if d == cx.pass + 1 {
            accumulate(kind, &mut cx.st[id], &a[0], cx.commit);
        }
        return Ok(Val::Num(f64::NAN));
    }
    let ew = |g: &dyn Fn(&[f64]) -> f64| elementwise(&a, g);
    let ser = |i: usize| series_arg(&a[i], f, at);
    let win = |i: usize| window(a.get(i), f, at);
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
            elementwise(&a[..1], |x| (x[0] * q).round() / q)?
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
        // two or more arguments, element by element (one channel is a one-number function, handled above)
        "min" => match &a[..] {
            [n] => n.clone(),
            _ => ew(&|x| x.iter().copied().fold(f64::INFINITY, f64::min))?,
        },
        "max" => match &a[..] {
            [n] => n.clone(),
            _ => ew(&|x| x.iter().copied().fold(f64::NEG_INFINITY, f64::max))?,
        },
        "delta" | "diff" => sample_op(cx, id, ser(0)?, false, |st, v, t| {
            let (p, _) = lag(st, v, t, 1);
            v.iter().zip(p).map(|(x, p)| x - p).collect()
        }),
        "prev" => {
            let n = a.get(1).and_then(num_of).unwrap_or(1.0).max(0.0) as usize;
            sample_op(cx, id, ser(0)?, false, move |st, v, t| lag(st, v, t, n).0)
        }
        "pct_change" => sample_op(cx, id, ser(0)?, false, |st, v, t| {
            let (p, _) = lag(st, v, t, 1);
            v.iter().zip(p).map(|(x, p)| if p != 0.0 { x / p - 1.0 } else { f64::NAN }).collect()
        }),
        "rate" => sample_op(cx, id, ser(0)?, true, |st, v, t| {
            let (p, pt) = lag(st, v, t, 1);
            (0..v.len()).map(|j| if t[j] > pt[j] { (v[j] - p[j]) / (t[j] - pt[j]) } else { f64::NAN }).collect()
        }),
        "cumsum" => sample_op(cx, id, ser(0)?, false, |st, v, _| {
            v.iter()
                .map(|x| {
                    st.a += x;
                    st.a
                })
                .collect()
        }),
        "integral" => sample_op(cx, id, ser(0)?, true, |st, v, t| {
            (0..v.len())
                .map(|j| {
                    if st.flag {
                        st.a += (v[j] + st.b) / 2.0 * (t[j] - st.c);
                    }
                    (st.b, st.c, st.flag) = (v[j], t[j], true);
                    st.a
                })
                .collect()
        }),
        "running_max" | "running_min" => {
            let max = f == "running_max";
            sample_op(cx, id, ser(0)?, false, move |st, v, _| {
                v.iter()
                    .map(|&x| {
                        st.a = if !st.flag {
                            x
                        } else if max {
                            st.a.max(x)
                        } else {
                            st.a.min(x)
                        };
                        st.flag = true;
                        st.a
                    })
                    .collect()
            })
        }
        "sma" | "rolling_mean" | "movavg" | "rolling_std" | "rolling_min" | "rolling_max" | "rolling_sum"
        | "median" => {
            let w = win(1)?;
            let kind = match f {
                "rolling_std" => Roll::Std,
                "rolling_min" => Roll::Min,
                "rolling_max" => Roll::Max,
                "rolling_sum" => Roll::Sum,
                "median" => Roll::Median,
                _ => Roll::Mean,
            };
            let s0 = ser(0)?;
            if let (true, Window::Samples(n)) = (cx.warm > 0, w) {
                // the window at the chunk's first sample must lie inside the replay
                if s0.v[..(cx.warm + 1).min(s0.len())].iter().filter(|x| !x.is_nan()).count() < n {
                    cx.st[id].short = true;
                }
            }
            sample_op(cx, id, s0, matches!(w, Window::Secs(_)), move |st, v, t| rolling(st, v, t, w, kind))
        }
        "ema" => {
            let w = win(1)?;
            sample_op(cx, id, ser(0)?, matches!(w, Window::Secs(_)), move |st, v, t| {
                (0..v.len())
                    .map(|j| {
                        let al = match w {
                            Window::Samples(n) => 2.0 / (n as f64 + 1.0),
                            Window::Secs(s) if st.flag => 1.0 - (-(t[j] - st.b) / s).exp(),
                            Window::Secs(_) => 1.0,
                        };
                        st.a = if st.flag { st.a + al * (v[j] - st.a) } else { v[j] };
                        if let Window::Secs(_) = w {
                            st.b = t[j];
                        }
                        st.flag = true;
                        st.a
                    })
                    .collect()
            })
        }
        "lowpass" | "highpass" | "bandpass" => {
            let s = ser(0)?;
            let cut = |i: usize| num_of(&a[i]).filter(|c| *c > 0.0);
            let fs = cx.env.hz().unwrap_or(match &s.grid {
                Grid::Rate { hz, .. } => *hz,
                Grid::Stamps(t) if t.len() > 1 => (t.len() - 1) as f64 / (t[t.len() - 1] - t[0]),
                Grid::Stamps(_) => 1.0,
            });
            let stages: Vec<(bool, f64)> = if f == "bandpass" {
                match (cut(1), cut(2)) {
                    (Some(x), Some(y)) if y > x => vec![(false, x), (true, y)],
                    _ => {
                        return Err(ExprError::new(
                            "bandpass(x, low Hz, high Hz): the second cutoff must be above the first",
                            at,
                        ));
                    }
                }
            } else {
                match cut(1) {
                    Some(c) => vec![(f == "lowpass", c)],
                    None => {
                        return Err(ExprError::new(
                            format!("{f}(x, Hz): the cutoff is a frequency in Hz, like {f}(accel_x, 5)"),
                            at,
                        ));
                    }
                }
            };
            let mut bqs = Vec::new();
            for (low, c) in stages {
                match Biquad::new(low, c, fs) {
                    Some(b) => bqs.push(b),
                    None => {
                        return Err(ExprError::new(
                            format!("The cutoff ({c} Hz) must be below half the sample rate ({} Hz)", fs / 2.0),
                            at,
                        ));
                    }
                }
            }
            let (idx, v, _) = compact(s, false);
            let k = idx.as_ref().map_or(cx.commit.min(v.len()), |idx| idx.partition_point(|&i| i < cx.commit));
            let mut v = v.into_owned();
            for (stage, bq) in bqs.iter().enumerate() {
                v = filters::zero_phase(bq, &mut cx.st[id].carry[stage], &v, k);
            }
            scatter(s, idx.as_deref(), v)
        }
        "interp" => {
            let mut s = ser(0)?.clone();
            s.linear = true;
            Val::Ser(s)
        }
        // seconds since the condition last held; a gap stays a gap, and so does everything before the first hit
        "since" => {
            let s = ser(0)?;
            let k = cx.commit.min(s.len());
            let st = &mut cx.st[id];
            let mut tmp = st.clone();
            let out = (0..s.len())
                .map(|i| {
                    let cur = if i < k { &mut *st } else { &mut tmp };
                    let x = s.v[i];
                    if x.is_nan() {
                        return f64::NAN;
                    }
                    if x != 0.0 {
                        (cur.a, cur.flag) = (s.time(i), true);
                    }
                    if cur.flag { s.time(i) - cur.a } else { f64::NAN }
                })
                .collect();
            Val::Ser(Series::new(out, s.grid.clone()))
        }
        "past_warn" | "past_alarm" => {
            let s = ser(0)?;
            let alarm = f == "past_alarm";
            let kind = if alarm { "alarm" } else { "warn" };
            let Some(ch) = s.ch.as_deref() else {
                return Err(ExprError::new(format!("{f}() takes a channel, like {f}(esc_temp_1)"), at));
            };
            let (lo, hi) = cx
                .env
                .limits(ch, alarm)
                .filter(|(l, h)| l.is_some() || h.is_some())
                .ok_or_else(|| ExprError::new(format!("{ch} has no {kind} limits"), at))?;
            elementwise(&a, |x| (lo.is_some_and(|l| x[0] <= l) || hi.is_some_and(|h| x[0] >= h)) as i32 as f64)?
        }
        _ => unreachable!("{f} is in the table but not evaluated"),
    })
}
