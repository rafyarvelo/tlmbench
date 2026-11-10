//! Checks and rules: an expectation over a whole recording, with a verdict, and event rules that mark where a
//! condition holds. Both are written in the expression language. A check says "expect X when S": its violations are
//! the samples where S holds (after a settle time) and X doesn't, and violations shorter than the grace time are
//! ignored. A rule marks the runs where its condition holds. A check whose X is one number (`max(esc_temp) < 80`) is a
//! summary: it is evaluated over the scope alone, and its verdict is that number against the limit.
//!
//! Sets of checks are TOML files, one per set (`checks/<name>.toml` in the app cache; Share writes the same files to
//! `.tlmbench/checks/`). Results are kept in the app cache by the recording's contents and the set's version, and the
//! events they make (Check failed, and a rule's own category) are read-only: they are recomputed with the results and
//! never stored with the events you add.

use std::collections::HashMap;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tlmbench_expr::{BinOp, Env, Expr, Recipe, Series, recipe};

use crate::derived::RecEnv;
use crate::events::Event;
use crate::{Error, Opened, Result, Store};

pub const SCHEMA: u32 = 1;
/// The event category of every violation of a check.
pub const CHECK_CATEGORY: &str = "check";
/// Violations and rule events kept per check (the count says how many there were).
const KEEP_CHECK: usize = 200;
const KEEP_RULE: usize = 500;
/// Violations closer than this are one violation.
const JOIN_SECS: f64 = 0.25;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Check,
    Rule,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    #[default]
    Fail,
    Warn,
}

fn yes() -> bool {
    true
}

fn default_group() -> String {
    "Checks".into()
}

/// One row of a set.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Check {
    pub name: String,
    #[serde(default)]
    pub kind: Kind,
    #[serde(default = "default_group")]
    pub group: String,
    /// What should hold (a rule: where to mark).
    pub expect: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    #[serde(default)]
    pub severity: Severity,
    /// A rule's event category.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Violations (or rule runs) shorter than this many seconds are ignored.
    #[serde(default)]
    pub grace: f64,
    /// Seconds after the scope starts that don't count.
    #[serde(default)]
    pub settle: f64,
    #[serde(default = "yes")]
    pub enabled: bool,
}

/// A named list of checks.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CheckSet {
    pub schema: u32,
    pub name: String,
    /// Runs on every file that has its channels, not only where it is attached.
    #[serde(default)]
    pub runs_on_any_file: bool,
    /// Groups that show their results but never fail the set.
    #[serde(default)]
    pub info_groups: Vec<String>,
    #[serde(default, rename = "check")]
    pub checks: Vec<Check>,
}

pub fn from_toml(text: &str) -> std::result::Result<CheckSet, String> {
    let s: CheckSet = toml::from_str(text).map_err(|e| e.to_string())?;
    if s.schema != SCHEMA {
        return Err(format!("check set schema {} isn't supported (this version reads {SCHEMA})", s.schema));
    }
    Ok(s)
}

pub fn to_toml(set: &CheckSet) -> String {
    toml::to_string(&CheckSet { schema: SCHEMA, ..set.clone() }).expect("check sets serialize")
}

/// What a check got on one file, worst first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Fail,
    Warn,
    /// It couldn't be evaluated (a mistake in the expression).
    Err,
    /// A channel it needs is missing.
    Cant,
    #[default]
    Pass,
    /// Its When never held.
    Na,
    /// A rule: marked events.
    Mark,
    Off,
}

impl Verdict {
    pub fn rank(self) -> u8 {
        match self {
            Verdict::Fail => 5,
            Verdict::Warn => 4,
            Verdict::Err => 3,
            Verdict::Cant => 2,
            Verdict::Pass => 1,
            Verdict::Na | Verdict::Mark | Verdict::Off => 0,
        }
    }
}

mod ns {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &i64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
        String::deserialize(d)?.parse().map_err(serde::de::Error::custom)
    }
}

