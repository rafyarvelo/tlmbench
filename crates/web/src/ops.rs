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
use crate::group::{route, slots};

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
        "import_file" => crate::import::import_file(st, arg::<usize>(a, "file")?, &arg::<String>(a, "path")?, bin),
        "import_tlm" => crate::import::import_tlm(st, arg::<usize>(a, "file")?, &arg::<String>(a, "path")?, bin),
        "import_csv" => crate::import::import_csv(st, arg::<usize>(a, "file")?, &arg::<String>(a, "path")?, bin),
        // a binary file laid out by a descriptor the page keeps: its packet types, then one file per type
        "described_parts" => Ok(Reply::Json(json!(crate::import::described_parts(&a["descriptor"])?))),
        "import_described" => crate::import::import_described(
            st,
            arg::<usize>(a, "file")?,
            &arg::<String>(a, "path")?,
            bin,
            &a["descriptor"],
            a["part"].as_str(),
        ),
        // the descriptor form over the first bytes of a file (`bin`), as the app's commands answer it
        "descriptor_preview" => Ok(Reply::Json(json!(match crate::import::descriptor_of(&a["descriptor"]) {
            Ok(d) => crate::preview::preview(bin, &d),
            Err(e) => crate::preview::Preview { problem: Some(e), ..Default::default() },
        }))),
        "descriptor_find_frames" => Ok(Reply::Json(json!(crate::descriptor::find_frames(bin)))),
        "config_of" => Ok(Reply::Json(crate::import::config_of(&a["files"])?)),
        "looks_binary" => Ok(Reply::Json(json!(crate::preview::looks_binary(bin)))),
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
            let id = arg::<u32>(a, "file")?;
            // closing a tab's id closes the tab, not its files (they stay open under their own ids)
            if st.groups.remove(&id).is_none()
                && let Some(f) = st.files.get_mut(id as usize)
            {
                *f = None;
            }
            Ok(Reply::Json(Value::Null))
        }
        "group_set" => Ok(Reply::Json(crate::group::set(
            st,
            a.get("group").and_then(Value::as_u64).map(|g| g as u32),
            a.get("members").unwrap_or(&Value::Null),
        )?)),
        "group_state" => Ok(Reply::Json(crate::group::opened(st, arg(a, "group")?)?)),
        "group_map" => Ok(Reply::Json(crate::group::set_map(
            st,
            arg(a, "group")?,
            arg(a, "slot")?,
            a.get("knots").unwrap_or(&Value::Null),
        )?)),
        // the BLAKE3 of the bytes sent, as hex: the page checks each file it fetches against `export.json`
        "hash" => Ok(Reply::Json(Value::String(blake3::hash(bin).to_hex().to_string()))),
        "view" => view(st, a),
        "values_at" => values_at(st, a),
        "stats" => stats(st, a),
        "widgets_frame" => widgets_frame(st, a),
        "xy_points" => xy_points(st, a),
        "spectrum" => spectrum(st, a),
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

/// One channel's trace over `vp` (its file's own clock): the exact view where the raw samples are held, else the
/// summaries, labelled coarse. Returns the trace, whether it is coarse, and whether a pyramid answered.
fn channel_trace(d: &Ds, c: usize, vp: Viewport, factor: f64, coarse_first: bool) -> (Trace, bool, bool) {
    let (lo, hi) = (d.lower_bound(vp.t0), d.lower_bound(vp.t1));
    let coarse_pays = hi.saturating_sub(lo) / vp.width_px.max(1) as usize >= MIN_BUCKET_SAMPLES;
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
    (trace, coarse, pyr.is_some())
}

