//! Live triggers: watch a channel (or any expression, derived channels included) as samples arrive, and when it
//! crosses a level, add an event and tell whoever follows the recording to freeze a window around it, like an
//! oscilloscope. It runs where the samples are folded in ([`crate::Live::refresh`]), so it works without an app: the CLI
//! and the Python bindings follow a live file the same way.
//!
//! An expression is evaluated over the new samples only, after a warm-up of the samples its windows need (a rolling
//! mean looks back by its window), so the cost of a refresh follows the new data, not the length of the session. An
//! expression whose state can't be rebuilt from a bounded look-back (a running total, a filter that must start at the
//! beginning) can't trigger, and says so.

use serde::{Deserialize, Serialize};
use tlmbench_expr::{Env, Expr, Recipe, Val, eval_range, eval_warm, is_stateless, warm_rows_at};

use crate::checks::prepare;
use crate::derived::RecEnv;
use crate::events::Event;
use crate::{Opened, Store};

/// The category the events of triggers are in.
pub const TRIGGER_CATEGORY: &str = "trigger";

/// Most new samples looked at in one refresh: after a longer stall the newest are what matters.
const MAX_NEW: usize = 4 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Goes from below the level to at or above it.
    Rise,
    /// Goes from above the level to at or below it.
    Fall,
    /// Enters the region above the level.
    Above,
    /// Enters the region below the level.
    Below,
}

impl Kind {
    fn phrase(self) -> &'static str {
        match self {
            Kind::Rise => "rises through",
            Kind::Fall => "falls through",
            Kind::Above => "goes above",
            Kind::Below => "goes below",
        }
    }
}

/// What to watch for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    /// A channel's name, or an expression (a derived channel's recipe, or its name).
    pub channel: String,
    pub kind: Kind,
    pub level: f64,
    /// Seconds before and after the crossing the view should show.
    pub pre: f64,
    pub post: f64,
    /// Keeps watching after the first crossing (else it fires once).
    pub repeat: bool,
}

/// Where a trigger stands, for the strip's button.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Status {
    pub armed: bool,
    /// Times it has fired.
    pub count: u32,
    /// The latest crossing (ns, decimal) and whether the recording hasn't yet reached its post time.
    pub last: Option<String>,
    pub pending: bool,
    /// Why it can't run, when it can't.
    pub error: Option<String>,
    pub spec: Option<Spec>,
    /// The same crossing as an event rule's condition (the design's "reuse the rule engine"): put in a rule, it marks
    /// each crossing in a recorded file the way the trigger does live.
    pub rule: Option<String>,
}

impl Spec {
    /// The crossing as an expression over `x` (the channel as an expression): true at each sample that crosses.
    pub fn rule(&self, x: &str) -> String {
        let l = fmt_level(self.level);
        match self.kind {
            Kind::Rise => format!("prev({x}) < {l} && {x} >= {l}"),
            Kind::Fall => format!("prev({x}) > {l} && {x} <= {l}"),
            Kind::Above => format!("prev({x}) <= {l} && {x} > {l}"),
            Kind::Below => format!("prev({x}) >= {l} && {x} < {l}"),
        }
    }
}

/// An armed trigger: what it evaluates and how far it has looked.
#[derive(Debug)]
pub(crate) struct Trigger {
    spec: Spec,
    expr: Expr,
    /// Samples looked at, and the last value before them.
    seen: usize,
    prev: Option<f64>,
    /// The post time of the latest crossing: crossings before it are part of its window.
    quiet_until: i64,
    count: u32,
    last: Option<i64>,
    error: Option<String>,
    armed: bool,
    label: String,
    /// The channel as an expression: its quoted name, or what was typed.
    source: String,
}

