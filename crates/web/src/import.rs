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
    // the channels in the file's column order: numbers, and text columns with at most MAX_LABELS texts (D-ingest-12)
    let _ = &numeric;
    let (mut channels, mut values, mut skipped, mut notes) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut nums, mut texts) = (rows.columns.into_iter(), rows.texts.into_iter());
    for c in &schema.columns {
        match c.kind {
            Kind::Number => {
                channels.push(ChannelInfo {
                    name: c.name.clone(),
                    unit: c.unit.clone(),
                    dtype: "f64",
                    category: c.category.clone(),
                    labels: Vec::new(),
                });
                values.push(nums.next().unwrap_or_default());
            }
            Kind::Text => {
                let t = texts.next().unwrap_or_default();
                if t.over {
                    skipped.push(c.name.clone());
                    notes.push(format!(
                        "Column \"{}\" left out: more than {} different texts",
                        c.name,
                        csv::MAX_LABELS
                    ));
                    continue;
                }
                channels.push(ChannelInfo {
                    name: c.name.clone(),
                    unit: c.unit.clone(),
                    dtype: "f64",
                    category: c.category.clone(),
                    labels: t.dict,
                });
                values.push(t.codes.iter().map(|&k| if k == u16::MAX { f64::NAN } else { k as f64 }).collect());
            }
            Kind::Skip => skipped.push(c.name.clone()),
            Kind::Time => {}
        }
    }
    let mut d = Ds::new(path, "CSV", None, n, time, tm, channels);
    for (col, v) in d.cols.iter_mut().zip(values) {
        col.raw = crate::data::Raw::whole(v);
    }
    d.info.skipped = skipped;
    d.info.notes = name.into_iter().chain(notes).collect();
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
    import_records(st, k, path, "tlmbench reference", None, &desc, records, stride, Vec::new(), Vec::new())
}

/// A descriptor as the page keeps it: TOML text, or the descriptor form's JSON (as the app's commands take it).
pub fn descriptor_of(v: &Value) -> Result<Descriptor, String> {
    match v {
        Value::String(t) => Descriptor::parse(t).map_err(|e| e.to_string()),
        other => serde_json::from_value(other.clone()).map_err(|e| e.to_string()),
    }
}

/// A folder's `.tlmbench/` as the page read it from its directory handle: each file's text by its path inside it.
pub struct Listing(pub std::collections::BTreeMap<String, String>);

impl Listing {
    /// The names directly in `rel` (`dirs`: the folders, else the files ending in `ext`), in name order.
    fn names(&self, rel: &str, dirs: bool, ext: &str) -> Vec<String> {
        let prefix = if rel.is_empty() { String::new() } else { format!("{rel}/") };
        let mut out: Vec<String> = self
            .0
            .keys()
            .filter_map(|k| k.strip_prefix(&prefix))
            .filter_map(|rest| match rest.split_once('/') {
                Some((d, _)) if dirs => Some(d.to_owned()),
                None if !dirs && rest.ends_with(ext) => Some(rest.to_owned()),
                _ => None,
            })
            .collect();
        out.dedup();
        out
    }
}

impl crate::config_core::Tree for Listing {
    fn text(&self, rel: &str) -> Option<String> {
        self.0.get(rel).cloned()
    }
    fn dirs(&self, rel: &str) -> Vec<String> {
        self.names(rel, true, "")
    }
    fn files(&self, rel: &str, ext: &str) -> Vec<String> {
        self.names(rel, false, ext)
    }
}

