//! Browser-only mode: a file opened in the page (CSV, or a tlmbench reference `.tlm`). The bytes come from a `File`, are read by the decoder's own CSV reader
//! (schema, units, time column, missing values: `crates/decode/src/csv.rs`, included as it is) and become a [`Ds`] the
//! viewer's queries answer. The columns can be dumped (the page keeps them in its private storage) and loaded again.

use serde_json::{Value, json};
use tlmbench_core::RateTimes;
use tlmbench_core::recording::{ChannelInfo, TimeSource};

use crate::backwards::{Backwards, clamp_backwards};
use crate::csv::{self, Kind, TimeColumn};
use crate::data::{Ds, Tm};
use crate::descriptor::{DType, Descriptor, Endian, TimeSpec, counter_range};
use crate::strided::{CounterTimes, FieldTimes, decode, wrap_positions};
use crate::{Reply, ops};
use tlmbench_core::Times;

/// Reads whatever the bytes are into file `k`: a tlmbench reference file by its magic, else a CSV.
pub fn import_file(st: &mut crate::data::State, k: usize, path: &str, bytes: &[u8]) -> Result<Reply, String> {
    if bytes.starts_with(crate::reference::MAGIC) {
        import_tlm(st, k, path, bytes)
    } else {
        import_csv(st, k, path, bytes)
    }
}

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
        col.raw = crate::data::Raw::whole(values);
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

/// One field of every record as `f64` (scaled): `records` are whole records of `stride` bytes.
fn field_values(
    records: &[u8],
    stride: usize,
    offset: usize,
    dtype: DType,
    big: bool,
    f: &crate::descriptor::Field,
) -> Vec<f64> {
    macro_rules! col {
        ($t:ty) => {
            records
                .chunks_exact(stride)
                .map(|r| decode::<$t>(big, f.bits, &r[offset..]) as f64 * f.scale + f.value_offset)
                .collect()
        };
    }
    match dtype {
        DType::I8 => col!(i8),
        DType::I16 => col!(i16),
        DType::I32 => col!(i32),
        DType::I64 => col!(i64),
        DType::U8 => col!(u8),
        DType::U16 => col!(u16),
        DType::U32 => col!(u32),
        DType::U64 => col!(u64),
        DType::F32 => col!(f32),
        DType::F64 => col!(f64),
    }
}

