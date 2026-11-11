//! The commands: loading an export's data into the core, and the queries the viewer asks of it.

use serde::Deserialize;
use serde_json::{Value, json};
use tlmbench_core::chunks::Held;
use tlmbench_core::gaps::{DEFAULT_FACTOR, MIN_FACTOR};
use tlmbench_core::pyramid::{MIN_BUCKET_SAMPLES, Summary};
use tlmbench_core::recording::{ChannelInfo, TimeSource};
use tlmbench_core::wire;
use tlmbench_core::{RateTimes, Recording, Trace, TraceKind, Viewport};

use crate::Reply;
use crate::data::{Ds, State, Tm, dtype};

const RAW_MAX: u32 = 10_000_000;
const MAX_ROWS: usize = 10_000;

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
            col.raw.get_or_insert_with(Vec::new).extend(f64s(bin));
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
        "view" => view(st, a),
        "values_at" => values_at(st, a),
        "stats" => stats(st, a),
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
            let has_raw = d.cols[c].raw.as_ref().is_some_and(|r| r.len() == d.len);
            let (trace, coarse) = match &pyr {
                // no raw samples: the pyramid answers at every zoom, labelled coarse past its finest level
                Some(_) if !has_raw && !coarse_pays => (summary_trace(d, c, vp), true),
                Some(p) if !has_raw => (d.view_coarse(c, vp, p, &breaks, &held), true),
                Some(p) if coarse_first && coarse_pays => (d.view_coarse(c, vp, p, &breaks, &held), true),
                p if has_raw => (d.view(c, vp, p.as_ref(), &breaks, &held), false),
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

/// A channel with no raw samples in a view of fewer samples per pixel than its finest summaries cover: each summary of 256
/// samples contributes its lowest and highest value at their own times, in order (exact aggregates, never a sample that
/// isn't there); a time jump over four median intervals breaks the line. Labelled coarse by the caller.
fn summary_trace(d: &Ds, c: usize, vp: Viewport) -> Trace {
    let empty = Trace { kind: TraceKind::M4, bucket_ns: vp.bucket_ns(), t: vec![], y: vec![] };
    let Some(l4) = d.cols[c].levels.first().filter(|_| d.cols[c].first_level == tlmbench_core::pyramid::MIN_LEVEL)
    else {
        return empty;
    };
    let w = tlmbench_core::pyramid::span(tlmbench_core::pyramid::MIN_LEVEL);
    let (lo, hi) = (d.lower_bound(vp.t0).saturating_sub(1), (d.lower_bound(vp.t1) + 1).min(d.len));
    let (a, b) = (lo / w, hi.div_ceil(w).min(l4.len()));
    let median = d.gaps.median_ns;
    let (mut t, mut y) = (Vec::new(), Vec::new());
    let mut prev_last: Option<i64> = None;
    for s in l4.get(a..b).unwrap_or_default() {
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

fn raw_of(d: &Ds, c: usize) -> Option<&[f64]> {
    d.cols[c].raw.as_deref().filter(|r| r.len() == d.len)
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
            let v = raw_of(d, c as usize)?[i];
            Some(json!({ "channel": c, "index": i, "t": ts, "value": (!v.is_nan()).then_some(v) }))
        })
        .collect();
    Ok(Reply::Json(Value::Array(out)))
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
        // exact stats need raw samples at the span's ragged edges
        let (Some(_), Some(py)) = (raw_of(d, c), d.pyramid(c)) else { continue };
        if lo >= hi {
            out.push(json!({ "channel": c, "count": 0, "min": null, "minT": null, "max": null, "maxT": null, "mean": null, "std": null }));
            continue;
        }
        let s = d.stats(c, &py, lo, hi, &Held::default());
        let at = |m: Option<(f64, usize)>| m.map(|(_, i)| d.time_at(i).to_string());
        out.push(json!({ "channel": c, "count": s.count, "min": s.min.map(|m| m.0), "minT": at(s.min), "max": s.max.map(|m| m.0), "maxT": at(s.max), "mean": num(s.mean), "std": num(s.std) }));
    }
    Ok(Reply::Json(Value::Array(out)))
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
        match raw_of(d, c) {
            Some(r) => r[start..end].iter().for_each(|v| out.extend_from_slice(&v.to_le_bytes())),
            None => (0..k).for_each(|_| out.extend_from_slice(&f64::NAN.to_le_bytes())),
        }
    }
    Ok(Reply::Bytes(out))
}
