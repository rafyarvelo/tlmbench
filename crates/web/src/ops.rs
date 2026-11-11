//! The commands: loading an export's data into the core, and the queries the viewer asks of it.

use serde::Deserialize;
use serde_json::{Value, json};
use tlmbench_core::chunks::Held;
use tlmbench_core::gaps::{DEFAULT_FACTOR, MIN_FACTOR};
use tlmbench_core::pyramid::{MIN_BUCKET_SAMPLES, Stats, Summary, span};
use tlmbench_core::recording::{ChannelInfo, ChannelOp, Channels, TimeSource};
use tlmbench_core::wire;
use tlmbench_core::{RateTimes, Recording, Trace, TraceKind, Viewport};

use crate::Reply;
use crate::data::{Ds, State, Tm, dtype};

const RAW_MAX: u32 = 10_000_000;
const MAX_ROWS: usize = 10_000;
/// The most raw samples a view reduces without a pyramid to help.
const EXACT_RAW_MAX: usize = 4_000_000;

fn time(s: &str) -> Result<i64, String> {
    s.parse::<i64>().map_err(|_| format!("{s:?} is not a time"))
}

fn ds(st: &State, file: u32) -> Result<&Ds, String> {
    st.files.get(file as usize).and_then(|f| f.as_ref()).ok_or_else(|| "that file is not open".to_string())
}

fn ds_mut(st: &mut State, file: u32) -> Result<&mut Ds, String> {
    st.files.get_mut(file as usize).and_then(|f| f.as_mut()).ok_or_else(|| "that file is not open".to_string())
}

fn arg<T: for<'a> Deserialize<'a>>(a: &Value, key: &str) -> Result<T, String> {
    serde_json::from_value(a.get(key).cloned().unwrap_or(Value::Null)).map_err(|e| format!("{key}: {e}"))
}

fn f64s(bin: &[u8]) -> Vec<f64> {
    bin.chunks_exact(8).map(|b| f64::from_le_bytes(b.try_into().unwrap())).collect()
}

fn i64s(bin: &[u8]) -> Vec<i64> {
    bin.chunks_exact(8).map(|b| i64::from_le_bytes(b.try_into().unwrap())).collect()
}

fn summaries(bin: &[u8]) -> Vec<Summary> {
    bin.chunks_exact(88)
        .map(|b| {
            let f = |i: usize| -> [u8; 8] { b[i * 8..i * 8 + 8].try_into().unwrap() };
            Summary {
                t_first: i64::from_le_bytes(f(0)),
                t_last: i64::from_le_bytes(f(1)),
                min: f64::from_le_bytes(f(2)),
                min_idx: u64::from_le_bytes(f(3)),
                max: f64::from_le_bytes(f(4)),
                max_idx: u64::from_le_bytes(f(5)),
                first: f64::from_le_bytes(f(6)),
                last: f64::from_le_bytes(f(7)),
                sum: f64::from_le_bytes(f(8)),
                sum_sq: f64::from_le_bytes(f(9)),
                count: u64::from_le_bytes(f(10)),
            }
        })
        .collect()
}

