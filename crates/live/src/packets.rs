//! Live packet streams (ledger D-ingest-11): a socket or serial port carrying packets that a descriptor's `[packets]`
//! table frames. The same rules as a file frame them (`tlmbench_decode::packets::frame_stream`), and each packet type
//! is recorded as a source of its own (owner, Nov 16), all from one connection.
//!
//! A type's records are written as they are read from a file, the packet header then the record, so its `.tlm` is laid
//! out by the type's own fields at the same offsets. A field a payload plugin decodes gets its value appended to each
//! record; a type a payload plugin decodes is written as the plugin's rows (a time, then its channels).

use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, Packets, TimeSpec, TimeUnit, params_json};
use tlmbench_decode::packets::{decompress, frame_stream, packet_records_of};
use tlmbench_decode::payload::{PayloadSession, payload_plugins};

use super::Ring;

/// How one packet type is recorded.
pub(crate) struct Plan {
    /// The type's id (`None` without `[[packet_types]]`).
    pub id: Option<i64>,
    /// The type's name, for the source's file and card.
    pub name: Option<String>,
    /// The `.tlm` records' layout.
    pub record: Descriptor,
    kind: Kind,
}

enum Kind {
    /// Records as read (`hs + rs` bytes each), then the plugin fields' values appended.
    Records { rs: u64, fields: Vec<PluginField> },
    /// A payload plugin's rows: a time (when timed) and its channels.
    Plugin { session: Box<dyn PayloadSession>, params: String, channels: Vec<DType>, timed: bool },
}

struct PluginField {
    session: Box<dyn PayloadSession>,
    name: String,
    params: String,
    dtype: DType,
    offset: usize,
    size: usize,
}

fn session(name: &str, what: &str) -> Result<Box<dyn PayloadSession>, String> {
    let p = payload_plugins().into_iter().find(|p| p.info().name == name).ok_or_else(|| {
        format!("{what} needs the plugin \"{name}\", which isn't loaded (Settings › Plugins lists them)")
    })?;
    p.session().map_err(|e| format!("{name}: {e}"))
}

fn plain(name: String, dtype: DType, offset: u64, unit: Option<String>) -> Field {
    Field { name, dtype, offset, unit, scale: 1.0, value_offset: 0.0, bits: None, plugin: None, size: 0, params: None }
}

/// One plan per packet type of `d` (which has `[packets]`), in the descriptor's order.
pub(crate) fn plans(d: &Descriptor) -> Result<Vec<Plan>, String> {
    let p = d.packets.as_ref().expect("a packet descriptor");
    let hs = p.header_size;
    let mut out = Vec::new();
    for (td, part) in d.parts() {
        let name = part.as_ref().map(|q| q.name.clone());
        let id = part.as_ref().map(|q| q.id);
        let label = name.clone().or_else(|| d.name.clone());
        let mut record = Descriptor {
            schema: d.schema,
            name: label.clone(),
            files: Vec::new(),
            endian: d.endian,
            header_size: 0,
            record_size: 0,
            sequence: None,
            time: TimeSpec::Index,
            fields: Vec::new(),
            packets: None,
            packet_types: Vec::new(),
        };
        if let Some((plugin, params)) = part.as_ref().and_then(|q| q.plugin.clone()) {
            let what = format!("packet type \"{}\"", label.clone().unwrap_or_default());
            let mut s = session(&plugin, &what)?;
            let schema =
                s.packet_schema(&label.clone().unwrap_or_default(), &params).map_err(|e| format!("{plugin}: {e}"))?;
            let mut at = 0;
            if schema.timed {
                record.fields.push(plain("time".into(), DType::I64, 0, None));
                record.time = TimeSpec::Field { field: "time".into(), unit: TimeUnit::Ns, t0_ns: 0 };
                at = 8;
            }
            for c in &schema.channels {
                record.fields.push(plain(c.name.clone(), c.dtype, at, c.unit.clone()));
                at += c.dtype.size();
            }
            record.record_size = at.max(1);
            out.push(Plan {
                id,
                name,
                record,
                kind: Kind::Plugin {
                    session: s,
                    params,
                    channels: schema.channels.iter().map(|c| c.dtype).collect(),
                    timed: schema.timed,
                },
            });
            continue;
        }
        // records as read: the packet header, then the record; plugin fields' values after them
        let mut at = hs + td.record_size;
        let mut fields = Vec::new();
        for f in &td.fields {
            match &f.plugin {
                None => record.fields.push(f.clone()),
                Some(plugin) => {
                    let s = session(plugin, &format!("field \"{}\"", f.name))?;
                    fields.push(PluginField {
                        session: s,
                        name: f.name.clone(),
                        params: params_json(f.params.as_ref()),
                        dtype: f.dtype,
                        offset: f.offset as usize,
                        size: f.size as usize,
                    });
                    record.fields.push(plain(f.name.clone(), f.dtype, at, f.unit.clone()));
                    at += f.dtype.size();
                }
            }
        }
        (record.time, record.sequence, record.record_size) = (td.time.clone(), td.sequence.clone(), at);
        record.validate().map_err(|e| e.to_string())?;
        out.push(Plan { id, name, record, kind: Kind::Records { rs: td.record_size, fields } });
    }
    Ok(out)
}

