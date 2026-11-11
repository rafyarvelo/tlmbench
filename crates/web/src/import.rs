//! Browser-only mode: a CSV opened in the page. The bytes come from a `File`, are read by the decoder's own CSV reader
//! (schema, units, time column, missing values: `crates/decode/src/csv.rs`, included as it is) and become a [`Ds`] the
//! viewer's queries answer. The columns can be dumped (the page keeps them in its private storage) and loaded again.

use serde_json::{Value, json};
use tlmbench_core::RateTimes;
use tlmbench_core::recording::{ChannelInfo, TimeSource};

use crate::backwards::{Backwards, clamp_backwards};
use crate::csv::{self, Kind, TimeColumn};
use crate::data::{Ds, Tm};
use crate::{Reply, ops};

/// Reads a CSV into file `k`, naming it `path`: the same `Opened` reply as an export's file (see `ops::opened`).
pub fn import_csv(st: &mut crate::data::State, k: usize, path: &str, bytes: &[u8]) -> Result<Reply, String> {
    let (schema, rows) = csv::parse_all(bytes).map_err(|e| e.to_string())?;
    let numeric: Vec<_> = schema.numeric_columns().map(|(_, c)| c.clone()).collect();
    let n = rows.rows;
    let mut t = rows.t;
    let (time, tm, name) = match schema.time {
        TimeColumn::Index => match schema.index_rate {
            Some(hz) => (TimeSource::Rate { hz }, Tm::Rate(RateTimes::from_rate(0, hz, n)), None),
            None => (TimeSource::Index, Tm::Rate(RateTimes::from_rate(0, 1.0, n)), None),
        },
        TimeColumn::Numeric { index, .. } | TimeColumn::Iso { index } => {
            let mut prev = None;
            let mut backwards = Backwards::default();
            clamp_backwards(&mut t, &mut prev, 1, &mut backwards);
            (TimeSource::Column { name: schema.columns[index].name.clone() }, Tm::Col(t), backwards.note())
        }
    };
    let channels = numeric
        .iter()
        .map(|c| ChannelInfo { name: c.name.clone(), unit: c.unit.clone(), dtype: "f64", category: c.category.clone() })
        .collect();
    let mut d = Ds::new(path, "CSV", None, n, time, tm, channels);
    for (col, values) in d.cols.iter_mut().zip(rows.columns) {
        col.raw = Some(values);
    }
    d.info.skipped =
        schema.columns.iter().filter(|c| matches!(c.kind, Kind::Text | Kind::Skip)).map(|c| c.name.clone()).collect();
    d.info.notes = name.into_iter().collect();
    d.refresh();
    for c in 0..d.cols.len() {
        d.build_pyramid(c);
    }
    let reply = ops::opened(&d, k as u32);
    if st.files.len() <= k {
        st.files.resize_with(k + 1, || None);
    }
    st.files[k] = Some(d);
    Ok(Reply::Json(reply))
}

/// File `k`'s samples as the page keeps them: the times (i64 LE; none for a fixed rate) then each channel's values (f64 LE).
/// The JSON says how to read them back.
pub fn dump(st: &crate::data::State, k: usize) -> Result<(Value, Vec<u8>), String> {
    let d = st.files.get(k).and_then(|f| f.as_ref()).ok_or("that file is not open")?;
    let mut out = Vec::new();
    let time = match &d.tm {
        Tm::Rate(r) => {
            json!({ "kind": "rate", "t0": r.t0.to_string(), "hz": match &d.info.time { TimeSource::Rate { hz } => *hz, _ => 1.0 } })
        }
        Tm::Col(v) => {
            v.iter().for_each(|t| out.extend_from_slice(&t.to_le_bytes()));
            json!({ "kind": "column", "name": match &d.info.time { TimeSource::Column { name } => name.clone(), _ => "time".into() } })
        }
    };
    let mut chans = Vec::new();
    for (c, col) in d.info.channels.iter().zip(&d.cols) {
        col.raw.iter().flatten().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
        chans.push(json!({ "name": c.name, "unit": c.unit, "category": c.category }));
    }
    let meta = json!({
        "samples": d.len, "time": time, "channels": chans, "format": d.info.format, "skipped": d.info.skipped, "notes": d.info.notes,
        "index": matches!(d.info.time, TimeSource::Index),
    });
    Ok((meta, out))
}

/// The inverse of [`dump`].
pub fn load(st: &mut crate::data::State, k: usize, path: &str, meta: &Value, bin: &[u8]) -> Result<Reply, String> {
    let n = meta["samples"].as_u64().ok_or("no samples")? as usize;
    let mut at = 0usize;
    let mut take = |len: usize| -> Result<&[u8], String> {
        let b = bin.get(at..at + len * 8).ok_or("the stored columns are short")?;
        at += len * 8;
        Ok(b)
    };
    let (time, tm) = match meta["time"]["kind"].as_str() {
        Some("rate") => {
            let t0: i64 = meta["time"]["t0"].as_str().unwrap_or("0").parse().map_err(|_| "bad t0")?;
            let hz = meta["time"]["hz"].as_f64().unwrap_or(1.0);
            let source =
                if meta["index"].as_bool() == Some(true) { TimeSource::Index } else { TimeSource::Rate { hz } };
            (source, Tm::Rate(RateTimes::from_rate(t0, hz, n)))
        }
        _ => {
            let v: Vec<i64> = take(n)?.chunks_exact(8).map(|b| i64::from_le_bytes(b.try_into().unwrap())).collect();
            (TimeSource::Column { name: meta["time"]["name"].as_str().unwrap_or("time").into() }, Tm::Col(v))
        }
    };
    let channels = meta["channels"]
        .as_array()
        .ok_or("no channels")?
        .iter()
        .map(|c| ChannelInfo {
            name: c["name"].as_str().unwrap_or("").into(),
            unit: c["unit"].as_str().map(str::to_owned),
            dtype: "f64",
            category: c["category"].as_str().map(str::to_owned),
        })
        .collect();
    let mut d = Ds::new(path, meta["format"].as_str().unwrap_or("CSV"), None, n, time, tm, channels);
    for col in d.cols.iter_mut() {
        col.raw = Some(take(n)?.chunks_exact(8).map(|b| f64::from_le_bytes(b.try_into().unwrap())).collect());
    }
    d.info.skipped = meta["skipped"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
        .unwrap_or_default();
    d.info.notes = meta["notes"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
        .unwrap_or_default();
    d.refresh();
    for c in 0..d.cols.len() {
        d.build_pyramid(c);
    }
    let reply = ops::opened(&d, k as u32);
    if st.files.len() <= k {
        st.files.resize_with(k + 1, || None);
    }
    st.files[k] = Some(d);
    Ok(Reply::Json(reply))
}