/// What a folder's `.tlmbench/` holds for the page: its categories, widgets and icons (as the app's `config_use` gives
/// them), and its `decoders/` as descriptors (the form's JSON, by id; one that doesn't parse is left out, as the app does).
pub fn config_of(files: &Value) -> Result<Value, String> {
    let map: std::collections::BTreeMap<String, String> =
        serde_json::from_value(files.clone()).map_err(|e| e.to_string())?;
    let l = Listing(map);
    let from = crate::config_core::from_tree(&l);
    let decoders: Vec<Value> = crate::config_core::Tree::files(&l, "decoders", ".toml")
        .into_iter()
        .filter_map(|n| {
            let d = Descriptor::parse(l.0.get(&format!("decoders/{n}"))?).ok()?;
            Some(json!({ "id": n.trim_end_matches(".toml"), "descriptor": d }))
        })
        .collect();
    Ok(json!({ "from": from, "decoders": decoders }))
}

/// The packet types of a descriptor kept in the page, each a recording of its own: their names, or one empty name for a
/// descriptor without `[[packet_types]]`. The page opens one file per type (`flight.bin · gps`), as the app does.
pub fn described_parts(desc: &Value) -> Result<Vec<String>, String> {
    let d = descriptor_of(desc)?;
    d.validate().map_err(|e| e.to_string())?;
    Ok(d.parts().into_iter().map(|(_, p)| p.map(|p| p.name).unwrap_or_default()).collect())
}

/// The descriptor's layout for packet type `part` (its only one when it has none), and that type.
fn layout_of(descriptor: &Value, part: Option<&str>) -> Result<(Descriptor, Option<crate::descriptor::Part>), String> {
    let whole = descriptor_of(descriptor)?;
    whole.validate().map_err(|e| e.to_string())?;
    let mut parts = whole.parts();
    // a file with several packet types opens one type at a time, by name (as the app's `open` takes `part`)
    let at = match part {
        Some(p) => parts.iter().position(|(_, q)| q.as_ref().is_some_and(|q| q.name == p)),
        None if parts.len() == 1 => Some(0),
        None => return Err(format!("it has {} packet types: open one at a time", parts.len())),
    }
    .ok_or("the descriptor has no such packet type")?;
    Ok(parts.swap_remove(at))
}

/// The records of a described file as read (a packet file's: each packet's header, then a record), their stride, and
/// what to say about damaged packets.
fn described_records(
    bytes: &[u8],
    desc: &Descriptor,
    part: Option<&crate::descriptor::Part>,
) -> Result<(Vec<u8>, usize, Vec<String>), String> {
    use crate::packets_core::{frame, packet_records};
    let start = (desc.header_size as usize).min(bytes.len());
    let Some(p) = desc.packets.clone() else {
        let stride = desc.record_size as usize;
        let n = (bytes.len() - start) / stride;
        return Ok((bytes[start..start + n * stride].to_vec(), stride, Vec::new()));
    };
    let big = desc.endian == Endian::Big;
    let sync = p.sync_bytes().map_err(|e| e.to_string())?;
    let framing = frame(bytes, start, &p, desc.record_size, part, &sync, big);
    let mut records = Vec::new();
    let mut damaged = framing.bad;
    for f in &framing.packets {
        match packet_records(bytes, &p, desc.record_size, f.offset, f.stored) {
            Some(r) => records.extend_from_slice(&r),
            None => damaged += 1,
        }
    }
    let notes = (damaged > 0)
        .then(|| format!("{damaged} damaged packet{} skipped", if damaged == 1 { "" } else { "s" }))
        .into_iter()
        .collect();
    Ok((records, (p.header_size + desc.record_size) as usize, notes))
}