/// Reads a tlmbench reference file (`.tlm`: fixed-width records behind a header that holds the descriptor) into file
/// `k`. The decoder's own header and descriptor code, read from bytes in memory (no mapped file, no threads).
pub fn import_tlm(st: &mut crate::data::State, k: usize, path: &str, bytes: &[u8]) -> Result<Reply, String> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let bad = |msg: String| format!("{name}: {msg}");
    let (header_len, desc): (usize, Descriptor) =
        crate::reference::read_header(bytes).map_err(|e| bad(e.to_string()))?;
    desc.validate().map_err(|e| bad(e.to_string()))?;
    if desc.packets.is_some() {
        return Err(bad("a descriptor with [packets] lays out a packet file, not fixed-width records".into()));
    }
    let stride = desc.record_size as usize;
    let n = (bytes.len() - header_len) / stride;
    let records = &bytes[header_len..header_len + n * stride];
    let big = desc.endian == Endian::Big;
    let times_of = |field: &str, scale_ns: i64, t0_ns: i64| -> FieldTimes<'_> {
        let tf = desc.fields.iter().find(|x| x.name == field).expect("validated");
        FieldTimes {
            records,
            offset: tf.offset as usize,
            stride,
            big_endian: big,
            dtype: tf.dtype,
            bits: tf.bits,
            scale_ns,
            t0_ns,
            plateaus: &[],
        }
    };
    let mut found = Backwards::default();
    let (time, tm) = match &desc.time {
        TimeSpec::Rate { rate_hz, t0_ns } => {
            (TimeSource::Rate { hz: *rate_hz }, Tm::Rate(RateTimes::from_rate(*t0_ns, *rate_hz, n)))
        }
        TimeSpec::Index => (TimeSource::Index, Tm::Rate(RateTimes::from_rate(0, 1.0, n))),
        TimeSpec::Field { field, unit, t0_ns } => {
            let t = times_of(field, unit.ns(), *t0_ns);
            let mut v: Vec<i64> = (0..n).map(|i| t.at(i)).collect();
            // time that goes backwards is flagged and held, never reordered (the decoder's rule for every format)
            clamp_backwards(&mut v, &mut None, 1, &mut found);
            (TimeSource::Column { name: field.clone() }, Tm::Col(v))
        }
        TimeSpec::Counter { field, clock_hz, wraps_at, t0_ns } => {
            let tf = desc.fields.iter().find(|x| &x.name == field).expect("validated");
            let range = counter_range(tf, *wraps_at).expect("validated");
            let raw = times_of(field, 1, 0);
            let wraps = wrap_positions(n, range, |i| raw.raw(i));
            let t = CounterTimes { raw, wraps: &wraps, range, clock_hz: *clock_hz, t0_ns: *t0_ns, plateaus: &[] };
            let mut v: Vec<i64> = (0..n).map(|i| t.at(i)).collect();
            clamp_backwards(&mut v, &mut None, 1, &mut found);
            (TimeSource::Column { name: field.clone() }, Tm::Col(v))
        }
    };
    let fields: Vec<_> = desc.channels().map(|(_, f)| f).collect();
    let channels = fields
        .iter()
        .map(|f| ChannelInfo { name: f.name.clone(), unit: f.unit.clone(), dtype: f.dtype.name(), category: None })
        .collect();
    let mut d = Ds::new(path, "tlmbench reference", None, n, time, tm, channels);
    d.info.notes = found.note_of("record", "record").into_iter().collect();
    for (col, f) in d.cols.iter_mut().zip(&fields) {
        col.raw = crate::data::Raw::whole(field_values(records, stride, f.offset as usize, f.dtype, big, f));
    }
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
        col.raw.slice(d.len).into_iter().flatten().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
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
        col.raw = crate::data::Raw::whole(
            take(n)?.chunks_exact(8).map(|b| f64::from_le_bytes(b.try_into().unwrap())).collect(),
        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::State;

    /// A `.tlm` of 8-byte records: a u32 time, an i16 and a u8 (then a pad byte).
    fn tlm(records: &[(u32, i16, u8)], toml: &str) -> Vec<u8> {
        let d = Descriptor::parse(toml).unwrap();
        let mut out = Vec::new();
        crate::reference::write_header(&mut out, &d).unwrap();
        for (t, v, f) in records {
            out.extend_from_slice(&t.to_le_bytes());
            out.extend_from_slice(&v.to_le_bytes());
            out.push(*f);
            out.push(0);
        }
        out
    }

    const FIELD_TIME: &str = r#"
schema = 1
name = "t"
record_size = 8
[time]
source = "field"
field = "t"
unit = "us"
[[fields]]
name = "t"
type = "u32"
offset = 0
[[fields]]
name = "v"
type = "i16"
offset = 4
unit = "V"
scale = 0.001
value_offset = 1.0
[[fields]]
name = "flag"
type = "u8"
offset = 6
"#;

    #[test]
    fn a_tlm_reads_its_times_scaled_values_and_units() {
        let bytes = tlm(&[(0, 1000, 1), (500, -2000, 0), (1000, 0, 1)], FIELD_TIME);
        let mut st = State::default();
        import_file(&mut st, 0, "local://x.tlm", &bytes).unwrap();
        let d = st.files[0].as_ref().unwrap();
        assert_eq!(d.len, 3);
        assert_eq!(d.info.format, "tlmbench reference");
        assert_eq!(d.info.channels.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["v", "flag"]);
        assert_eq!(d.info.channels[0].unit.as_deref(), Some("V"));
        let Tm::Col(t) = &d.tm else { panic!("a time field gives a time column") };
        assert_eq!(t, &[0, 500_000, 1_000_000]);
        assert_eq!(d.cols[0].raw.slice(3).unwrap(), [2.0, -1.0, 1.0]);
        assert_eq!(d.cols[1].raw.slice(3).unwrap(), [1.0, 0.0, 1.0]);
    }

    #[test]
    fn a_tlm_whose_time_goes_backwards_is_flagged_and_held() {
        let bytes = tlm(&[(10, 0, 0), (5, 0, 0), (20, 0, 0)], FIELD_TIME);
        let mut st = State::default();
        import_file(&mut st, 0, "local://x.tlm", &bytes).unwrap();
        let d = st.files[0].as_ref().unwrap();
        let Tm::Col(t) = &d.tm else { panic!("a time field gives a time column") };
        assert_eq!(t, &[10_000, 10_000, 20_000]);
        let note = d.info.notes.join(" ");
        assert!(note.contains("Time goes backwards in 1 record (first at record 2"), "{note}");
    }

    #[test]
    fn a_counter_that_wraps_keeps_counting_and_a_rate_needs_no_field() {
        let counter = FIELD_TIME
            .replace(r#"source = "field""#, r#"source = "counter""#)
            .replace(r#"unit = "us""#, "clock_hz = 1000000.0\nwraps_at = 65536");
        let bytes = tlm(&[(65000, 0, 0), (200, 0, 0), (700, 0, 0)], &counter);
        let mut st = State::default();
        import_file(&mut st, 0, "x.tlm", &bytes).unwrap();
        let Tm::Col(t) = &st.files[0].as_ref().unwrap().tm else { panic!() };
        assert_eq!(t, &[65_000_000, 65_736_000, 66_236_000], "{t:?}");
        let rate = FIELD_TIME
            .replace(r#"source = "field""#, "source = \"rate\"\nrate_hz = 100.0")
            .replace("field = \"t\"\nunit = \"us\"\n", "");
        let mut st = State::default();
        import_file(&mut st, 0, "x.tlm", &tlm(&[(1, 2, 3), (4, 5, 6)], &rate)).unwrap();
        let d = st.files[0].as_ref().unwrap();
        assert!(matches!(d.info.time, TimeSource::Rate { hz } if hz == 100.0));
        assert_eq!(d.info.channels.len(), 3, "with no time field every field is a channel");
    }

    #[test]
    fn what_is_not_a_tlm_is_read_as_csv_and_a_cut_header_is_an_error() {
        let mut st = State::default();
        import_file(&mut st, 0, "a.csv", b"t,x\n0,1\n1,2\n").unwrap();
        assert_eq!(st.files[0].as_ref().unwrap().info.format, "CSV");
        let cut = &tlm(&[(0, 0, 0)], FIELD_TIME)[..20];
        assert!(import_file(&mut st, 1, "b.tlm", cut).is_err());
    }
}