/// A violation or marked run, in nanoseconds as a recording's times are (decimal strings on the wire).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    #[serde(with = "ns")]
    pub start: i64,
    /// Equal to `start` for an instant.
    #[serde(with = "ns")]
    pub end: i64,
}

/// One check's result on one file.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    pub name: String,
    pub verdict: Verdict,
    /// How many violations (or events).
    pub count: usize,
    /// Their total time, in seconds.
    pub secs: f64,
    /// The first `KEEP_*` of them.
    pub runs: Vec<Span>,
    /// The worst value of the checked quantity inside the violations, and when.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worst: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "opt_ns")]
    pub worst_t: Option<i64>,
    /// A summary check: the number it computed, the function that made it, and the limit it was held to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
    /// How long the scope held, in seconds (the whole recording without a When).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_secs: Option<f64>,
    /// Why it didn't run, or didn't apply.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub message: String,
    /// The channels its expressions read.
    #[serde(default)]
    pub channels: Vec<String>,
}

mod opt_ns {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &Option<i64>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(v) => s.serialize_str(&v.to_string()),
            None => s.serialize_none(),
        }
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
        Option::<String>::deserialize(d)?.map(|t| t.parse().map_err(serde::de::Error::custom)).transpose()
    }
}

/// A group's worst result.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GroupRoll {
    pub name: String,
    pub verdict: Verdict,
    /// Event count of its rules.
    pub events: usize,
    pub info: bool,
    /// Checks that counted (not N/A, rules or off), and how many of them failed, warned and passed.
    pub total: usize,
    pub failed: usize,
    pub warned: usize,
    pub passed: usize,
}

/// A set's roll-up: a group takes the worst of its checks, a set the worst of its groups that aren't info only.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rollup {
    pub groups: Vec<GroupRoll>,
    pub verdict: Verdict,
    pub failed: usize,
    pub warned: usize,
    pub events: usize,
    /// Whether anything ran (a set where every check can't run doesn't apply to the file).
    pub applies: bool,
}

/// A set's results on one file.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SetResult {
    pub set: String,
    pub outcomes: Vec<Outcome>,
    pub rollup: Rollup,
}

pub fn rollup(set: &CheckSet, outcomes: &[Outcome]) -> Rollup {
    let mut groups: Vec<(String, Vec<&Outcome>)> = Vec::new();
    for (k, o) in set.checks.iter().zip(outcomes) {
        match groups.iter_mut().find(|(g, _)| *g == k.group) {
            Some((_, v)) => v.push(o),
            None => groups.push((k.group.clone(), vec![o])),
        }
    }
    let worst =
        |vs: &mut dyn Iterator<Item = Verdict>| vs.fold(Verdict::Pass, |a, b| if b.rank() > a.rank() { b } else { a });
    let rolls: Vec<GroupRoll> = groups
        .into_iter()
        .map(|(name, os)| {
            let counted: Vec<Verdict> = os
                .iter()
                .map(|o| o.verdict)
                .filter(|v| !matches!(v, Verdict::Na | Verdict::Mark | Verdict::Off))
                .collect();
            let events = os.iter().filter(|o| o.verdict == Verdict::Mark).map(|o| o.count).sum();
            let verdict = if !counted.is_empty() {
                worst(&mut counted.iter().copied())
            } else if os.iter().any(|o| o.verdict == Verdict::Mark) {
                Verdict::Mark
            } else if !os.is_empty() && os.iter().all(|o| o.verdict == Verdict::Off) {
                Verdict::Off
            } else {
                Verdict::Na
            };
            let n = |v: Verdict| os.iter().filter(|o| o.verdict == v).count();
            GroupRoll {
                info: set.info_groups.contains(&name),
                name,
                verdict,
                events,
                total: counted.len(),
                failed: n(Verdict::Fail),
                warned: n(Verdict::Warn),
                passed: n(Verdict::Pass),
            }
        })
        .collect();
    let main: Vec<&GroupRoll> =
        rolls.iter().filter(|g| !g.info && !matches!(g.verdict, Verdict::Na | Verdict::Mark | Verdict::Off)).collect();
    let verdict = if !main.is_empty() {
        worst(&mut main.iter().map(|g| g.verdict))
    } else if rolls.iter().any(|g| g.verdict == Verdict::Mark) {
        Verdict::Mark
    } else {
        Verdict::Na
    };
    Rollup {
        verdict,
        failed: outcomes.iter().filter(|o| o.verdict == Verdict::Fail).count(),
        warned: outcomes.iter().filter(|o| o.verdict == Verdict::Warn).count(),
        events: rolls.iter().map(|g| g.events).sum(),
        applies: outcomes.iter().any(|o| !matches!(o.verdict, Verdict::Cant | Verdict::Err | Verdict::Off)),
        groups: rolls,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Running

/// The inputs of a check, read only inside the scope: samples outside it are gaps, so one-number functions see the
/// scope alone and sample checks count only samples in it.
struct Masked<'a> {
    inner: &'a dyn Env,
    /// Seconds from the recording's start, sorted and apart.
    scope: &'a [(f64, f64)],
}

impl Env for Masked<'_> {
    fn series(&self, name: &str) -> Option<Series> {
        self.read(name, 0..self.inner.samples()?)
    }
    fn read(&self, name: &str, r: Range<usize>) -> Option<Series> {
        let mut s = self.inner.read(name, r)?;
        let mut at = 0;
        for i in 0..s.len() {
            let t = s.time(i);
            while at < self.scope.len() && self.scope[at].1 <= t {
                at += 1;
            }
            if !(at < self.scope.len() && self.scope[at].0 <= t) {
                s.v[i] = f64::NAN;
            }
        }
        Some(s)
    }
    fn samples(&self) -> Option<usize> {
        self.inner.samples()
    }
    fn hz(&self) -> Option<f64> {
        self.inner.hz()
    }
    fn label(&self, c: &str, t: &str) -> Option<f64> {
        self.inner.label(c, t)
    }
    fn limits(&self, c: &str, alarm: bool) -> Option<(Option<f64>, Option<f64>)> {
        self.inner.limits(c, alarm)
    }
}