/// What a descriptor's payload plugin (D-ingest-09) must decode before the page reads the file, since the plugin runs
/// in a Worker of its own: the JSON says which and the bytes hold the input.
///
/// - `{"kind": "packets", plugin, params, type, packets, damaged}`: packet type `part` has a plugin; the bytes are each
///   packet as `[header length u32][payload length u32][header][payload]` (the payload after checksum and
///   decompression), in file order. Its rows then load as a plugin's columns do (`import_columns`).
/// - `{"kind": "fields", records, fields: [{name, plugin, params, dtype, size}]}`: plugin fields; the bytes are each
///   field's `size` bytes of every record, field after field. Their values go back to [`import_described`].
/// - `{"kind": "none"}`: no plugin.
pub fn payload_requests(bytes: &[u8], descriptor: &Value, part: Option<&str>) -> Result<(Value, Vec<u8>), String> {
    use crate::packets_core::{decompress, frame};
    let (desc, part) = layout_of(descriptor, part)?;
    if let (Some(q), Some(p)) = (part.as_ref().filter(|q| q.plugin.is_some()), desc.packets.clone()) {
        let (plugin, params) = q.plugin.clone().expect("checked");
        let big = desc.endian == Endian::Big;
        let sync = p.sync_bytes().map_err(|e| e.to_string())?;
        let framing = frame(bytes, (desc.header_size as usize).min(bytes.len()), &p, 0, Some(q), &sync, big);
        let hs = p.header_size as usize;
        let (mut out, mut n, mut bad) = (Vec::new(), 0usize, framing.bad);
        for f in &framing.packets {
            let at = f.offset as usize;
            match decompress(p.compression, &bytes[at + hs..at + hs + f.stored as usize]) {
                Some(payload) => {
                    out.extend_from_slice(&(hs as u32).to_le_bytes());
                    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
                    out.extend_from_slice(&bytes[at..at + hs]);
                    out.extend_from_slice(&payload);
                    n += 1;
                }
                None => bad += 1,
            }
        }
        let meta = json!({ "kind": "packets", "plugin": plugin, "params": params, "type": q.name, "packets": n, "damaged": bad });
        return Ok((meta, out));
    }
    let fields: Vec<&crate::descriptor::Field> = desc.plugin_fields().collect();
    if fields.is_empty() {
        return Ok((json!({ "kind": "none" }), Vec::new()));
    }
    let (records, stride, _) = described_records(bytes, &desc, part.as_ref())?;
    let n = records.len() / stride;
    let mut out = Vec::new();
    for f in &fields {
        let (at, size) = (f.offset as usize, f.size as usize);
        for i in 0..n {
            out.extend_from_slice(&records[i * stride + at..i * stride + at + size]);
        }
    }
    let list: Vec<Value> = fields
        .iter()
        .map(|f| {
            json!({
                "name": f.name, "plugin": f.plugin, "params": crate::descriptor::params_json(f.params.as_ref()),
                "dtype": f.dtype.name(), "size": f.size, "unit": f.unit,
            })
        })
        .collect();
    Ok((json!({ "kind": "fields", "records": n, "fields": list }), out))
}

/// Reads a binary file laid out by a descriptor the page keeps (fixed-width records after `header_size` bytes, or
/// packets: framing, checksums and decompression by the app's own `packets_core.rs`) into file `k`: packet type `part`
/// of the descriptor's (0 when it has none). Damaged packets are skipped and counted, as the app does. A descriptor
/// with plugin fields needs their values (`plugin`: f64 LE, field after field, a value per record, from the plugin the
/// page ran on [`payload_requests`]); they join the channels after the descriptor's own.
pub fn import_described(
    st: &mut crate::data::State,
    k: usize,
    path: &str,
    bytes: &[u8],
    descriptor: &Value,
    part: Option<&str>,
    plugin: Option<&[u8]>,
) -> Result<Reply, String> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let bad = |msg: String| format!("{name}: {msg}");
    let (desc, part) = layout_of(descriptor, part).map_err(bad)?;
    if let Some((n, _)) = part.as_ref().and_then(|q| q.plugin.as_ref()) {
        return Err(bad(format!(
            "packet type {} is the payload plugin \"{n}\"'s to decode first",
            part.as_ref().map_or("", |q| q.name.as_str())
        )));
    }
    let part_name = part.as_ref().map(|p| p.name.clone());
    let (records, stride, notes) = described_records(bytes, &desc, part.as_ref()).map_err(bad)?;
    let fields: Vec<&crate::descriptor::Field> = desc.plugin_fields().collect();
    let mut extra = Vec::new();
    if !fields.is_empty() {
        let n = records.len() / stride;
        let Some(values) = plugin else {
            let p = fields[0].plugin.as_deref().unwrap_or_default();
            return Err(bad(format!("its field {} needs the payload plugin \"{p}\"", fields[0].name)));
        };
        if values.len() != fields.len() * n * 8 {
            return Err(bad(format!("the payload plugin gave {} values for {} records", values.len() / 8, n)));
        }
        for (k, f) in fields.iter().enumerate() {
            let v: Vec<f64> = values[k * n * 8..(k + 1) * n * 8]
                .chunks_exact(8)
                .map(|b| f64::from_le_bytes(b.try_into().expect("8 bytes")))
                .collect();
            let info = ChannelInfo {
                name: f.name.clone(),
                unit: f.unit.clone(),
                dtype: f.dtype.name(),
                category: None,
                labels: Vec::new(),
            };
            extra.push((info, v));
        }
    }
    import_records(st, k, path, "Descriptor", part_name, &desc, &records, stride, notes, extra)
}