impl Trigger {
    /// Arms `spec` on `opened` as it is now: only samples that arrive from here on can trigger it.
    pub(crate) fn arm(spec: Spec, opened: &Opened, recipes: &[Recipe]) -> Result<Trigger, String> {
        let rec = opened.recording();
        let env = RecEnv::new(rec);
        let named = rec.info().channels.iter().any(|c| c.name == spec.channel);
        let text = if named { format!("\"{}\"", spec.channel) } else { spec.channel.clone() };
        let expr = prepare(&text, recipes, &env).map_err(|(_, m)| m)?;
        let hz = env.hz().unwrap_or(1.0);
        if !is_stateless(&expr) && warm_rows_at(&expr, hz).is_none() {
            return Err("This channel needs the whole recording to compute, so it can't trigger live: a running \
                        total or a filter that starts at the beginning. Rolling windows and element-wise maths can."
                .into());
        }
        let label = spec.channel.clone();
        let source = if named { text.clone() } else { format!("({text})") };
        Ok(Trigger {
            seen: rec.info().samples,
            prev: None,
            quiet_until: i64::MIN,
            count: 0,
            last: None,
            error: None,
            armed: true,
            spec,
            expr,
            label,
            source,
        })
    }

    pub(crate) fn disarm(&mut self) {
        self.armed = false;
    }

    pub(crate) fn status(&self, opened: &Opened) -> Status {
        let t_last = opened.recording().info().t_last.unwrap_or(0);
        let post = (self.spec.post * 1e9) as i64;
        Status {
            armed: self.armed,
            count: self.count,
            last: self.last.map(|t| t.to_string()),
            pending: self.last.is_some_and(|t| t + post > t_last),
            error: self.error.clone(),
            spec: Some(self.spec.clone()),
            rule: Some(self.spec.rule(&self.source)),
        }
    }

    /// Looks at the samples that arrived since the last call; returns the events of the crossings in them.
    pub(crate) fn poll(&mut self, store: &Store, opened: &Opened) -> Vec<Event> {
        let rec = opened.recording();
        let info = rec.info();
        let n = info.samples;
        if !self.armed || n <= self.seen {
            return Vec::new();
        }
        let a = self.seen.max(n.saturating_sub(MAX_NEW));
        self.seen = n;
        let shared = store.expr_env.read().unwrap().clone();
        let env = RecEnv::new(rec).with_labels(shared.labels).with_limits(shared.limits);
        let got = if is_stateless(&self.expr) {
            eval_range(&self.expr, &env, a..n).map(Some)
        } else {
            // the window of an expression such as rolling_mean(x, 50ms) is as many samples as the rate says
            let warm = warm_rows_at(&self.expr, env.hz().unwrap_or(1.0)).unwrap_or(0);
            eval_warm(&self.expr, &env, a..n, warm)
        };
        let series = match got {
            Ok(Some(Val::Ser(s))) => s,
            Ok(_) => return Vec::new(), // a number never crosses; a window that fell short waits for more
            Err(e) => {
                self.error = Some(e.msg);
                self.armed = false;
                return Vec::new();
            }
        };
        let t_first = info.t_first.unwrap_or(0);
        let mut out = Vec::new();
        let level = self.spec.level;
        for (i, &v) in series.v.iter().enumerate() {
            if v.is_nan() {
                continue;
            }
            let Some(p) = self.prev.replace(v) else { continue };
            let hit = match self.spec.kind {
                Kind::Rise => p < level && v >= level,
                Kind::Fall => p > level && v <= level,
                Kind::Above => p <= level && v > level,
                Kind::Below => p >= level && v < level,
            };
            if !hit {
                continue;
            }
            let t = t_first + (series.time(i) * 1e9).round() as i64;
            if t < self.quiet_until {
                continue;
            }
            self.count += 1;
            self.last = Some(t);
            self.quiet_until = t + (self.spec.post * 1e9) as i64;
            out.push(Event {
                id: format!("trigger:{t:x}"),
                name: format!("Trigger: {} {} {}", self.label, self.spec.kind.phrase(), fmt_level(level)),
                start: t.to_string(),
                duration: "0".into(),
                category: TRIGGER_CATEGORY.into(),
                source: "rule".into(),
                channels: expr_channels(&self.expr),
                by: None,
            });
            if !self.spec.repeat {
                self.armed = false;
                break;
            }
        }
        out
    }
}

fn expr_channels(e: &Expr) -> Vec<String> {
    e.names().into_iter().map(|n| n.split('|').next().unwrap_or(n).trim().to_owned()).collect()
}

fn fmt_level(x: f64) -> String {
    if x == x.trunc() && x.abs() < 1e15 { format!("{}", x as i64) } else { format!("{x}") }
}