/// What evaluating a (sub)expression over the whole recording gave.
enum Got {
    Number(f64),
    Series,
}

fn stream(e: &Expr, env: &dyn Env, sink: &mut dyn FnMut(&Series)) -> std::result::Result<Got, String> {
    match tlmbench_expr::run(e, env, tlmbench_expr::CHUNK, &mut |_, s| sink(s)) {
        Ok(Some(x)) => Ok(Got::Number(x)),
        Ok(None) => Ok(Got::Series),
        Err(e) => Err(e.msg),
    }
}

fn truthy(x: f64) -> bool {
    !x.is_nan() && x != 0.0
}

/// A parsed check with its recipes substituted.
pub(crate) fn prepare(text: &str, recipes: &[Recipe], env: &dyn Env) -> std::result::Result<Expr, (Verdict, String)> {
    let has = |n: &str| env.read(n, 0..0).is_some();
    let mut all = vec![Recipe {
        name: "\0check".into(),
        expr: text.into(),
        unit: None,
        category: None,
        decimals: None,
        automatic: false,
    }];
    all.extend(recipes.iter().filter(|r| r.name != "\0check").cloned());
    let (mut ok, mut failed) = recipe::resolve(&all[..], &has);
    // `resolve` resolves every recipe; only the first is ours
    if let Some(pos) = ok.iter().position(|r| r.recipe.name == "\0check") {
        return Ok(ok.swap_remove(pos).expr);
    }
    let e = failed.drain(..).find(|(n, _)| n == "\0check").map(|(_, e)| e).expect("the check was resolved or not");
    Err(if e.msg.starts_with("There's no channel called") { (Verdict::Cant, e.msg) } else { (Verdict::Err, e.msg) })
}

fn ns_of(t_first: i64, secs: f64) -> i64 {
    t_first + (secs * 1e9).round() as i64
}

fn function_of(e: &Expr) -> Option<String> {
    if let Expr::Call { f, .. } = e { Some(f.clone()) } else { None }
}