/// Whole records of `stride` bytes laid out by `desc` (a packet file's: the packet header then the record) into file
/// `k`: the time base, each channel's values (scaled), and time that goes backwards flagged and held.
#[allow(clippy::too_many_arguments)]
fn import_records(
    st: &mut crate::data::State,
    k: usize,
    path: &str,
    format: &str,
    part: Option<String>,
    desc: &Descriptor,
    records: &[u8],
    stride: usize,
    mut notes: Vec<String>,
    extra: Vec<(ChannelInfo, Vec<f64>)>,
) -> Result<Reply, String> {
    let n = records.len() / stride;
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
        .map(|f| ChannelInfo {
            name: f.name.clone(),
            unit: f.unit.clone(),
            dtype: f.dtype.name(),
            category: None,
            labels: Vec::new(),
        })
        .chain(extra.iter().map(|(c, _)| c.clone()))
        .collect();
    let mut d = Ds::new(path, format, part, n, time, tm, channels);
    notes.extend(found.note_of("record", "record"));
    d.info.notes = notes;
    for (col, f) in d.cols.iter_mut().zip(&fields) {
        col.raw = crate::data::Raw::whole(field_values(records, stride, f.offset as usize, f.dtype, big, f));
    }
    // a payload plugin's fields, after the descriptor's own (as the app joins them)
    for (col, (_, v)) in d.cols.iter_mut().skip(fields.len()).zip(extra) {
        col.raw = crate::data::Raw::whole(v);
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
        chans.push(json!({ "name": c.name, "unit": c.unit, "category": c.category, "labels": c.labels }));
    }
    let meta = json!({
        "samples": d.len, "time": time, "channels": chans, "format": d.info.format, "part": d.info.part, "skipped": d.info.skipped, "notes": d.info.notes,
        "index": matches!(d.info.time, TimeSource::Index),
    });
    Ok((meta, out))
}