/// Frames a packet stream and hands each type's records to its ring.
pub(crate) struct Demux {
    buf: Vec<u8>,
    p: Packets,
    known: Vec<(i64, u64)>,
    record_size: u64,
    sync: Vec<u8>,
    big: bool,
    plans: Vec<Plan>,
}

/// The most a stream keeps while it waits for the rest of a packet.
const MAX_PENDING: usize = 64 << 20;

impl Demux {
    pub(crate) fn new(d: &Descriptor, plans: Vec<Plan>) -> Result<Demux, String> {
        let p = d.packets.clone().expect("a packet descriptor");
        let sync = p.sync_bytes().map_err(|e| e.to_string())?;
        Ok(Demux {
            buf: Vec::new(),
            known: d.packet_types.iter().map(|t| (t.id, t.record_size)).collect(),
            record_size: d.record_size,
            big: d.endian == Endian::Big,
            p,
            sync,
            plans,
        })
    }

    /// Takes the next bytes of the stream: every packet now whole goes to its type's ring (`rings[k]` for plan `k`).
    pub(crate) fn feed(&mut self, bytes: &[u8], rings: &mut [Ring]) {
        for r in rings.iter() {
            r.counters.bytes_in.fetch_add(bytes.len() as u64, std::sync::atomic::Ordering::Relaxed);
        }
        self.buf.extend_from_slice(bytes);
        let f = frame_stream(&self.buf, &self.p, &self.known, self.record_size, &self.sync, self.big);
        let malformed = |rings: &mut [Ring], n: usize| {
            if let Some(r) = rings.first() {
                r.counters.malformed.fetch_add(n as u64, std::sync::atomic::Ordering::Relaxed);
            }
        };
        malformed(rings, f.skipped);
        let hs = self.p.header_size as usize;
        for pk in &f.packets {
            let Some(k) = self.plans.iter().position(|pl| pl.id == pk.id) else { continue };
            let plan = &mut self.plans[k];
            match &mut plan.kind {
                Kind::Records { rs, fields } => {
                    let Some(recs) = packet_records_of(&self.buf, &self.p, *rs, pk.offset as u64, pk.stored as u64)
                    else {
                        malformed(rings, hs + pk.stored);
                        continue;
                    };
                    let read = hs + *rs as usize;
                    if fields.is_empty() {
                        rings[k].push(&recs);
                        continue;
                    }
                    let n = recs.len() / read;
                    let mut values = Vec::new();
                    for pf in fields.iter_mut() {
                        let data: Vec<u8> =
                            recs.chunks_exact(read).flat_map(|r| r[pf.offset..pf.offset + pf.size].to_vec()).collect();
                        match pf.session.decode_field(&pf.name, &pf.params, pf.dtype, pf.size as u32, n as u32, &data) {
                            Ok(v) if v.len() == n * pf.dtype.size() as usize => {
                                values.push((v, pf.dtype.size() as usize))
                            }
                            _ => values.push((vec![0; n * pf.dtype.size() as usize], pf.dtype.size() as usize)),
                        }
                    }
                    let mut out = Vec::with_capacity(recs.len() + values.iter().map(|v| v.0.len()).sum::<usize>());
                    for (i, r) in recs.chunks_exact(read).enumerate() {
                        out.extend_from_slice(r);
                        for (v, w) in &values {
                            out.extend_from_slice(&v[i * w..(i + 1) * w]);
                        }
                    }
                    rings[k].push(&out);
                }
                Kind::Plugin { session, params, channels, timed } => {
                    let at = pk.offset;
                    let Some(payload) = decompress(self.p.compression, &self.buf[at + hs..at + hs + pk.stored]) else {
                        malformed(rings, hs + pk.stored);
                        continue;
                    };
                    let header = self.buf[at..at + hs].to_vec();
                    let name = plan.name.clone().unwrap_or_default();
                    let Ok(rows) = session.decode_packets(&name, params, &[(header, payload.into_owned())]) else {
                        malformed(rings, hs + pk.stored);
                        continue;
                    };
                    let sizes: Vec<usize> = channels.iter().map(|d| d.size() as usize).collect();
                    if rows.columns.len() != sizes.len()
                        || rows.columns.iter().zip(&sizes).any(|(c, s)| c.len() != rows.rows * s)
                        || (*timed && rows.time.len() != rows.rows)
                    {
                        malformed(rings, hs + pk.stored);
                        continue;
                    }
                    let mut out = Vec::new();
                    for i in 0..rows.rows {
                        if *timed {
                            out.extend_from_slice(&rows.time[i].to_le_bytes());
                        }
                        for (c, s) in rows.columns.iter().zip(&sizes) {
                            out.extend_from_slice(&c[i * s..(i + 1) * s]);
                        }
                    }
                    rings[k].push(&out);
                }
            }
        }
        self.buf.drain(..f.consumed);
        // a stream that never completes a packet doesn't grow without end
        if self.buf.len() > MAX_PENDING {
            malformed(rings, self.buf.len());
            self.buf.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::net::TcpListener;
    use std::sync::Arc;

    use tlmbench_core::Recording;
    use tlmbench_core::chunks::Held;
    use tlmbench_decode::fixed::FixedWidth;
    use tlmbench_decode::packets::encode_packet;
    use tlmbench_decode::payload::{PayloadPlugin, set_payload_plugins};
    use tlmbench_decode::plugin::{Chunk, PluginChannel, PluginInfo, PluginSchema, Tier};

    use super::*;
    use crate::{Endpoint, Options, Session};

    /// A payload plugin in Rust: a status packet is a time in µs and a temperature; a field's 8 bytes are a u64, halved.
    #[derive(Debug)]
    struct Fake(PluginInfo);
    struct FakeSession;

    impl PayloadPlugin for Fake {
        fn info(&self) -> &PluginInfo {
            &self.0
        }
        fn session(&self) -> Result<Box<dyn PayloadSession>, String> {
            Ok(Box::new(FakeSession))
        }
    }

    impl PayloadSession for FakeSession {
        fn packet_schema(&mut self, _: &str, _: &str) -> Result<PluginSchema, String> {
            Ok(PluginSchema {
                channels: vec![PluginChannel { name: "temp".into(), unit: None, dtype: DType::F32 }],
                timed: true,
            })
        }
        fn decode_packets(&mut self, _: &str, _: &str, packets: &[(Vec<u8>, Vec<u8>)]) -> Result<Chunk, String> {
            let mut c = Chunk { rows: 0, time: Vec::new(), columns: vec![Vec::new()] };
            for (_, p) in packets {
                c.time.push(u64::from_le_bytes(p[0..8].try_into().unwrap()) as i64 * 1000);
                c.columns[0].extend_from_slice(&p[8..12]);
                c.rows += 1;
            }
            Ok(c)
        }
        fn decode_field(
            &mut self,
            _: &str,
            _: &str,
            _: DType,
            size: u32,
            _: u32,
            data: &[u8],
        ) -> Result<Vec<u8>, String> {
            Ok(data
                .chunks_exact(size as usize)
                .flat_map(|b| (u64::from_le_bytes(b.try_into().unwrap()) as f64 / 2.0).to_le_bytes())
                .collect())
        }
    }

    const ROVER: &str = r#"
schema = 1
name = "rover"
[packets]
sync = "EB90"
header_size = 6
length = { offset = 2, type = "u16" }
id = { offset = 4, type = "u8" }
checksum = "crc16-ccitt"
compression = "zstd"

[[packet_types]]
id = 1
name = "imu"
record_size = 9
time = { source = "field", field = "t", unit = "us" }
fields = [
  { name = "t", type = "u64", offset = 6 },
  { name = "count", type = "u8", offset = 14 },
  { name = "half", type = "f64", offset = 6, plugin = "fake", size = 8 },
]

[[packet_types]]
id = 2
name = "status"
plugin = "fake"
"#;

    #[test]
    fn a_packet_stream_records_a_source_per_type_from_one_connection() {
        set_payload_plugins(vec![Arc::new(Fake(PluginInfo {
            name: "fake".into(),
            version: "1".into(),
            tier: Tier::Wasm,
            description: None,
            files: Vec::new(),
            path: "fake".into(),
        }))]);
        let d = Descriptor::parse(ROVER).unwrap();
        let p = d.packets.clone().unwrap();
        let mut sent = Vec::new();
        for k in 0..50u64 {
            let imu: Vec<u8> = (0..4u64)
                .flat_map(|j| {
                    let i = k * 4 + j;
                    [(i * 1000).to_le_bytes().to_vec(), vec![i as u8]].concat()
                })
                .collect();
            sent.extend(encode_packet(&p, false, &[0, 0, 0, 0, 1, 0], &imu).unwrap());
            if k % 10 == 3 {
                sent.extend_from_slice(b"junk"); // damage between packets: skipped, then the next sync is found
            }
            let status =
                [(k * 4000 + 500).to_le_bytes().to_vec(), (20.0f32 + k as f32).to_le_bytes().to_vec()].concat();
            sent.extend(encode_packet(&p, false, &[0, 0, 0, 0, 2, 0], &status).unwrap());
        }
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let sender = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            // odd-sized writes: packets arrive split across reads
            for part in sent.chunks(333) {
                s.write_all(part).unwrap();
            }
        });
        let dir = tempfile::tempdir().unwrap();
        let mut session = Session::create(dir.path()).unwrap();
        let sources = session.add_sources(Endpoint::Tcp(addr.to_string()), Some(&d), &Options::default()).unwrap();
        assert_eq!(sources.iter().map(|s| s.part().unwrap().to_owned()).collect::<Vec<_>>(), ["imu", "status"]);
        let health: Vec<_> = sources.into_iter().map(|s| s.wait()).collect();
        sender.join().unwrap();
        for h in &health {
            assert!(h.ended && h.error.is_none(), "{h:?}");
        }
        assert_eq!((health[0].records_written, health[1].records_written), (200, 50));
        assert_eq!(health[0].malformed_bytes, 5 * 4, "the junk was skipped");
        let h = Held::default();
        let imu = FixedWidth::open_reference(&health[0].files[0]).unwrap();
        let names: Vec<_> = imu.info().channels.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["count", "half"]);
        assert_eq!((imu.value_at(0, 77, &h), imu.value_at(1, 77, &h)), (77.0, 38_500.0));
        assert_eq!(imu.time_at(77), 77_000_000);
        let status = FixedWidth::open_reference(&health[1].files[0]).unwrap();
        assert_eq!(status.info().channels[0].name, "temp");
        assert_eq!((status.value_at(0, 9, &h), status.time_at(9)), (29.0, 36_500_000));
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(session.dir().join("session.json")).unwrap()).unwrap();
        assert_eq!(json["sources"][1]["descriptor"], "status");
        assert_eq!(json["sources"][1]["files"][0], "source-2.tlm");
    }
}