/// Runs one check on a recording. Times in the result are nanoseconds on the recording's own clock (`t_first` is the
/// first sample's); `recipes` are the library, for names that aren't channels.
pub fn run_check(env: &dyn Env, k: &Check, t_first: i64, recipes: &[Recipe]) -> Outcome {
    let mark = k.kind == Kind::Rule;
    let mut out = Outcome {
        name: k.name.clone(),
        verdict: if mark { Verdict::Mark } else { Verdict::Pass },
        ..Outcome::default()
    };
    let bail = |mut out: Outcome, v: Verdict, msg: &str| {
        out.verdict = v;
        out.message = msg.to_string();
        out
    };
    if !k.enabled {
        return bail(out, Verdict::Off, "turned off");
    }
    if k.expect.trim().is_empty() {
        return bail(out, Verdict::Err, if mark { "no condition yet" } else { "nothing to expect yet" });
    }
    let expect = match prepare(&k.expect, recipes, env) {
        Ok(e) => e,
        Err((v, m)) => return bail(out, v, &m),
    };
    let when = match k.when.as_deref().filter(|w| !w.trim().is_empty()) {
        None => None,
        Some(w) => match prepare(w, recipes, env) {
            Ok(e) => Some(e),
            Err((v, m)) => return bail(out, v, &format!("When: {m}")),
        },
    };
    for e in std::iter::once(&expect).chain(&when) {
        for n in e.names() {
            let first = n.split('|').next().unwrap_or(n).trim().to_string();
            if !out.channels.contains(&first) {
                out.channels.push(first);
            }
        }
    }
    let hz = env.hz().filter(|h| *h > 0.0).unwrap_or(1.0);
    let dt = 1.0 / hz;

    // the scope: runs where When holds, after the settle time
    let mut scope: Vec<(f64, f64)> = Vec::new();
    if let Some(w) = &when {
        let (mut start, mut last): (Option<f64>, f64) = (None, 0.0);
        let mut close = |start: &mut Option<f64>, last: f64| {
            if let Some(a) = start.take() {
                let (a, b) = (a + k.settle - dt / 2.0, last + dt / 2.0);
                if b > a {
                    scope.push((a, b));
                }
            }
        };
        let got = stream(w, env, &mut |s| {
            for i in 0..s.len() {
                if truthy(s.v[i]) {
                    start.get_or_insert(s.time(i));
                    last = s.time(i);
                } else {
                    close(&mut start, last);
                }
            }
        });
        match got {
            Err(m) => return bail(out, Verdict::Err, &format!("When: {m}")),
            Ok(Got::Number(x)) if !truthy(x) => return bail(out, Verdict::Na, "When is never true"),
            Ok(Got::Number(_)) => {}
            Ok(Got::Series) => {
                close(&mut start, last);
                if scope.is_empty() {
                    return bail(out, Verdict::Na, "its When never held");
                }
            }
        }
    }
    let scoped = !scope.is_empty();
    let masked = Masked { inner: env, scope: &scope };
    let env: &dyn Env = if scoped { &masked } else { env };
    if scoped {
        out.scope_secs = Some(scope.iter().map(|(a, b)| b - a).sum());
    }

    // a comparison's left side is the quantity checked: its worst value in a violation, or a summary's number
    let (lhs, rhs) = match &expect {
        Expr::Bin { op: BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge, a, b } => (Some(a.as_ref()), Some(b.as_ref())),
        _ => (None, None),
    };
    let op = if let Expr::Bin { op, .. } = &expect { Some(*op) } else { None };

    let mut in_scope = 0usize;
    let mut runs: Vec<(f64, f64)> = Vec::new(); // first and last sample time of each violation
    let mut cur: Option<(f64, f64)> = None;
    let got = stream(&expect, env, &mut |s| {
        for i in 0..s.len() {
            let v = s.v[i];
            if v.is_nan() {
                continue;
            }
            in_scope += 1;
            if truthy(v) == mark {
                let t = s.time(i);
                match &mut cur {
                    Some((_, last)) if t - *last <= JOIN_SECS + 1e-9 => *last = t,
                    c => {
                        if let Some(done) = c.take() {
                            runs.push(done);
                        }
                        *c = Some((t, t));
                    }
                }
            }
        }
    });
    let got = match got {
        Ok(g) => g,
        Err(m) => return bail(out, Verdict::Err, &m),
    };
    if let Some(done) = cur.take() {
        runs.push(done);
    }
    let sev = if k.severity == Severity::Warn { Verdict::Warn } else { Verdict::Fail };

    if let Got::Number(x) = got {
        // a summary: one number for the scope
        if mark {
            return bail(
                out,
                Verdict::Err,
                "a rule marks where something holds over time; this gives one number (use it as a check)",
            );
        }
        if let Some(l) = lhs {
            let mut num = None;
            if let Ok(Got::Number(v)) = stream(l, env, &mut |_| {}) {
                num = Some(v);
            }
            out.value = num;
            out.function = function_of(l);
            out.limit = rhs.and_then(|r| if let Expr::Num(v) = r { Some(*v) } else { None });
        }
        if x.is_nan() {
            return bail(out, Verdict::Na, if scoped { "no samples in scope" } else { "no samples" });
        }
        if x == 0.0 {
            out.verdict = sev;
        }
        return out;
    }

    if in_scope == 0 {
        return bail(out, Verdict::Na, if when.is_some() { "its When never held" } else { "no samples" });
    }
    out.scope_secs.get_or_insert(in_scope as f64 * dt);
    let kept: Vec<(f64, f64)> = runs.into_iter().filter(|(a, b)| b - a + dt + 1e-6 >= k.grace).collect();
    if kept.is_empty() {
        return out;
    }
    out.verdict = if mark { Verdict::Mark } else { sev };
    out.count = kept.len();
    out.secs = kept.iter().map(|(a, b)| b - a + dt).sum();
    // a rule's one-sample run is an instant
    let keep = if mark { KEEP_RULE } else { KEEP_CHECK };
    out.runs = kept
        .iter()
        .take(keep)
        .map(|&(a, b)| Span { start: ns_of(t_first, a), end: ns_of(t_first, if mark { b } else { b + dt }) })
        .collect();

    // the worst value: furthest past the limit, within the violations
    if let (Some(l), Some(op)) = (lhs, op) {
        let up = matches!(op, BinOp::Lt | BinOp::Le) != mark;
        let (mut w, mut wt) = (if up { f64::NEG_INFINITY } else { f64::INFINITY }, None);
        let mut at = 0;
        let _ = stream(l, env, &mut |s| {
            for i in 0..s.len() {
                let t = s.time(i);
                while at < kept.len() && kept[at].1 < t {
                    at += 1;
                }
                let v = s.v[i];
                if at < kept.len() && kept[at].0 <= t && !v.is_nan() && (if up { v > w } else { v < w }) {
                    (w, wt) = (v, Some(t));
                }
            }
        });
        if w.is_finite() {
            out.worst = Some(w);
            out.worst_t = wt.map(|t| ns_of(t_first, t));
        }
    }
    out
}