/// A file a WASM decoder plugin decoded in the page (the plugin's Worker sends the columns as `load` reads them: the
/// times when timed, then each channel as f64): time that goes backwards is flagged and held, as the app does for a
/// plugin's file, then it loads as a dump does.
pub fn import_columns(
    st: &mut crate::data::State,
    k: usize,
    path: &str,
    meta: &Value,
    bin: &[u8],
) -> Result<Reply, String> {
    let n = meta["samples"].as_u64().ok_or("no samples")? as usize;
    let mut meta = meta.clone();
    let mut bin = bin.to_vec();
    if meta["time"]["kind"].as_str() == Some("column") {
        let head = bin.get(..n * 8).ok_or("the columns are short")?;
        let mut t: Vec<i64> = head.chunks_exact(8).map(|b| i64::from_le_bytes(b.try_into().unwrap())).collect();
        let mut found = Backwards::default();
        clamp_backwards(&mut t, &mut None, 1, &mut found);
        for (i, v) in t.iter().enumerate() {
            bin[i * 8..i * 8 + 8].copy_from_slice(&v.to_le_bytes());
        }
        if let Some(note) = found.note() {
            meta["notes"] = json!([note]);
        }
    }
    load(st, k, path, &meta, &bin)
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
            labels: c["labels"]
                .as_array()
                .map(|a| a.iter().filter_map(|l| l.as_str().map(str::to_owned)).collect())
                .unwrap_or_default(),
        })
        .collect();
    let mut d = Ds::new(
        path,
        meta["format"].as_str().unwrap_or("CSV"),
        meta["part"].as_str().map(str::to_owned),
        n,
        time,
        tm,
        channels,
    );
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

    /// Zstandard frames (made by the `zstd` tool) of three records (t u16 ms, v i16): (0, 10), (10, 20), (20, 30); and
    /// of one, (30, 40).
    const ZST_3: [u8; 25] = [
        0x28, 0xb5, 0x2f, 0xfd, 0x24, 0x0c, 0x61, 0x00, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x0a, 0x00, 0x14, 0x00, 0x14,
        0x00, 0x1e, 0x00, 0x59, 0x7c, 0x80, 0x63,
    ];
    const ZST_1: [u8; 17] =
        [0x28, 0xb5, 0x2f, 0xfd, 0x24, 0x04, 0x21, 0x00, 0x00, 0x1e, 0x00, 0x28, 0x00, 0x23, 0xfc, 0x9d, 0xa9];

    /// A packet: sync EB90, the u16 payload length, a type byte and a pad byte, the payload, then a sum8 checksum.
    fn packet(kind: u8, payload: &[u8]) -> Vec<u8> {
        let mut out = vec![0xeb, 0x90];
        out.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        out.extend_from_slice(&[kind, 0]);
        out.extend_from_slice(payload);
        out.push(out.iter().fold(0u8, |a, &b| a.wrapping_add(b)));
        out
    }

    const PACKETS_ZSTD: &str = r#"