pub fn dispatch(st: &mut State, cmd: &str, a: &Value, bin: &[u8]) -> Result<Reply, String> {
    match cmd {
        "open_export" => open_export(st, a),
        // browser-only mode: a CSV from the page, and the columns the page keeps in its private storage
        "import_csv" => crate::import::import_csv(st, arg::<usize>(a, "file")?, &arg::<String>(a, "path")?, bin),
        "dump" => {
            let (meta, data) = crate::import::dump(st, arg::<usize>(a, "file")?)?;
            let json = serde_json::to_vec(&meta).map_err(|e| e.to_string())?;
            Ok(Reply::Bytes([(json.len() as u64).to_le_bytes().to_vec(), json, data].concat()))
        }
        "load" => {
            let n = u64::from_le_bytes(bin.get(..8).ok_or("empty")?.try_into().unwrap()) as usize;
            let meta: Value = serde_json::from_slice(bin.get(8..8 + n).ok_or("short")?).map_err(|e| e.to_string())?;
            crate::import::load(st, arg::<usize>(a, "file")?, &arg::<String>(a, "path")?, &meta, &bin[8 + n..])
        }
        "opened" => {
            let k: u32 = arg(a, "file")?;
            Ok(Reply::Json(opened(ds(st, k)?, k)))
        }
        "put_times" => {
            let d = ds_mut(st, arg(a, "file")?)?;
            if let Tm::Col(v) = &mut d.tm {
                v.extend(i64s(bin));
            }
            if d.times_ready() {
                d.refresh();
            }
            Ok(Reply::Json(Value::Null))
        }
        "put_raw" => {
            let d = ds_mut(st, arg(a, "file")?)?;
            let c: usize = arg(a, "chan")?;
            let col = d.cols.get_mut(c).ok_or("no such channel")?;
            // `start`: the first sample of these values; without it they continue the channel's last span
            let start = match a.get("start") {
                Some(_) => arg::<usize>(a, "start")?,
                None => col.raw.next(),
            };
            col.raw.put(start, f64s(bin))?;
            Ok(Reply::Json(Value::Null))
        }
        "put_pyr" => {
            let d = ds_mut(st, arg(a, "file")?)?;
            let c: usize = arg(a, "chan")?;
            let lens: Vec<usize> = arg(a, "levels")?;
            let first: u32 = arg(a, "first_level")?;
            let all = summaries(bin);
            let col = d.cols.get_mut(c).ok_or("no such channel")?;
            col.levels.clear();
            let mut at = 0;
            for n in lens {
                col.levels.push(all.get(at..at + n).ok_or("the pyramid file is short")?.to_vec());
                at += n;
            }
            col.first_level = first;
            Ok(Reply::Json(Value::Null))
        }
        "build_pyramids" => {
            let d = ds_mut(st, arg(a, "file")?)?;
            for c in 0..d.cols.len() {
                if d.cols[c].levels.is_empty() {
                    d.build_pyramid(c);
                }
            }
            Ok(Reply::Json(Value::Null))
        }
        "close" => {
            if let Some(f) = st.files.get_mut(arg::<u32>(a, "file")? as usize) {
                *f = None;
            }
            Ok(Reply::Json(Value::Null))
        }
        // the BLAKE3 of the bytes sent, as hex: the page checks each file it fetches against `export.json`
        "hash" => Ok(Reply::Json(Value::String(blake3::hash(bin).to_hex().to_string()))),
        "view" => view(st, a),
        "values_at" => values_at(st, a),
        "stats" => stats(st, a),
        "widgets_frame" => widgets_frame(st, a),
        "rows" => rows(st, a),
        _ => Err(format!("the viewer has no command {cmd:?}")),
    }
}

/// `files[k]` of the export's manifest.
fn open_export(st: &mut State, a: &Value) -> Result<Reply, String> {
    let m = a.get("file").ok_or("open_export: no file")?;
    let k = m["k"].as_u64().ok_or("no k")? as usize;
    let len = m["samples"].as_u64().ok_or("no samples")? as usize;
    let t = &m["time"];
    let (time, tm) = match t["kind"].as_str() {
        Some("rate") => {
            let t0: i64 = t["t0"].as_str().unwrap_or("0").parse().map_err(|_| "bad t0")?;
            let hz = t["hz"].as_f64().unwrap_or(1.0);
            (TimeSource::Rate { hz }, Tm::Rate(RateTimes::from_rate(t0, hz, len)))
        }
        _ => (TimeSource::Column { name: "time".into() }, Tm::Col(Vec::with_capacity(len))),
    };
    let channels = m["channels"]
        .as_array()
        .ok_or("no channels")?
        .iter()
        .map(|c| ChannelInfo {
            name: c["name"].as_str().unwrap_or("").to_owned(),
            unit: c["unit"].as_str().map(str::to_owned),
            dtype: dtype(c["dtype"].as_str().unwrap_or("f64")),
            category: c["category"].as_str().map(str::to_owned),
        })
        .collect();
    let d = Ds::new(
        m["path"].as_str().unwrap_or(""),
        m["format"].as_str().unwrap_or("export"),
        m["part"].as_str().map(str::to_owned),
        len,
        time,
        tm,
        channels,
    );
    let reply = opened(&d, k as u32);
    if st.files.len() <= k {
        st.files.resize_with(k + 1, || None);
    }
    st.files[k] = Some(d);
    Ok(Reply::Json(reply))
}