/// Runs every row of `set` on a recording and rolls the results up.
pub fn run_set(env: &dyn Env, set: &CheckSet, t_first: i64, recipes: &[Recipe]) -> SetResult {
    let outcomes: Vec<Outcome> = set.checks.iter().map(|k| run_check(env, k, t_first, recipes)).collect();
    SetResult { set: set.name.clone(), rollup: rollup(set, &outcomes), outcomes }
}

/// The events a set's results make, read-only: a check's violations in [`CHECK_CATEGORY`], a rule's in its own
/// category. Ids are stable, so an event keeps its identity while the data and the set stay as they are.
pub fn events_of(set: &CheckSet, res: &SetResult) -> Vec<Event> {
    let mut out = Vec::new();
    for (k, o) in set.checks.iter().zip(&res.outcomes) {
        let rule = k.kind == Kind::Rule;
        for (n, s) in o.runs.iter().enumerate() {
            out.push(Event {
                id: format!(
                    "{}:{}:{n}",
                    if rule { "rule" } else { "check" },
                    blake3::hash(format!("{}\0{}", set.name, k.name).as_bytes()).to_hex()[..12].to_owned()
                ),
                name: k.name.clone(),
                start: s.start.to_string(),
                duration: (s.end - s.start).to_string(),
                category: if rule { k.category.clone().unwrap_or_default() } else { CHECK_CATEGORY.into() },
                source: if rule { "rule" } else { "check" }.into(),
                channels: o.channels.clone(),
            });
        }
    }
    out.sort_by_key(|e| e.start.parse::<i64>().unwrap_or(0));
    out
}