schema = 1
name = "zp"
record_size = 4
[time]
source = "field"
field = "t"
unit = "ms"
[packets]
sync = "EB90"
header_size = 6
length = { offset = 2, type = "u16" }
checksum = "sum8"
compression = "zstd"
[[fields]]
name = "t"
type = "u16"
offset = 6
[[fields]]
name = "v"
type = "i16"
offset = 8
unit = "V"
"#;

    #[test]
    fn packets_frame_check_and_decompress_zstd_in_the_page() {
        // two good packets with a damaged one between them (its checksum is wrong) and a byte of junk before the first
        let mut bytes = vec![0x55];
        bytes.extend(packet(0, &ZST_3));
        let mut broken = packet(0, &ZST_1);
        *broken.last_mut().unwrap() ^= 0xff;
        bytes.extend(broken);
        bytes.extend(packet(0, &ZST_1));
        assert_eq!(described_parts(&json!(PACKETS_ZSTD)).unwrap(), [""]);
        let mut st = State::default();
        import_described(&mut st, 0, "local://run.bin", &bytes, &json!(PACKETS_ZSTD), None, None).unwrap();
        let d = st.files[0].as_ref().unwrap();
        assert_eq!(d.len, 4, "three records, then one");
        assert_eq!(d.info.format, "Descriptor");
        let Tm::Col(t) = &d.tm else { panic!("a time field") };
        assert_eq!(t, &[0, 10_000_000, 20_000_000, 30_000_000]);
        assert_eq!(d.cols[0].raw.slice(4).unwrap(), [10.0, 20.0, 30.0, 40.0]);
        assert_eq!(d.info.notes, ["1 damaged packet skipped"]);
    }

    #[test]
    fn a_folders_tlmbench_reads_from_a_listing_as_the_app_reads_it() {
        let files = json!({
            "categories.toml": "schema = 1\n[[category]]\nid = \"warn\"\nname = \"Warnings\"\ncolor = \"#f80\"\n",
            "widgets/dial/widget.toml": "schema = 1\nname = \"Dial\"\nkind = \"svg\"\nentry = \"dial.svg\"\n",
            "widgets/dial/dial.svg": "<svg/>",
            "widgets/broken/widget.toml": "schema = 9\nname = \"x\"\nkind = \"svg\"\nentry = \"x.svg\"\n",
            "icons/flag.svg": "<svg id='flag'/>",
            "decoders/esc.toml": FIELD_TIME.replace("name = \"t\"\nrecord_size", "name = \"t\"\nfiles = [\"*.esc\"]\nrecord_size"),
            "decoders/bad.toml": "not toml [",
            "views/a.tlmview.json": "{}",
        });
        let c = config_of(&files).unwrap();
        assert_eq!(c["from"]["categories"][0]["name"], "Warnings");
        assert_eq!(c["from"]["widgets"].as_array().unwrap().len(), 1, "a widget of another schema is left out");
        assert_eq!(c["from"]["widgets"][0]["id"], "dial");
        assert_eq!(c["from"]["widgets"][0]["svg"], "<svg/>");
        assert_eq!(c["from"]["icons"][0]["name"], "flag");
        let d = c["decoders"].as_array().unwrap();
        assert_eq!(d.len(), 1, "a descriptor that doesn't parse is left out");
        assert_eq!((d[0]["id"].as_str(), d[0]["descriptor"]["files"][0].as_str()), (Some("esc"), Some("*.esc")));
    }

    #[test]
    fn a_plugins_columns_load_with_time_going_backwards_flagged() {
        let t: [i64; 4] = [0, 2_000, 1_000, 3_000];
        let v = [1.0f64, 2.0, 3.0, 4.0];
        let mut bin: Vec<u8> = t.iter().flat_map(|x| x.to_le_bytes()).collect();
        bin.extend(v.iter().flat_map(|x| x.to_le_bytes()));
        let meta = json!({ "samples": 4, "time": { "kind": "column", "name": "time" }, "channels": [{ "name": "volts", "unit": "V" }], "format": "demo (plugin)" });
        let mut st = State::default();
        import_columns(&mut st, 0, "x.demo", &meta, &bin).unwrap();
        let d = st.files[0].as_ref().unwrap();
        let Tm::Col(times) = &d.tm else { panic!("a time column") };
        assert_eq!(times, &[0, 2_000, 2_000, 3_000], "held, not reordered");
        assert!(d.info.notes[0].starts_with("Time goes backwards in 1 row"), "{:?}", d.info.notes);
        assert_eq!(d.info.format, "demo (plugin)");
        assert_eq!(d.cols[0].raw.slice(4).unwrap(), v);
    }

    #[test]
    fn lz4_and_gzip_payloads_read_in_the_page() {
        use std::io::Write;
        let records: Vec<u8> = [(0u16, 10i16), (10, 20), (20, 30)]
            .iter()
            .flat_map(|(t, v)| [t.to_le_bytes(), v.to_le_bytes()].concat())
            .collect();
        let lz4 = {
            let mut w = lz4_flex::frame::FrameEncoder::new(Vec::new());
            w.write_all(&records).unwrap();
            w.finish().unwrap()
        };
        let gz = {
            let mut w = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            w.write_all(&records).unwrap();
            w.finish().unwrap()
        };
        for (kind, payload) in [("lz4", lz4), ("gzip", gz)] {
            let toml = PACKETS_ZSTD.replace("compression = \"zstd\"", &format!("compression = \"{kind}\""));
            let mut st = State::default();
            import_described(&mut st, 0, "x.bin", &packet(0, &payload), &json!(toml), None, None).unwrap();
            let d = st.files[0].as_ref().unwrap();
            assert_eq!(d.cols[0].raw.slice(3).unwrap(), [10.0, 20.0, 30.0], "{kind}");
            assert!(d.info.notes.is_empty(), "{kind}: {:?}", d.info.notes);
        }
    }

    #[test]
    fn packet_types_open_one_at_a_time_and_fixed_width_needs_only_a_descriptor() {
        let types = r#"
schema = 1
name = "two"
[packets]
sync = "EB90"
header_size = 6
length = { offset = 2, type = "u16" }
id = { offset = 4, type = "u8" }
[[packet_types]]
id = 1
name = "gps"
record_size = 4
time = { source = "field", field = "t", unit = "ms" }
fields = [ { name = "t", type = "u16", offset = 6 }, { name = "alt", type = "i16", offset = 8 } ]
[[packet_types]]
id = 2
name = "imu"
record_size = 2
time = { source = "index" }
fields = [ { name = "ax", type = "i16", offset = 6 } ]
"#;
        let unchecked = |kind: u8, payload: &[u8]| {
            let mut p = packet(kind, payload);
            p.pop();
            p
        };
        let mut bytes = unchecked(1, &[0, 0, 5, 0]);
        bytes.extend(unchecked(2, &[7, 0, 8, 0]));
        bytes.extend(unchecked(1, &[100, 0, 6, 0]));
        bytes.extend(unchecked(9, &[1, 2]));
        assert_eq!(described_parts(&json!(types)).unwrap(), ["gps", "imu"]);
        let types = &json!(types);
        let mut st = State::default();
        import_described(&mut st, 0, "x.bin", &bytes, types, Some("gps"), None).unwrap();
        import_described(&mut st, 1, "x.bin", &bytes, types, Some("imu"), None).unwrap();
        let (gps, imu) = (st.files[0].as_ref().unwrap(), st.files[1].as_ref().unwrap());
        assert_eq!(gps.cols[0].raw.slice(2).unwrap(), [5.0, 6.0]);
        assert_eq!(imu.cols[0].raw.slice(2).unwrap(), [7.0, 8.0]);
        assert_eq!(gps.info.part.as_deref(), Some("gps"));
        let (meta, bin) = dump(&st, 1).unwrap();
        load(&mut st, 4, "x.bin", &meta, &bin).unwrap();
        assert_eq!(
            st.files[4].as_ref().unwrap().info.part.as_deref(),
            Some("imu"),
            "a type kept in the page keeps its name"
        );
        assert!(import_described(&mut st, 2, "x.bin", &bytes, types, Some("nope"), None).is_err());
        assert!(
            import_described(&mut st, 2, "x.bin", &bytes, types, None, None).err().unwrap().contains("2 packet types")
        );

        // fixed-width records after a header, with the descriptor kept in the page (no packets)
        let fixed = FIELD_TIME.replace("record_size = 8", "record_size = 8\nheader_size = 4");
        let mut raw = vec![0xaa; 4];
        raw.extend(
            &tlm(&[(0, 1000, 1), (500, 2000, 0)], FIELD_TIME)
                [crate::reference::read_header(&tlm(&[], FIELD_TIME)).unwrap().0..],
        );
        // the form's JSON works as well as TOML
        let as_json = serde_json::to_value(Descriptor::parse(&fixed).unwrap()).unwrap();
        import_described(&mut st, 3, "plain.bin", &raw, &as_json, None, None).unwrap();
        assert_eq!(st.files[3].as_ref().unwrap().cols[0].raw.slice(2).unwrap(), [2.0, 3.0]);
    }

    /// Payload plugins in the page (D-ingest-09): the core lists what the plugin must decode, a packet type's packets
    /// (header and payload, after checksum and decompression) or a field's bytes of every record, and loads the
    /// plugin's field values after the descriptor's own channels.
    #[test]
    fn a_payload_plugins_work_is_listed_then_its_values_load() {
        let types = r#"
schema = 1
name = "pl"
[packets]
sync = "EB90"
header_size = 6
length = { offset = 2, type = "u16" }
id = { offset = 4, type = "u8" }
[[packet_types]]
id = 1
name = "status"
plugin = "demo-payload"
params = { temp_scale = 0.5 }
[[packet_types]]
id = 2
name = "imu"
record_size = 5
time = { source = "index" }
fields = [ { name = "ax", type = "i16", offset = 6 }, { name = "packed", type = "f64", offset = 8, size = 3, plugin = "demo-payload" } ]
"#;
        let unchecked = |kind: u8, payload: &[u8]| {
            let mut p = packet(kind, payload);
            p.pop();
            p
        };
        let mut bytes = unchecked(1, &[1, 2, 3]);
        bytes.extend(unchecked(2, &[7, 0, 0, 0, 1]));
        bytes.extend(unchecked(1, &[4, 5]));
        bytes.extend(unchecked(2, &[8, 0, 0, 0, 2, 9, 0, 0, 0, 3]));
        let types = &json!(types);
        // the plugin packet type: two packets, header then payload each
        let (meta, bin) = payload_requests(&bytes, types, Some("status")).unwrap();
        assert_eq!(
            (meta["kind"].as_str(), meta["plugin"].as_str(), meta["type"].as_str(), meta["packets"].as_u64()),
            (Some("packets"), Some("demo-payload"), Some("status"), Some(2))
        );
        assert_eq!(meta["params"].as_str(), Some(r#"{"temp_scale":0.5}"#));
        let first = |b: &[u8]| {
            (u32::from_le_bytes(b[0..4].try_into().unwrap()), u32::from_le_bytes(b[4..8].try_into().unwrap()))
        };
        assert_eq!(first(&bin), (6, 3));
        assert_eq!(&bin[8 + 6..8 + 9], &[1, 2, 3]);
        assert_eq!(bin.len(), (8 + 6 + 3) + (8 + 6 + 2));
        let mut st = State::default();
        assert!(
            import_described(&mut st, 0, "x.bin", &bytes, types, Some("status"), None)
                .err()
                .unwrap()
                .contains("decode first")
        );

        // the plugin field: 3 bytes of each of the 3 records
        let (meta, bin) = payload_requests(&bytes, types, Some("imu")).unwrap();
        assert_eq!((meta["kind"].as_str(), meta["records"].as_u64()), (Some("fields"), Some(3)));
        assert_eq!(
            (
                meta["fields"][0]["name"].as_str(),
                meta["fields"][0]["size"].as_u64(),
                meta["fields"][0]["dtype"].as_str()
            ),
            (Some("packed"), Some(3), Some("f64"))
        );
        assert_eq!(bin, [0, 0, 1, 0, 0, 2, 0, 0, 3]);
        assert!(
            import_described(&mut st, 0, "x.bin", &bytes, types, Some("imu"), None)
                .err()
                .unwrap()
                .contains("needs the payload plugin")
        );
        let values: Vec<u8> = [10.0f64, 20.0, 30.0].iter().flat_map(|v| v.to_le_bytes()).collect();
        assert!(import_described(&mut st, 0, "x.bin", &bytes, types, Some("imu"), Some(&values[..16])).is_err());
        import_described(&mut st, 0, "x.bin", &bytes, types, Some("imu"), Some(&values)).unwrap();
        let d = st.files[0].as_ref().unwrap();
        let names: Vec<&str> = d.info.channels.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["ax", "packed"]);
        assert_eq!(d.cols[0].raw.slice(3).unwrap(), [7.0, 8.0, 9.0]);
        assert_eq!(d.cols[1].raw.slice(3).unwrap(), [10.0, 20.0, 30.0]);

        // a descriptor with no plugin asks for nothing
        let none = payload_requests(&bytes, &json!(FIELD_TIME), None).unwrap();
        assert_eq!(none.0["kind"].as_str(), Some("none"));
    }
}