fn view(st: &State, a: &Value) -> Result<Reply, String> {
    let sl = slots(st, arg(a, "file")?)?;
    let generation: u32 = arg(a, "generation")?;
    let coarse_first = a.get("precision").and_then(Value::as_str) == Some("coarse");
    let entries: Vec<ViewEntry> = arg(a, "entries")?;
    let mut parts = Vec::new();
    for e in entries {
        let vp = Viewport {
            t0: time(&e.t0)?,
            t1: time(&e.t1)?,
            width_px: e.width_px.clamp(1, 16_384),
            raw_max: e.raw_max.min(RAW_MAX),
        };
        let mut views: Vec<(usize, Trace, Vec<u32>, bool, bool)> = Vec::new();
        for (i, &c) in e.channels.iter().enumerate() {
            let Some((si, local)) = route(&sl, c as usize) else { continue };
            let s = &sl[si];
            // the shared span goes through the inverse of the file's map; the times that come back go through the map
            let own = Viewport { t0: s.map.inverse(vp.t0), t1: s.map.inverse(vp.t1), ..vp };
            let factor = e.gap_factors.get(i).copied().unwrap_or(DEFAULT_FACTOR).max(MIN_FACTOR);
            let (mut trace, coarse, from_pyramid) = channel_trace(s.ds, local, own, factor, coarse_first);
            if !s.map.knots.is_empty() {
                trace.t.iter_mut().for_each(|t| *t = s.map.apply(*t));
                trace.bucket_ns = (s.map.apply(trace.bucket_ns) - s.map.apply(0)).max(1);
            }
            let (trace, dashed) =
                if e.connect_gaps { tlmbench_core::connect_gaps(&trace) } else { (trace, Vec::new()) };
            views.push((c as usize, trace, dashed, coarse, from_pyramid));
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
    let sl = slots(st, arg(a, "file")?)?;
    let channels: Vec<u32> = arg(a, "channels")?;
    let t = time(&arg::<String>(a, "t")?)?;
    // each channel's sample nearest the shared time, in its own file (files at other rates each give a real sample);
    // a channel whose raw sample isn't in the export has no value to show: never a pyramid value as one
    let out: Vec<Value> = channels
        .iter()
        .filter_map(|&c| {
            let (si, local) = route(&sl, c as usize)?;
            let s = &sl[si];
            let i = nearest(s.ds, s.map.inverse(t))?;
            let v = s.ds.cols[local].raw.get(i)?;
            let ts = s.map.apply(s.ds.time_at(i)).to_string();
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
    let sl = slots(st, arg(a, "file")?)?;
    let channels: Vec<u32> = arg(a, "channels")?;
    let span: Value = arg(a, "span")?;
    let (t0, t1) = (time(span["t0"].as_str().unwrap_or(""))?, time(span["t1"].as_str().unwrap_or(""))?);
    let num = |v: f64| (!v.is_nan()).then_some(v);
    let mut out = Vec::new();
    for &c in &channels {
        let Some((si, local)) = route(&sl, c as usize) else { continue };
        let (d, m) = (sl[si].ds, &sl[si].map);
        let (lo, hi) = (d.lower_bound(m.inverse(t0)), d.lower_bound(m.inverse(t1)));
        if d.cols[local].levels.is_empty() {
            continue;
        }
        if lo >= hi {
            out.push(json!({ "channel": c, "count": 0, "min": null, "minT": null, "max": null, "maxT": null, "mean": null, "std": null }));
            continue;
        }
        // exact stats need raw samples at the span's ragged edges: a channel without them is left out
        let Some(s) = stats_of(d, local, lo, hi) else { continue };
        let at = |mm: Option<(f64, usize)>| mm.map(|(_, i)| m.apply(d.time_at(i)).to_string());
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

/// One file's answer to a Gauges frame: the nearest sample's index and time, and each widget's answer (`asks` name the
/// file's own channels, `view` is in the file's own clock).
fn widgets_one(d: &Ds, t: i64, view: (i64, i64), asks: &[Value]) -> Option<(usize, i64, Vec<Value>)> {
    let i = nearest(d, t)?;
    let ts = d.time_at(i);
    let n = d.info.channels.len();
    let num = |v: f64| (!v.is_nan()).then_some(v);
    let (v0, v1) = view;
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
            let run = flag("run").then(|| run_start(d, c, i)).flatten();
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
    Some((i, ts, widgets))
}

/// Every widget of a Gauges pane in one call (see the app's `widgets_frame`): values from held raw samples only,
/// the lowest and highest value from the exact statistics, a run's start while its samples are held, and a trace
/// drawn from the summaries. In a tab of several files each file answers its own widgets through its map; the frame's
/// sample is the first answering file's.
fn widgets_frame(st: &State, a: &Value) -> Result<Reply, String> {
    let sl = slots(st, arg(a, "file")?)?;
    let t = time(&arg::<String>(a, "t")?)?;
    let view: Value = arg(a, "view")?;
    let (v0, v1) = (time(view["t0"].as_str().unwrap_or(""))?, time(view["t1"].as_str().unwrap_or(""))?);
    let asks: Vec<Value> = arg(a, "widgets")?;
    let total: usize = sl.iter().map(|s| s.ds.info.channels.len()).sum();
    let mut out: Vec<Value> = vec![Value::Null; asks.len()];
    let mut head: Option<(usize, i64)> = None;
    for (si, s) in sl.iter().enumerate() {
        let (mut at, mut sub) = (Vec::new(), Vec::new());
        for (k, w) in asks.iter().enumerate() {
            let c = (w["channel"].as_u64().unwrap_or(0) as usize).min(total.saturating_sub(1));
            if route(&sl, c).is_some_and(|(r, _)| r == si) {
                let mut w = w.clone();
                w["channel"] = json!(c - s.first);
                at.push(k);
                sub.push(w);
            }
        }
        if at.is_empty() {
            continue;
        }
        let m = &s.map;
        let Some((i, ts, got)) = widgets_one(s.ds, m.inverse(t), (m.inverse(v0), m.inverse(v1)), &sub) else {
            return Ok(Reply::Json(Value::Null));
        };
        head.get_or_insert((i, m.apply(ts)));
        for (k, mut w) in at.into_iter().zip(got) {
            w["runStart"] =
                w["runStart"].as_i64().map(|t| Value::String(m.apply(t).to_string())).unwrap_or(Value::Null);
            out[k] = w;
        }
    }
    let Some((index, ts)) = head else { return Ok(Reply::Json(Value::Null)) };
    Ok(Reply::Json(json!({ "index": index, "t": ts.to_string(), "widgets": out })))
}

/// One Y channel's points: its number, then the times, x and y values.
type XySeries = (usize, Vec<i64>, Vec<f64>, Vec<f64>);

/// Samples read at a time while an XY plot reduces its pairs.
const XY_SLICE: usize = 1 << 18;

/// Channel `c`'s held samples `a..b`: an absent sample reads as missing, so a pair with one breaks the line (never a
/// point from a sample the export doesn't hold).
fn held(d: &Ds, c: usize, a: usize, b: usize) -> Vec<f64> {
    let raw = &d.cols[c].raw;
    (a..b).map(|i| raw.get(i).unwrap_or(f64::NAN)).collect()
}

/// An XY plot (see the app's `xy_points`): Y channels against an X channel of one file over a span, the pairs reduced
/// to a grid of cells. Samples are paired by index, so the channels share their times.
/// Little-endian: `TLMX`, u32 series, u32 flags (bit 0 sampled, bit 1 has a range), u32 0, f64 x0, x1, y0, y1; then per
/// series u32 channel, u32 n, and n x i64 time, n x f64 x, n x f64 y (NaN pairs break the line).
fn xy_points(st: &State, a: &Value) -> Result<Reply, String> {
    let sl = slots(st, arg(a, "file")?)?;
    let span: Value = arg(a, "span")?;
    let ask: Value = a.get("ask").cloned().ok_or("no ask")?;
    let (t0, t1) = (time(span["t0"].as_str().unwrap_or(""))?, time(span["t1"].as_str().unwrap_or(""))?);
    let total: usize = sl.iter().map(|s| s.ds.info.channels.len()).sum();
    let x = (ask["x"].as_u64().unwrap_or(0) as usize).min(total.saturating_sub(1));
    let (sx, lx) = route(&sl, x).ok_or("no such channel")?;
    let mut ys = Vec::new();
    for y in
        ask["ys"].as_array().into_iter().flatten().filter_map(Value::as_u64).map(|y| y as usize).filter(|&y| y < total)
    {
        let (s, l) = route(&sl, y).ok_or("no such channel")?;
        if s != sx {
            return Err("An XY plot pairs channels of one file".into());
        }
        ys.push((y, l));
    }
    let (d, m) = (sl[sx].ds, &sl[sx].map);
    let rect: Option<[f64; 4]> = ask["rect"]
        .as_array()
        .filter(|r| r.len() == 4)
        .map(|r| std::array::from_fn(|i| r[i].as_f64().unwrap_or(f64::NAN)));
    let grid = (ask["grid"][0].as_u64().unwrap_or(1).max(1) as f64, ask["grid"][1].as_u64().unwrap_or(1).max(1) as f64);
    let max_points = ask["maxPoints"].as_u64().unwrap_or(100_000) as usize;
    let (lo, hi) = (d.lower_bound(m.inverse(t0)), d.lower_bound(m.inverse(t1)));
    let mut series: Vec<XySeries> = ys.iter().map(|&(c, _)| (c, Vec::new(), Vec::new(), Vec::new())).collect();
    let (mut range, mut sampled) = (None::<[f64; 4]>, false);
    if hi > lo {
        // pass 1: the range of the pairs
        let mut r = [f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY];
        let mut s0 = lo;
        while s0 < hi {
            let b = (s0 + XY_SLICE).min(hi);
            let xs = held(d, lx, s0, b);
            for &(_, yl) in &ys {
                for (&xv, &yv) in xs.iter().zip(&held(d, yl, s0, b)) {
                    if xv.is_finite() && yv.is_finite() {
                        r = [r[0].min(xv), r[1].max(xv), r[2].min(yv), r[3].max(yv)];
                    }
                }
            }
            s0 = b;
        }
        if r[0].is_finite() {
            range = Some(r);
            // pass 2: reduce on the grid of the visible area (the data's own range when not zoomed)
            let rc = rect.unwrap_or(r);
            let cell = |xv: f64, yv: f64| {
                let cx = ((xv - rc[0]) / (rc[1] - rc[0]).max(f64::MIN_POSITIVE) * grid.0).floor();
                let cy = ((yv - rc[2]) / (rc[3] - rc[2]).max(f64::MIN_POSITIVE) * grid.1).floor();
                (cx as i64, cy as i64)
            };
            let inside =
                |xv: f64, yv: f64| rect.is_none() || (xv >= rc[0] && xv <= rc[1] && yv >= rc[2] && yv <= rc[3]);
            for (s, &(_, yl)) in series.iter_mut().zip(&ys) {
                let mut last: Option<(i64, i64)> = None;
                let mut was_in = false;
                let mut pending: Option<(i64, f64, f64)> = None;
                let mut s0 = lo;
                while s0 < hi {
                    let b = (s0 + XY_SLICE).min(hi);
                    let (xs, yv, ts) = (held(d, lx, s0, b), held(d, yl, s0, b), d.read_times(s0, b));
                    for k in 0..xs.len() {
                        let (xk, yk) = (xs[k], yv[k]);
                        let t = m.apply(ts[k]);
                        if !(xk.is_finite() && yk.is_finite()) {
                            if s.2.last().is_some_and(|v| !v.is_nan()) {
                                s.1.push(t);
                                s.2.push(f64::NAN);
                                s.3.push(f64::NAN);
                            }
                            (last, was_in, pending) = (None, false, None);
                            continue;
                        }
                        let c = cell(xk, yk);
                        if inside(xk, yk) {
                            if let Some((pt, px, py)) = pending.take() {
                                s.1.push(pt);
                                s.2.push(px);
                                s.3.push(py);
                            }
                            if last != Some(c) {
                                s.1.push(t);
                                s.2.push(xk);
                                s.3.push(yk);
                            }
                            was_in = true;
                        } else {
                            if was_in {
                                s.1.push(t);
                                s.2.push(xk);
                                s.3.push(yk);
                            } else {
                                pending = Some((t, xk, yk));
                            }
                            was_in = false;
                        }
                        last = Some(c);
                    }
                    s0 = b;
                }
                if max_points > 0 && s.2.len() > max_points {
                    let step = s.2.len().div_ceil(max_points);
                    let idx: Vec<usize> = (0..s.2.len())
                        .filter(|&i| i.is_multiple_of(step) || s.2[i].is_nan() || i + 1 == s.2.len())
                        .collect();
                    s.1 = idx.iter().map(|&i| s.1[i]).collect();
                    s.3 = idx.iter().map(|&i| s.3[i]).collect();
                    s.2 = idx.iter().map(|&i| s.2[i]).collect();
                    sampled = true;
                }
            }
        }
    }
    let mut out = Vec::new();
    out.extend_from_slice(b"TLMX");
    out.extend_from_slice(&(series.len() as u32).to_le_bytes());
    out.extend_from_slice(&(u32::from(sampled) | (u32::from(range.is_some()) << 1)).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    for v in range.unwrap_or([f64::NAN; 4]) {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for (c, t, x, y) in &series {
        out.extend_from_slice(&(*c as u32).to_le_bytes());
        out.extend_from_slice(&(x.len() as u32).to_le_bytes());
        t.iter().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
        x.iter().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
        y.iter().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
    }
    Ok(Reply::Bytes(out))
}

/// The sample rate of a file: its fixed rate, else from its median interval.
fn rate_hz(d: &Ds) -> Option<f64> {
    match &d.info.time {
        TimeSource::Rate { hz } if *hz > 0.0 => Some(*hz),
        _ if d.gaps.median_ns > 0 => Some(1e9 / d.gaps.median_ns as f64),
        _ => None,
    }
}

/// The Spectrum view: each channel's frequency content over `t0..t1`, from a window of up to 16384 held samples
/// centred in the span at the file's sample rate. A window that needs a sample the export doesn't hold has no spectrum
/// (nothing is filled in).
fn spectrum(st: &State, a: &Value) -> Result<Reply, String> {
    let sl = slots(st, arg(a, "file")?)?;
    let channels: Vec<u32> = arg(a, "channels")?;
    let (t0, t1) = (time(&arg::<String>(a, "t0")?)?, time(&arg::<String>(a, "t1")?)?);
    let mut out = Vec::new();
    for &c in &channels {
        let Some((si, local)) = route(&sl, c as usize) else { continue };
        let (d, m) = (sl[si].ds, &sl[si].map);
        let (t0, t1) = (m.inverse(t0), m.inverse(t1));
        let (Some(first), Some(last), Some(rate)) = (d.info.t_first, d.info.t_last, rate_hz(d)) else { continue };
        let available = ((t1 - t0) as f64 / 1e9 * rate).floor();
        let Some(n) = tlmbench_expr::fft::points(
            if available.is_finite() && available > 0.0 { available as usize } else { 0 },
            tlmbench_expr::fft::MAX_POINTS,
        ) else {
            continue;
        };
        let start = t0 as f64 + ((t1 - t0) as f64 - n as f64 / rate * 1e9) / 2.0;
        let idx: Vec<Option<usize>> = (0..n)
            .map(|i| {
                let t = (start + i as f64 / rate * 1e9).round() as i64;
                (first..=last).contains(&t).then(|| d.lower_bound(t.saturating_add(1)).clamp(1, d.len) - 1)
            })
            .collect();
        let raw = &d.cols[local].raw;
        // every sample the window reads must be held
        let Some(x) = idx
            .iter()
            .map(|i| match i {
                Some(i) => raw.get(*i),
                None => Some(f64::NAN),
            })
            .collect::<Option<Vec<f64>>>()
        else {
            continue;
        };
        let Some(s) = tlmbench_expr::spectrum(&x, rate) else { continue };
        out.push(json!({ "channel": c, "df": s.df, "nyquist": s.nyquist, "peak_hz": s.peak_hz, "peak_amp": s.peak_amp, "n": s.n, "mag": s.mag.iter().map(|&v| v as f32).collect::<Vec<_>>() }));
    }
    Ok(Reply::Json(Value::Array(out)))
}

fn rows(st: &State, a: &Value) -> Result<Reply, String> {
    let sl = slots(st, arg(a, "file")?)?;
    let channels: Vec<u32> = arg(a, "channels")?;
    let span: Value = arg(a, "span")?;
    let offset: u64 = arg(a, "offset")?;
    let limit: u32 = arg(a, "limit")?;
    let (t0, t1) = (time(span["t0"].as_str().unwrap_or(""))?, time(span["t1"].as_str().unwrap_or(""))?);
    // the rows are the primary's samples; another file's columns hold its last sample at or before each row's time
    let (d, pm) = (sl[0].ds, &sl[0].map);
    let chans: Vec<(usize, usize)> = channels.iter().filter_map(|&c| route(&sl, c as usize)).collect();
    let lo = d.lower_bound(pm.inverse(t0));
    let hi = d.lower_bound(pm.inverse(t1)).max(lo);
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
    let times: Vec<i64> = d.read_times(start, end).into_iter().map(|t| pm.apply(t)).collect();
    for t in &times {
        out.extend_from_slice(&t.to_le_bytes());
    }
    for &(si, local) in &chans {
        let s = &sl[si];
        for (j, &t) in times.iter().enumerate() {
            // a sample the export doesn't hold is a blank cell
            let v = if si == 0 {
                s.ds.cols[local].raw.get(start + j)
            } else {
                s.ds.lower_bound(s.map.inverse(t).saturating_add(1))
                    .checked_sub(1)
                    .and_then(|i| s.ds.cols[local].raw.get(i))
            };
            out.extend_from_slice(&v.unwrap_or(f64::NAN).to_le_bytes());
        }
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

    /// Two files on one timeline: the second is logged 10 s after the first starts.
    fn two_files() -> (State, u32) {
        let mut st = State::default();
        st.files.push(Some(trimmed(0, &[(0, N)])));
        st.files.push(Some(trimmed(2, &[(20_000, 30_000)])));
        let m = json!([{ "file": 0, "knots": [] }, { "file": 1, "knots": [["0", "10000000000"]] }]);
        let g = crate::group::set(&mut st, None, &m).unwrap();
        let id = g["id"].as_u64().unwrap() as u32;
        (st, id)
    }

    #[test]
    fn a_group_reads_each_file_through_its_map() {
        let (st, g) = two_files();
        // channel 0 is file 1's, channel 1 is file 2's (one channel each)
        let at = |t_s: i64| {
            let a = json!({ "file": g, "channels": [0, 1], "t": (t_s * 1_000_000_000).to_string() });
            match values_at(&st, &a).unwrap() {
                Reply::Json(Value::Array(v)) => v,
                _ => unreachable!(),
            }
        };
        // shared 55 s is 55 s in file 1 and 45 s in file 2, whose raw samples are held from 20 s to 30 s only
        let v = at(55);
        assert_eq!(v.len(), 1, "file 2 holds nothing at its 45 s: {v:?}");
        assert_eq!(v[0]["channel"], 0);
        // shared 32 s is 22 s in file 2: held, and reported at 32 s
        let v = at(32);
        assert_eq!(v.len(), 2);
        assert_eq!(v[1]["value"].as_f64(), Some(value(22_000)));
        assert_eq!(v[1]["t"], "32000000000");
        assert_eq!(v[0]["value"].as_f64(), Some(value(32_000)));
        // stats of a span in file 2's window, through its map: shared 32..36 s is file 2's 22..26 s
        let a = json!({ "file": g, "channels": [1], "span": { "t0": "32000000000", "t1": "36000000000" } });
        let Reply::Json(Value::Array(s)) = stats(&st, &a).unwrap() else { unreachable!() };
        let (mn, mx, _) = brute(22_000, 26_000);
        assert_eq!((s[0]["min"].as_f64(), s[0]["max"].as_f64()), (Some(mn), Some(mx)));
        // the time of the extreme is in the shared clock
        let (_, imax) =
            (22_000..26_000).fold((f64::MIN, 0), |(m, k), i| if value(i) > m { (value(i), i) } else { (m, k) });
        assert_eq!(s[0]["maxT"], ((imax as i64 + 10_000) * 1_000_000).to_string());
    }

    #[test]
    fn a_group_view_is_in_the_shared_clock_and_labels_what_it_lacks() {
        let (st, g) = two_files();
        let view_of = |t0_s: i64, t1_s: i64| {
            let a = json!({ "file": g, "generation": 1, "entries": [{ "id": 7, "channels": [1],
                "t0": (t0_s * 1_000_000_000).to_string(), "t1": (t1_s * 1_000_000_000).to_string(), "widthPx": 300 }] });
            let Reply::Bytes(b) = view(&st, &a).unwrap() else { unreachable!() };
            tlmbench_core::wire::decode_batch(&b).unwrap().entries.remove(0).1
        };
        // inside file 2's window (shared 32..35 s): exact, and the times are shared times
        let r = view_of(32, 35);
        assert_eq!(r.traces.len(), 1);
        assert!(!r.traces[0].coarse && !r.traces[0].x.is_empty());
        // outside it (shared 50..90 s): coarse, from the summaries
        assert!(view_of(50, 90).traces[0].coarse);
        // a channel number past every file is ignored
        let a = json!({ "file": g, "generation": 1, "entries": [{ "id": 1, "channels": [9], "t0": "0", "t1": "1000000000", "widthPx": 10 }] });
        let Reply::Bytes(b) = view(&st, &a).unwrap() else { unreachable!() };
        assert!(tlmbench_core::wire::decode_batch(&b).unwrap().entries[0].1.traces.is_empty());
    }

    #[test]
    fn a_groups_table_holds_the_other_files_last_sample_and_blanks_what_is_absent() {
        let (st, g) = two_files();
        // rows of the primary from shared 30 s; file 2's column (channel 1) holds its sample at or before each time
        let a = json!({ "file": g, "channels": [0, 1], "span": { "t0": "30000000000", "t1": "30003000000" }, "offset": 0, "limit": 10 });
        let Reply::Bytes(b) = rows(&st, &a).unwrap() else { unreachable!() };
        let f = |o: usize| f64::from_le_bytes(b[o..o + 8].try_into().unwrap());
        let k = 3;
        assert_eq!(u32::from_le_bytes(b[24..28].try_into().unwrap()) as usize, k);
        let col0 = 32; // after the 32-byte header: times (k), then channel 0, then channel 1
        let c0 = col0 + 8 * k;
        let c1 = c0 + 8 * k;
        assert_eq!(f(c0), value(30_000));
        // file 2's own time is 20 s there: its first held sample, 20_000
        assert_eq!(f(c1), value(20_000));
        assert_eq!(f(c1 + 16), value(20_002));
        // before file 2's window: blank
        let a = json!({ "file": g, "channels": [1], "span": { "t0": "20000000000", "t1": "20001000000" }, "offset": 0, "limit": 10 });
        let Reply::Bytes(b) = rows(&st, &a).unwrap() else { unreachable!() };
        assert!(f64::from_le_bytes(b[32 + 8..32 + 16].try_into().unwrap()).is_nan());
    }

    #[test]
    fn a_groups_gauges_answer_each_file_in_the_shared_clock() {
        let (st, g) = two_files();
        let a = json!({ "file": g, "t": "32000000000", "view": { "t0": "30000000000", "t1": "40000000000" },
            "widgets": [{ "channel": 0, "run": true }, { "channel": 1, "minmax": true, "run": true }] });
        let Reply::Json(f) = widgets_frame(&st, &a).unwrap() else { unreachable!() };
        assert_eq!(f["t"], "32000000000");
        assert_eq!(f["widgets"][0]["value"].as_f64(), Some(value(32_000)));
        assert_eq!(f["widgets"][1]["value"].as_f64(), Some(value(22_000)));
        // file 2's run starts are in the shared clock (its own 10 s is shared 20 s at the earliest)
        let run = f["widgets"][1]["runStart"].as_str().map(|s| s.parse::<i64>().unwrap());
        assert!(run.is_some_and(|r| r >= 30_000_000_000), "{run:?}");
    }

    #[test]
    fn closing_a_tab_leaves_its_files_open() {
        let (mut st, g) = two_files();
        dispatch(&mut st, "close", &json!({ "file": g }), &[]).unwrap();
        assert!(st.groups.is_empty() && st.files.iter().all(Option::is_some));
        dispatch(&mut st, "close", &json!({ "file": 1 }), &[]).unwrap();
        assert!(st.files[1].is_none() && st.files[0].is_some());
    }

    #[test]
    fn xy_pairs_only_held_samples_and_spectrum_needs_its_whole_window() {
        // x and y are the same channel here: points lie on the diagonal, and only inside the held window
        let d = trimmed(0, &[(20_000, 30_000)]);
        let mut st = State::default();
        st.files.push(Some(d));
        let xy = |t0_s: i64, t1_s: i64| {
            let a = json!({ "file": 0, "span": { "t0": (t0_s * 1_000_000_000).to_string(), "t1": (t1_s * 1_000_000_000).to_string() },
                "ask": { "x": 0, "ys": [0], "grid": [200, 200], "maxPoints": 100000 } });
            let Reply::Bytes(b) = xy_points(&st, &a).unwrap() else { unreachable!() };
            b
        };
        let counts = |b: &[u8]| {
            (
                u32::from_le_bytes(b[4..8].try_into().unwrap()),
                u32::from_le_bytes(b[8..12].try_into().unwrap()) >> 1 & 1,
                u32::from_le_bytes(b[48 + 4..48 + 8].try_into().unwrap()),
            )
        };
        let inside = xy(20, 30);
        let (series, has_range, n) = counts(&inside);
        assert_eq!((series, has_range), (1, 1));
        assert!(n > 10, "a diagonal through the held samples: {n} points");
        // outside the window nothing is held: no range, no points
        let outside = xy(50, 60);
        assert_eq!((counts(&outside).1, counts(&outside).2), (0, 0));
        // a span that is half inside draws only the half the export holds
        let half = xy(25, 40);
        assert!(counts(&half).2 > 0 && counts(&half).2 < n + 1);
        // the spectrum of held samples finds the sine (0.013 rad/sample at 1 kHz is 2.07 Hz); a window with absent samples has none
        let whole = trimmed(0, &[(0, N)]);
        let mut st2 = State::default();
        st2.files.push(Some(whole));
        let sp = |st: &State, t0: i64, t1: i64| {
            let a = json!({ "file": 0, "channels": [0], "t0": (t0 * 1_000_000_000).to_string(), "t1": (t1 * 1_000_000_000).to_string() });
            let Reply::Json(Value::Array(v)) = spectrum(st, &a).unwrap() else { unreachable!() };
            v
        };
        let v = sp(&st2, 0, 90);
        assert_eq!(v.len(), 1);
        let peak = v[0]["peak_hz"].as_f64().unwrap();
        assert!((1.5..3.0).contains(&peak), "{peak}");
        assert!(sp(&st, 0, 90).is_empty(), "the window reads samples the export does not hold");
        assert_eq!(sp(&st, 20, 30).len(), 1, "a window inside the held samples has a spectrum");
    }
}