pub fn opened(d: &Ds, id: u32) -> Value {
    let range = d.info.t_first.zip(d.info.t_last).map(|(a, b)| (a.to_string(), b.to_string()));
    json!({ "id": id, "info": d.info, "range": range, "status": { "state": "built" }, "derived": [], "skipped": [], "missing": [] })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ViewEntry {
    id: u32,
    channels: Vec<u32>,
    #[serde(default)]
    gap_factors: Vec<f64>,
    t0: String,
    t1: String,
    width_px: u32,
    #[serde(default)]
    raw_max: u32,
    #[serde(default)]
    connect_gaps: bool,
}

fn view(st: &State, a: &Value) -> Result<Reply, String> {
    let d = ds(st, arg(a, "file")?)?;
    let generation: u32 = arg(a, "generation")?;
    let coarse_first = a.get("precision").and_then(Value::as_str) == Some("coarse");
    let entries: Vec<ViewEntry> = arg(a, "entries")?;
    let n = d.info.channels.len();
    let mut parts = Vec::new();
    for e in entries {
        let vp = Viewport {
            t0: time(&e.t0)?,
            t1: time(&e.t1)?,
            width_px: e.width_px.clamp(1, 16_384),
            raw_max: e.raw_max.min(RAW_MAX),
        };
        let (lo, hi) = (d.lower_bound(vp.t0), d.lower_bound(vp.t1));
        let coarse_pays = hi.saturating_sub(lo) / vp.width_px.max(1) as usize >= MIN_BUCKET_SAMPLES;
        let mut views: Vec<(usize, Trace, Vec<u32>, bool, bool)> = Vec::new();
        for (i, &c) in e.channels.iter().enumerate().filter(|&(_, &c)| (c as usize) < n) {
            let c = c as usize;
            let factor = e.gap_factors.get(i).copied().unwrap_or(DEFAULT_FACTOR).max(MIN_FACTOR);
            let breaks = d.gaps.breaks(factor);
            let held = Held::default();
            let pyr = d.pyramid(c);
            let col = &d.cols[c];
            let whole = col.raw.slice(d.len).is_some();
            // the samples the view reads (and one either side) are all here: an exact view
            let covered = whole || col.raw.covers(lo.saturating_sub(1), (hi + 1).min(d.len));
            // a trimmed pyramid (its finest level is above 4) never goes through the core's; a view of very many raw
            // samples without one is answered from the summaries
            let exact_ok = covered && (pyr.is_some() || hi.saturating_sub(lo) <= EXACT_RAW_MAX);
            let (trace, coarse) = match &pyr {
                // raw samples missing for the view: the pyramid answers, labelled coarse
                Some(_) if !exact_ok && !coarse_pays => (summary_trace(d, c, vp), true),
                Some(p) if !exact_ok => (d.view_coarse(c, vp, p, &breaks, &held), true),
                Some(p) if coarse_first && coarse_pays && whole => (d.view_coarse(c, vp, p, &breaks, &held), true),
                p if exact_ok => (d.view(c, vp, p.as_ref(), &breaks, &held), false),
                // a trimmed pyramid: its finest level that fits, every value an exact aggregate
                None if !col.levels.is_empty() => (summary_trace(d, c, vp), true),
                _ => (Trace { kind: TraceKind::Raw, bucket_ns: vp.bucket_ns(), t: vec![], y: vec![] }, false),
            };
            let (trace, dashed) =
                if e.connect_gaps { tlmbench_core::connect_gaps(&trace) } else { (trace, Vec::new()) };
            views.push((c, trace, dashed, coarse, pyr.is_some()));
        }
        let outs: Vec<wire::Out> = views
            .iter()
            .map(|(c, t, dashed, coarse, from_pyramid)| wire::Out {
                channel: *c as u32,
                trace: t,
                reduce_us: 0,
                coarse: *coarse,
                from_pyramid: *from_pyramid,
                dashed,
            })
            .collect();
        parts.push((e.id, wire::encode(generation, vp, None, &outs)));
    }
    Ok(Reply::Bytes(wire::encode_batch(generation, &parts)))
}

/// A channel whose raw samples aren't all here, in a view of fewer samples than the raw samples it would need: the
/// stored level that fits (the finest with about two summaries a pixel, or the finest there is), each non-empty
/// summary contributing its lowest and highest value at their own times, in order (exact aggregates, never a sample
/// that isn't there); a time jump over four median intervals breaks the line. Labelled coarse by the caller.
fn summary_trace(d: &Ds, c: usize, vp: Viewport) -> Trace {
    let empty = Trace { kind: TraceKind::M4, bucket_ns: vp.bucket_ns(), t: vec![], y: vec![] };
    let col = &d.cols[c];
    if col.levels.is_empty() {
        return empty;
    }
    let (first, top) = (col.first_level, col.first_level + col.levels.len() as u32 - 1);
    let (lo, hi) = (d.lower_bound(vp.t0).saturating_sub(1), (d.lower_bound(vp.t1) + 1).min(d.len));
    let per_px = (hi.saturating_sub(lo) / vp.width_px.max(1) as usize).max(1);
    let k = (first..=top).rev().find(|&k| 2 * span(k) <= per_px).unwrap_or(first);
    let (w, lk) = (span(k), &col.levels[(k - first) as usize]);
    let (a, b) = (lo / w, hi.div_ceil(w).min(lk.len()));
    let median = d.gaps.median_ns;
    let (mut t, mut y) = (Vec::new(), Vec::new());
    let mut prev_last: Option<i64> = None;
    for s in lk.get(a..b).unwrap_or_default() {
        if s.count == 0 {
            continue;
        }
        if let Some(p) = prev_last
            && median > 0
            && s.t_first - p > 4 * median
        {
            t.push(p + 1);
            y.push(f32::NAN);
        }
        let mut pairs = [(s.min_idx, s.min), (s.max_idx, s.max)];
        pairs.sort_by_key(|p| p.0);
        for (k, (idx, v)) in pairs.into_iter().enumerate() {
            if k == 1 && idx == pairs[0].0 {
                continue;
            }
            t.push(d.time_at(idx as usize));
            y.push(v as f32);
        }
        prev_last = Some(s.t_last);
    }
    Trace { kind: TraceKind::M4, bucket_ns: vp.bucket_ns(), t, y }
}

/// The summary of the samples `lo..hi` of a channel: whole summaries of the stored levels between, and the raw samples
/// of the two ragged edges (up to the finest stored level's grid). `None` unless every raw sample an edge needs is here,
/// or the channel has no pyramid: exact or nothing.
fn exact_summary(d: &Ds, c: usize, lo: usize, hi: usize) -> Option<Summary> {
    struct Edge(usize, usize);
    impl ChannelOp for Edge {
        type Out = Summary;
        fn run<T: tlmbench_core::Times + ?Sized, S: tlmbench_core::Samples + ?Sized>(
            self,
            times: &T,
            values: &S,
        ) -> Summary {
            Summary::of(times, values, self.0, self.1)
        }
    }
    let col = &d.cols[c];
    if col.levels.is_empty() || lo >= hi {
        return None;
    }
    let (first, top) = (col.first_level, col.first_level + col.levels.len() as u32 - 1);
    let w = span(first);
    let a1 = lo.next_multiple_of(w).min(hi);
    let b1 = (if hi == d.len { hi } else { hi / w * w }).max(a1);
    let mut acc: Option<Summary> = None;
    let mut add = |s: Summary| acc = Some(acc.map_or(s, |p| p.merge(&s)));
    if lo < a1 {
        col.raw.covers(lo, a1).then_some(())?;
        add(d.dispatch(c, Edge(lo, a1)));
    }
    let mut i = a1;
    while i < b1 {
        let k = (first..=top).rev().find(|&k| i.is_multiple_of(span(k)) && (i + span(k)).min(d.len) <= b1)?;
        add(*col.levels[(k - first) as usize].get(i >> (2 * k))?);
        i = (i + span(k)).min(d.len);
    }
    if b1 < hi {
        col.raw.covers(b1, hi).then_some(())?;
        add(d.dispatch(c, Edge(b1, hi)));
    }
    acc
}

fn nearest(d: &Ds, t: i64) -> Option<usize> {
    let n = d.len;
    if n == 0 || !d.times_ready() {
        return None;
    }
    let i = d.lower_bound(t);
    Some(match (i.checked_sub(1), (i < n).then_some(i)) {
        (Some(a), Some(b)) => {
            if t - d.time_at(a) <= d.time_at(b) - t {
                a
            } else {
                b
            }
        }
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => return None,
    })
}

fn values_at(st: &State, a: &Value) -> Result<Reply, String> {
    let d = ds(st, arg(a, "file")?)?;
    let channels: Vec<u32> = arg(a, "channels")?;
    let t = time(&arg::<String>(a, "t")?)?;
    let Some(i) = nearest(d, t) else { return Ok(Reply::Json(json!([]))) };
    let ts = d.time_at(i).to_string();
    // a channel whose raw samples aren't in the export has no value to show: never a pyramid value as a sample
    let out: Vec<Value> = channels
        .iter()
        .filter(|&&c| (c as usize) < d.info.channels.len())
        .filter_map(|&c| {
            let v = d.cols[c as usize].raw.get(i)?;
            Some(json!({ "channel": c, "index": i, "t": ts, "value": (!v.is_nan()).then_some(v) }))
        })
        .collect();
    Ok(Reply::Json(Value::Array(out)))
}

/// The exact statistics of samples `lo..hi`, when the channel has what they need (see [`exact_summary`]).
fn stats_of(d: &Ds, c: usize, lo: usize, hi: usize) -> Option<Stats> {
    match d.pyramid(c) {
        Some(py) if d.cols[c].raw.slice(d.len).is_some() => Some(d.stats(c, &py, lo, hi, &Held::default())),
        _ => exact_summary(d, c, lo, hi).map(Stats::from),
    }
}

fn stats(st: &State, a: &Value) -> Result<Reply, String> {
    let d = ds(st, arg(a, "file")?)?;
    let channels: Vec<u32> = arg(a, "channels")?;
    let span: Value = arg(a, "span")?;
    let (t0, t1) = (time(span["t0"].as_str().unwrap_or(""))?, time(span["t1"].as_str().unwrap_or(""))?);
    let (lo, hi) = (d.lower_bound(t0), d.lower_bound(t1));
    let num = |v: f64| (!v.is_nan()).then_some(v);
    let mut out = Vec::new();
    for &c in channels.iter().filter(|&&c| (c as usize) < d.info.channels.len()) {
        let c = c as usize;
        if d.cols[c].levels.is_empty() {
            continue;
        }
        if lo >= hi {
            out.push(json!({ "channel": c, "count": 0, "min": null, "minT": null, "max": null, "maxT": null, "mean": null, "std": null }));
            continue;
        }
        // exact stats need raw samples at the span's ragged edges: a channel without them is left out
        let Some(s) = stats_of(d, c, lo, hi) else { continue };
        let at = |m: Option<(f64, usize)>| m.map(|(_, i)| d.time_at(i).to_string());
        out.push(json!({ "channel": c, "count": s.count, "min": s.min.map(|m| m.0), "minT": at(s.min), "max": s.max.map(|m| m.0), "maxT": at(s.max), "mean": num(s.mean), "std": num(s.std) }));
    }
    Ok(Reply::Json(Value::Array(out)))
}

/// How far back a run of equal values is followed.
const LOOK_BACK: usize = 4 << 20;

/// The time the run of equal values that sample `i` of channel `c` is in began, when the samples back to it are held.
fn run_start(d: &Ds, c: usize, i: usize) -> Option<i64> {
    let raw = &d.cols[c].raw;
    let cur = raw.get(i)?;
    let same = |v: f64| v == cur || (v.is_nan() && cur.is_nan());
    let mut j = i;
    while j > 0 && i - j < LOOK_BACK {
        // a sample that isn't held may be a different value: the start is unknown
        if !same(raw.get(j - 1)?) {
            return Some(d.time_at(j));
        }
        j -= 1;
    }
    Some(d.time_at(j))
}

/// Every widget of a Gauges pane in one call (see the app's `widgets_frame`): values from held raw samples only,
/// the lowest and highest value from the exact statistics, a run's start while its samples are held, and a trace
/// drawn from the summaries.
fn widgets_frame(st: &State, a: &Value) -> Result<Reply, String> {
    let d = ds(st, arg(a, "file")?)?;
    let t = time(&arg::<String>(a, "t")?)?;
    let view: Value = arg(a, "view")?;
    let (v0, v1) = (time(view["t0"].as_str().unwrap_or(""))?, time(view["t1"].as_str().unwrap_or(""))?);
    let asks: Vec<Value> = arg(a, "widgets")?;
    let Some(i) = nearest(d, t) else { return Ok(Reply::Json(Value::Null)) };
    let ts = d.time_at(i);
    let n = d.info.channels.len();
    let num = |v: f64| (!v.is_nan()).then_some(v);
    let widgets: Vec<Value> = asks
        .iter()
        .map(|w| {
            let c = (w["channel"].as_u64().unwrap_or(0) as usize).min(n.saturating_sub(1));
            let flag = |k: &str| w[k].as_bool().unwrap_or(false);
            let value = d.cols[c].raw.get(i).and_then(num);
            let (mut min, mut max) = (None, None);
            if flag("minmax") {
                let end = ts.min(v1.saturating_sub(1)).saturating_add(1);
                let start = v0.min(ts);
                let (lo, hi) = (d.lower_bound(start), d.lower_bound(end.max(start + 1)));
                if let Some(s) = stats_of(d, c, lo, hi).filter(|_| lo < hi) {
                    (min, max) = (s.min.map(|m| m.0), s.max.map(|m| m.0));
                }
            }
            let run = flag("run").then(|| run_start(d, c, i)).flatten().map(|t| t.to_string());
            let px = w["trace"].as_u64().unwrap_or(0).min(600) as u32;
            let mut trace: Vec<Option<f32>> = Vec::new();
            if px > 0 {
                let vp = Viewport { t0: v0, t1: v1, width_px: px, raw_max: 0 };
                let span = (v1 - v0).max(1) as f64;
                let tr = summary_trace(d, c, vp);
                for (&x, &y) in tr.t.iter().zip(&tr.y) {
                    trace.push(Some(((x - v0) as f64 / span) as f32));
                    trace.push((!y.is_nan()).then_some(y));
                }
            }
            json!({ "value": value, "min": min, "max": max, "runStart": run, "trace": trace })
        })
        .collect();
    Ok(Reply::Json(json!({ "index": i, "t": ts.to_string(), "widgets": widgets })))
}

fn rows(st: &State, a: &Value) -> Result<Reply, String> {
    let d = ds(st, arg(a, "file")?)?;
    let channels: Vec<u32> = arg(a, "channels")?;
    let span: Value = arg(a, "span")?;
    let offset: u64 = arg(a, "offset")?;
    let limit: u32 = arg(a, "limit")?;
    let (t0, t1) = (time(span["t0"].as_str().unwrap_or(""))?, time(span["t1"].as_str().unwrap_or(""))?);
    let chans: Vec<usize> = channels.iter().map(|&c| c as usize).filter(|&c| c < d.info.channels.len()).collect();
    let lo = d.lower_bound(t0);
    let hi = d.lower_bound(t1).max(lo);
    let start = (lo as u64).saturating_add(offset).min(hi as u64) as usize;
    let end = start + (hi - start).min(limit as usize).min(MAX_ROWS);
    let k = end - start;
    let mut out = Vec::with_capacity(32 + 8 * k * (1 + chans.len()));
    out.extend_from_slice(b"TLMR");
    out.extend_from_slice(&(chans.len() as u32).to_le_bytes());
    out.extend_from_slice(&(start as u64).to_le_bytes());
    out.extend_from_slice(&((hi - lo) as u64).to_le_bytes());
    out.extend_from_slice(&(k as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    for t in d.read_times(start, end) {
        out.extend_from_slice(&t.to_le_bytes());
    }
    for &c in &chans {
        // a sample the export doesn't hold is a blank cell
        (start..end).for_each(|i| out.extend_from_slice(&d.cols[c].raw.get(i).unwrap_or(f64::NAN).to_le_bytes()));
    }
    Ok(Reply::Bytes(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Raw;
    use tlmbench_core::pyramid::{build_levels, summarize};

    const N: usize = 100_000;

    fn value(i: usize) -> f64 {
        ((i as f64) * 0.013).sin() * 50.0 + (i % 97) as f64 * 0.25
    }

    /// A 1 kHz channel of `N` samples whose pyramid has its `dropped` finest levels left out and whose raw samples
    /// are the `windows` only.
    fn trimmed(dropped: usize, windows: &[(usize, usize)]) -> Ds {
        let mut d = Ds::new(
            "t",
            "export",
            None,
            N,
            TimeSource::Rate { hz: 1000.0 },
            Tm::Rate(RateTimes::from_rate(0, 1000.0, N)),
            vec![ChannelInfo { name: "x".into(), unit: None, dtype: "f64", category: None }],
        );
        let vals: Vec<f64> = (0..N).map(value).collect();
        let Tm::Rate(t) = &d.tm else { unreachable!() };
        let levels = build_levels(summarize(t, &vals, 0, N));
        d.cols[0].levels = levels.into_iter().skip(dropped).collect();
        d.cols[0].first_level = 4 + dropped as u32;
        for &(a, b) in windows {
            d.cols[0].raw.put(a, vals[a..b].to_vec()).unwrap();
        }
        d
    }

    fn brute(a: usize, b: usize) -> (f64, f64, f64) {
        let v: Vec<f64> = (a..b).map(value).collect();
        (
            v.iter().cloned().fold(f64::INFINITY, f64::min),
            v.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            v.iter().sum(),
        )
    }

    #[test]
    fn stats_are_exact_with_raw_edges_on_a_trimmed_pyramid() {
        // level 6 is the finest kept: whole summaries are 4096 samples, so edges up to 4095 samples are raw
        let d = trimmed(2, &[(10_000, 14_096), (60_000, 70_000)]);
        for (lo, hi) in [(10_300, 13_900), (10_300, 12_000), (61_000, 66_000)] {
            let s = exact_summary(&d, 0, lo, hi);
            // 10_300..13_900 and 10_300..12_000 contain no whole level-6 summary, so they need only raw
            let s = s.unwrap_or_else(|| panic!("{lo}..{hi} should be exact"));
            let (mn, mx, sum) = brute(lo, hi);
            assert_eq!((s.count as usize, s.min, s.max), (hi - lo, mn, mx), "{lo}..{hi}");
            assert!((s.sum - sum).abs() < 1e-6 * sum.abs().max(1.0), "{lo}..{hi}");
        }
        // a span whose edge falls where no raw sample is held is not answered, never approximated
        assert!(exact_summary(&d, 0, 20_000, 40_000).is_none());
        assert!(exact_summary(&d, 0, 10_300, 30_000).is_none());
    }

    #[test]
    fn a_whole_pyramid_with_raw_only_at_the_edges_answers_whole_spans() {
        let d = trimmed(0, &[(0, 300), (N - 300, N)]);
        let s = exact_summary(&d, 0, 100, N).expect("edges held");
        let (mn, mx, _) = brute(100, N);
        assert_eq!((s.count as usize, s.min, s.max), (N - 100, mn, mx));
        assert!(exact_summary(&d, 0, 5_000, 6_000).is_none(), "its edges are not held");
    }

    #[test]
    fn the_summary_view_shows_only_values_the_data_has() {
        let d = trimmed(3, &[]);
        let vp = Viewport { t0: 0, t1: 100_000_000_000, width_px: 200, raw_max: 0 };
        let tr = summary_trace(&d, 0, vp);
        assert!(!tr.t.is_empty());
        // every point is a sample's own time and value: a summary extreme, at the time of the sample that made it
        for (t, y) in tr.t.iter().zip(&tr.y) {
            if y.is_nan() {
                continue;
            }
            let i = d.lower_bound(*t);
            assert_eq!(d.time_at(i), *t);
            assert_eq!(*y, value(i) as f32, "sample {i}");
        }
    }

    #[test]
    fn hash_is_blake3_hex() {
        let mut st = State::default();
        let Reply::Json(Value::String(h)) = dispatch(&mut st, "hash", &Value::Null, b"abc").unwrap() else { panic!() };
        assert_eq!(h, blake3::hash(b"abc").to_hex().to_string());
        assert_eq!(h.len(), 64);
    }

    #[test]
    fn a_widgets_frame_answers_from_held_samples_and_exact_statistics() {
        let d = trimmed(2, &[(1_000, 6_000)]);
        let mut st = State::default();
        st.files.push(Some(d));
        let frame = |t_ms: i64, v0_ms: i64| {
            let a = json!({ "file": 0, "t": (t_ms * 1_000_000).to_string(),
                "view": { "t0": (v0_ms * 1_000_000).to_string(), "t1": "100000000000" },
                "widgets": [{ "channel": 0, "minmax": true, "run": true, "trace": 50 }] });
            match widgets_frame(&st, &a).unwrap() {
                Reply::Json(v) => v,
                _ => unreachable!(),
            }
        };
        // inside the window: the value, exact min and max over 1_500..=4_000, a run start, and a drawn trace
        let f = frame(4_000, 1_500);
        let w = &f["widgets"][0];
        assert_eq!(w["value"].as_f64(), Some(value(4_000)));
        let (mn, mx, _) = brute(1_500, 4_001);
        assert_eq!((w["min"].as_f64(), w["max"].as_f64()), (Some(mn), Some(mx)));
        assert!(w["runStart"].is_string() && !w["trace"].as_array().unwrap().is_empty());
        // outside every window: no value, no run, and min and max only when the edges are held (they are not)
        let f = frame(50_000, 20_000);
        let w = &f["widgets"][0];
        assert!(w["value"].is_null() && w["runStart"].is_null() && w["min"].is_null() && w["max"].is_null());
        assert!(!w["trace"].as_array().unwrap().is_empty(), "the trace comes from the summaries");
    }

    #[test]
    fn values_come_only_from_held_raw_samples() {
        let d = trimmed(2, &[(1_000, 2_000)]);
        let mut st = State::default();
        st.files.push(Some(d));
        let at = |t_ms: i64| {
            let a = json!({ "file": 0, "channels": [0], "t": (t_ms * 1_000_000).to_string() });
            match values_at(&st, &a).unwrap() {
                Reply::Json(Value::Array(v)) => v,
                _ => unreachable!(),
            }
        };
        let inside = at(1_500);
        assert_eq!(inside.len(), 1);
        assert_eq!(inside[0]["value"].as_f64(), Some(value(1_500)));
        assert!(at(5_000).is_empty(), "a sample not in the export has no value, and a summary is never one");
    }

    #[test]
    fn a_view_inside_a_window_is_exact_and_outside_it_is_coarse() {
        let d = trimmed(2, &[(20_000, 30_000)]);
        let mut st = State::default();
        st.files.push(Some(d));
        let view_of = |t0_ms: i64, t1_ms: i64| {
            let a = json!({ "file": 0, "generation": 1, "entries": [{ "id": 7, "channels": [0],
                "t0": (t0_ms * 1_000_000).to_string(), "t1": (t1_ms * 1_000_000).to_string(), "widthPx": 300 }] });
            let Reply::Bytes(b) = view(&st, &a).unwrap() else { unreachable!() };
            b
        };
        let (inside, outside) = (view_of(22_000, 25_000), view_of(40_000, 80_000));
        assert_ne!(inside, outside);
        // the wire format marks a coarse channel: decode both and compare the flag
        let flag = |b: &[u8]| {
            tlmbench_core::wire::decode_batch(b)
                .map(|bt| bt.entries[0].1.traces.iter().map(|t| t.coarse).collect::<Vec<_>>())
        };
        assert_eq!(flag(&inside), Some(vec![false]));
        assert_eq!(flag(&outside), Some(vec![true]));
        // no window and no pyramid: nothing to draw, and not called coarse
        let mut d2 = trimmed(0, &[]);
        d2.cols[0].levels.clear();
        let mut st2 = State::default();
        st2.files.push(Some(d2));
        let a = json!({ "file": 0, "generation": 1, "entries": [{ "id": 7, "channels": [0], "t0": "0", "t1": "100000000000", "widthPx": 300 }] });
        let Reply::Bytes(b) = view(&st2, &a).unwrap() else { unreachable!() };
        let tr = &tlmbench_core::wire::decode_batch(&b).unwrap().entries[0].1.traces[0];
        assert!(tr.x.is_empty() && !tr.coarse);
    }
}