// ---------------------------------------------------------------------------------------------------------------
// The library of sets, and cached results

/// The sets the app keeps, in a folder.
#[derive(Clone, Debug)]
pub struct Sets {
    dir: PathBuf,
}

fn slug(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "set".into() } else { s }
}

impl Sets {
    /// The sets under the cache root `root`.
    pub fn new(root: &Path) -> Self {
        Self { dir: root.join("checks") }
    }

    /// Sets kept in an arbitrary folder (`.tlmbench/checks/`, or a folder given to the CLI).
    pub fn in_dir(dir: &Path) -> Self {
        Self { dir: dir.to_owned() }
    }

    fn files(&self) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "toml"))
            .collect();
        v.sort();
        v
    }

    /// Every set, by name. A file that doesn't read is skipped (see [`Sets::problems`]).
    pub fn list(&self) -> Vec<CheckSet> {
        let mut v: Vec<CheckSet> =
            self.files().iter().filter_map(|f| fs::read_to_string(f).ok()).filter_map(|t| from_toml(&t).ok()).collect();
        v.sort_by_key(|s| s.name.to_lowercase());
        v.dedup_by(|a, b| a.name == b.name);
        v
    }

    pub fn problems(&self) -> Vec<(PathBuf, String)> {
        self.files()
            .into_iter()
            .filter_map(|f| {
                let r = fs::read_to_string(&f).map_err(|e| e.to_string()).and_then(|t| from_toml(&t));
                r.err().map(|e| (f, e))
            })
            .collect()
    }

    pub fn get(&self, name: &str) -> Option<CheckSet> {
        self.list().into_iter().find(|s| s.name == name)
    }

    /// Saves `set`; `replacing` is the name it had when it was edited (a rename removes the old file).
    pub fn save(&self, set: &CheckSet, replacing: Option<&str>) -> Result<()> {
        if set.name.trim().is_empty() {
            return Err(Error::Invalid("A check set needs a name".into()));
        }
        fs::create_dir_all(&self.dir).map_err(Error::io(&self.dir))?;
        if let Some(old) = replacing.filter(|o| *o != set.name) {
            self.delete(old)?;
        }
        let path = self.file_of(&set.name);
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, to_toml(set)).map_err(Error::io(&tmp))?;
        fs::rename(&tmp, &path).map_err(Error::io(&path))
    }

    fn file_of(&self, name: &str) -> PathBuf {
        // a file whose set has this name keeps it; another set with the same slug gets a number
        let mut n = 1;
        loop {
            let p =
                self.dir.join(if n == 1 { format!("{}.toml", slug(name)) } else { format!("{}-{n}.toml", slug(name)) });
            match fs::read_to_string(&p) {
                Ok(t) if from_toml(&t).is_ok_and(|s| s.name != name) => n += 1,
                _ => return p,
            }
        }
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        for f in self.files() {
            if fs::read_to_string(&f).ok().and_then(|t| from_toml(&t).ok()).is_some_and(|s| s.name == name) {
                fs::remove_file(&f).map_err(Error::io(&f))?;
            }
        }
        Ok(())
    }
}

impl CheckSet {
    /// 32 hex digits that change when anything that changes a result changes.
    pub fn version(&self) -> String {
        let text = format!("{:?}", (&self.checks, &self.info_groups));
        blake3::hash(text.as_bytes()).to_hex()[..32].to_owned()
    }
}

fn result_path(root: &Path, key: &str) -> PathBuf {
    root.join("checks-results").join(format!("{key}.json"))
}

impl Store {
    /// Runs `set` on an opened recording, or reads the result kept for this recording's contents, this version of
    /// the set, the recipes it names and (for expressions that use them) the value labels and limits. Nothing is
    /// written beside the recording.
    pub fn run_checks(&self, base: &Opened, set: &CheckSet, recipes: &[Recipe]) -> Result<SetResult> {
        let rec = base.recording();
        let shared = self.expr_env.read().unwrap().clone();
        let mut h = blake3::Hasher::new();
        h.update(base.fingerprint().as_bytes());
        h.update(&rec.layout_id());
        h.update(set.version().as_bytes());
        let uses_env = set.checks.iter().any(|k| {
            let t = format!("{}{}", k.expect, k.when.as_deref().unwrap_or(""));
            t.contains('\'')
                || t.contains("past_")
                || recipes.iter().any(|r| r.expr.contains('\'') || r.expr.contains("past_"))
        });
        if uses_env {
            h.update(&shared.digest());
        }
        // recipes a check might name: any whose name appears in it
        for r in recipes {
            if set
                .checks
                .iter()
                .any(|k| k.expect.contains(&r.name) || k.when.as_deref().is_some_and(|w| w.contains(&r.name)))
            {
                h.update(r.name.as_bytes());
                h.update(r.expr.as_bytes());
            }
        }
        let key = h.finalize().to_hex()[..32].to_owned();
        let path = result_path(self.root(), &key);
        if let Some(r) = fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<SetResult>(&t).ok()) {
            return Ok(r);
        }
        let env = RecEnv::new(rec).with_labels(shared.labels).with_limits(shared.limits);
        let res = run_set(&env, set, rec.info().t_first.unwrap_or(0), recipes);
        let dir = path.parent().expect("a file in checks-results/");
        fs::create_dir_all(dir).map_err(Error::io(dir))?;
        // files with the same contents share a key, and may be checked at the same moment: each writes its own
        // temporary file
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let tmp = path.with_extension(format!(
            "{}-{}.tmp",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::write(&tmp, serde_json::to_string(&res).expect("results serialize")).map_err(Error::io(&tmp))?;
        fs::rename(&tmp, &path).map_err(Error::io(&path))?;
        Ok(res)
    }
}

impl Store {
    /// Runs `set` on a recording that is still growing (a live snapshot): not cached, since the recording's contents
    /// are different the next time. The app asks every couple of seconds; each run reads the whole recording so far.
    pub fn run_checks_live(&self, live: &Opened, set: &CheckSet, recipes: &[Recipe]) -> SetResult {
        let rec = live.recording();
        let shared = self.expr_env.read().unwrap().clone();
        let env = RecEnv::new(rec).with_labels(shared.labels).with_limits(shared.limits);
        run_set(&env, set, rec.info().t_first.unwrap_or(0), recipes)
    }
}

impl Store {
    /// One row on an opened recording, not kept: the editor's live preview.
    pub fn check_one(&self, base: &Opened, k: &Check, recipes: &[Recipe]) -> Outcome {
        let rec = base.recording();
        let shared = self.expr_env.read().unwrap().clone();
        let env = RecEnv::new(rec).with_labels(shared.labels).with_limits(shared.limits);
        run_check(&env, k, rec.info().t_first.unwrap_or(0), recipes)
    }
}

/// Which channels fail checks (Fail beats Warn) of every check of the results fall in a map of channel → failing (for badges).
pub fn failing_channels(set: &CheckSet, res: &SetResult) -> HashMap<String, Verdict> {
    let mut m: HashMap<String, Verdict> = HashMap::new();
    for (k, o) in set.checks.iter().zip(&res.outcomes) {
        if k.kind == Kind::Check && matches!(o.verdict, Verdict::Fail | Verdict::Warn) {
            for c in &o.channels {
                let e = m.entry(c.clone()).or_insert(o.verdict);
                if o.verdict.rank() > e.rank() {
                    *e = o.verdict;
                }
            }
        }
    }
    m
}
