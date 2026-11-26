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
    Field {
        name,
        dtype,
        offset,
        unit,
        scale: 1.0,
        value_offset: 0.0,
        category: None,
        bits: None,
        plugin: None,
        size: 0,
        params: None,
    }
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
            events: Vec::new(),
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
        install();
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

    fn info(name: &str) -> PluginInfo {
        PluginInfo {
            name: name.into(),
            version: "1".into(),
            tier: Tier::Wasm,
            description: None,
            files: Vec::new(),
            path: name.into(),
        }
    }

    /// A payload plugin that misbehaves in the way its name says: it has no session, no schema, no time, or returns
    /// rows that are wrong or fail.
    #[derive(Debug)]
    struct Odd(PluginInfo);
    struct OddSession(String);

    impl PayloadPlugin for Odd {
        fn info(&self) -> &PluginInfo {
            &self.0
        }
        fn session(&self) -> Result<Box<dyn PayloadSession>, String> {
            match self.0.name.as_str() {
                "nosession" => Err("it won't start".into()),
                n => Ok(Box::new(OddSession(n.into()))),
            }
        }
    }

    impl PayloadSession for OddSession {
        fn packet_schema(&mut self, _: &str, _: &str) -> Result<PluginSchema, String> {
            match self.0.as_str() {
                "noschema" => Err("no schema for you".into()),
                n => Ok(PluginSchema {
                    channels: vec![PluginChannel { name: "temp".into(), unit: None, dtype: DType::F32 }],
                    timed: n != "untimed",
                }),
            }
        }
        fn decode_packets(&mut self, _: &str, _: &str, _: &[(Vec<u8>, Vec<u8>)]) -> Result<Chunk, String> {
            let col = |n: usize| vec![vec![0u8; n]];
            match self.0.as_str() {
                "badrows" => Err("can't read these".into()),
                // two rows announced, one row's bytes
                "shortrows" => Ok(Chunk { rows: 2, time: vec![0, 1], columns: col(4) }),
                // a timed type's rows without their times
                "shorttime" => Ok(Chunk { rows: 1, time: Vec::new(), columns: col(4) }),
                // untimed: one row, 1.5
                _ => Ok(Chunk { rows: 1, time: Vec::new(), columns: vec![1.5f32.to_le_bytes().to_vec()] }),
            }
        }
        fn decode_field(&mut self, _: &str, _: &str, _: DType, _: u32, _: u32, _: &[u8]) -> Result<Vec<u8>, String> {
            Err("can't read this field".into())
        }
    }

    /// Installs every plugin these tests use, once for all: the list is one per process, so tests that each set it
    /// would take it from one another when they share a process.
    fn install() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            let mut all: Vec<Arc<dyn PayloadPlugin>> = vec![Arc::new(Fake(info("fake")))];
            for n in ["nosession", "noschema", "untimed", "badrows", "shortrows", "shorttime", "badfield"] {
                all.push(Arc::new(Odd(info(n))));
            }
            set_payload_plugins(all);
        });
    }

    fn descriptor(compression: &str, types: &str) -> Descriptor {
        Descriptor::parse(&format!(
            "schema = 1\nname = \"t\"\n[packets]\nsync = \"EB90\"\nheader_size = 6\nlength = {{ offset = 2, type = \"u16\" }}\n\
             id = {{ offset = 4, type = \"u8\" }}\nchecksum = \"crc16-ccitt\"\ncompression = \"{compression}\"\n{types}"
        ))
        .unwrap()
    }

    const IMU: &str = r#"
[[packet_types]]
id = 1
name = "imu"
record_size = 9
time = { source = "field", field = "t", unit = "us" }
fields = [
  { name = "t", type = "u64", offset = 6 },
  { name = "count", type = "u8", offset = 14 },
]
"#;

    fn typed(plugin: &str) -> String {
        format!("[[packet_types]]\nid = 2\nname = \"status\"\nplugin = \"{plugin}\"\n")
    }

    fn packet(d: &Descriptor, id: u8, payload: &[u8]) -> Vec<u8> {
        encode_packet(d.packets.as_ref().unwrap(), false, &[0, 0, 0, 0, id, 0], payload).unwrap()
    }

    /// Feeds `chunks` to a demux of `d`, each type's ring behind it: what each ring got, and the malformed bytes.
    fn feed_all(d: &Descriptor, chunks: &[Vec<u8>]) -> (Vec<Vec<u8>>, u64) {
        let plans = plans(d).unwrap();
        let sizes: Vec<usize> = plans.iter().map(|p| p.record.record_size as usize).collect();
        let mut demux = Demux::new(d, plans).unwrap();
        let counters: Vec<crate::Counters> = sizes.iter().map(|_| Default::default()).collect();
        let parts: Vec<crate::Parts> = sizes.iter().map(|_| Default::default()).collect();
        let (mut rings, mut outs) = (Vec::new(), Vec::new());
        for (k, &rs) in sizes.iter().enumerate() {
            let (producer, consumer) = rtrb::RingBuffer::<u8>::new(1 << 16);
            outs.push(consumer);
            rings.push(Ring { producer, rs, cap: 1 << 16, pushed: 0, counters: &counters[k], parts: &parts[k] });
        }
        for c in chunks {
            demux.feed(c, &mut rings);
        }
        let got = outs
            .iter_mut()
            .map(|c| {
                let chunk = c.read_chunk(c.slots()).unwrap();
                let (a, b) = chunk.as_slices();
                let v = [a, b].concat();
                chunk.commit_all();
                v
            })
            .collect();
        (got, counters[0].malformed.load(std::sync::atomic::Ordering::Relaxed))
    }

    fn imu_payload(range: std::ops::Range<u64>) -> Vec<u8> {
        range.flat_map(|i| [(i * 1000).to_le_bytes().to_vec(), vec![i as u8]].concat()).collect()
    }

    /// A type with no plugin goes to its ring as read (header, then record), whole packets only; a packet whose size
    /// isn't a whole number of records is counted as malformed, and the stream goes on.
    #[test]
    fn plain_records_go_through_as_read() {
        install();
        let d = descriptor("none", IMU);
        let stream = [
            packet(&d, 1, &imu_payload(0..2)),
            packet(&d, 1, &imu_payload(0..1)[..5]), // 5 bytes: not a record
            packet(&d, 1, &imu_payload(2..3)),
        ]
        .concat();
        let whole = |chunks: Vec<Vec<u8>>| {
            let (out, malformed) = feed_all(&d, &chunks);
            assert_eq!(out[0].len(), 3 * 15, "three records of header and payload");
            assert_eq!((out[0][4], &out[0][6..14], out[0][14]), (1, &0u64.to_le_bytes()[..], 0), "the first record");
            assert_eq!((&out[0][36..44], out[0][44]), (&2000u64.to_le_bytes()[..], 2), "the third");
            assert_eq!(malformed, 6 + 5, "the odd packet's header and bytes");
        };
        whole(vec![stream.clone()]);
        whole(stream.chunks(7).map(<[u8]>::to_vec).collect());
    }

    /// A type without a name or an id (a stream of one kind of packet) is recorded under the descriptor's name.
    #[test]
    fn a_stream_of_one_kind_is_one_plan_named_for_the_descriptor() {
        let d = Descriptor::parse(
            r#"
schema = 1
name = "beacon"
record_size = 2
time = { source = "rate", rate_hz = 100.0 }
fields = [{ name = "v", type = "u16", offset = 4 }]
[packets]
sync = "EB90"
header_size = 4
length = { offset = 2, type = "u16" }
"#,
        )
        .unwrap();
        let plans = plans(&d).unwrap();
        assert_eq!(plans.len(), 1);
        assert_eq!(
            (plans[0].id, plans[0].name.clone(), plans[0].record.name.clone()),
            (None, None, Some("beacon".into()))
        );
        let p = encode_packet(d.packets.as_ref().unwrap(), false, &[0; 4], &[1, 0, 2, 0]).unwrap();
        let (out, malformed) = feed_all(&d, &[p]);
        assert_eq!((out[0].len(), malformed), (12, 0), "two records of header and value");
        assert_eq!((out[0][4], out[0][10]), (1, 2));
    }

    /// A plugin that can't be had: the plan says which one, and for what.
    #[test]
    fn plans_say_which_plugin_they_cannot_have() {
        install();
        let err = |plugin: &str| plans(&descriptor("none", &typed(plugin))).err().expect("a refusal");
        assert!(err("ghost").contains("packet type \"status\" needs the plugin \"ghost\", which isn't loaded"));
        assert_eq!(err("nosession"), "nosession: it won't start");
        assert_eq!(err("noschema"), "noschema: no schema for you");
        // a field's plugin is looked for the same way
        let d = descriptor(
            "none",
            &IMU.replace(
                "offset = 14 }",
                "offset = 14 },\n  { name = \"h\", type = \"f64\", offset = 6, plugin = \"ghost\", size = 8 }",
            ),
        );
        let e = plans(&d).err().expect("a refusal");
        assert!(e.contains("field \"h\" needs the plugin \"ghost\""), "{e}");
        // a record layout that doesn't hold together
        let mut d = descriptor("none", IMU);
        d.packet_types[0].time = TimeSpec::Field { field: "nope".into(), unit: TimeUnit::Us, t0_ns: 0 };
        assert!(plans(&d).err().expect("a refusal").contains("nope"));
        // a sync word that isn't hex is the demux's refusal
        let mut d = descriptor("none", IMU);
        let p = plans(&d).unwrap();
        d.packets.as_mut().unwrap().sync = Some("zz".into());
        assert!(Demux::new(&d, p).err().expect("a refusal").contains("sync"));
    }

    /// A plugin's rows with no time are written as its channels alone.
    #[test]
    fn a_plugin_type_without_time_records_its_channels_only() {
        install();
        let d = descriptor("none", &typed("untimed"));
        let p = plans(&d).unwrap();
        assert_eq!(p[0].record.record_size, 4);
        assert_eq!(p[0].record.time, TimeSpec::Index);
        let (out, malformed) = feed_all(&d, &[packet(&d, 2, &[0; 4])]);
        assert_eq!((out[0].clone(), malformed), (1.5f32.to_le_bytes().to_vec(), 0));
    }

    /// Rows a plugin can't give, or gives wrongly, are counted as malformed: the packet's header and bytes.
    #[test]
    fn a_plugins_bad_rows_are_malformed_packets() {
        install();
        for plugin in ["badrows", "shortrows", "shorttime"] {
            let d = descriptor("none", &typed(plugin));
            let stream = [packet(&d, 2, &[1; 12]), packet(&d, 2, &[2; 12])].concat();
            let (out, malformed) = feed_all(&d, &[stream]);
            assert!(out[0].is_empty(), "{plugin}");
            assert_eq!(malformed, 2 * (6 + 12), "{plugin}");
        }
    }

    /// A payload that doesn't decompress is malformed too.
    #[test]
    fn a_payload_that_does_not_decompress_is_malformed() {
        install();
        let d = descriptor("zstd", &typed("untimed"));
        let mut raw = d.clone();
        raw.packets.as_mut().unwrap().compression = tlmbench_decode::descriptor::Compression::None;
        // framed as plain bytes, read as zstd
        let (out, malformed) = feed_all(&d, &[packet(&raw, 2, &[7; 12]), packet(&d, 2, &[0; 4])]);
        assert_eq!(out[0], 1.5f32.to_le_bytes().to_vec(), "the good packet after it is read");
        assert_eq!(malformed, 6 + 12);
    }

    /// A plugin that can't decode a field leaves zeros where its values go, and the record is kept.
    #[test]
    fn a_field_the_plugin_cannot_decode_is_zero() {
        install();
        let d = descriptor(
            "none",
            &IMU.replace(
                "offset = 14 }",
                "offset = 14 },\n  { name = \"h\", type = \"f64\", offset = 6, plugin = \"badfield\", size = 8 }",
            ),
        );
        let (out, malformed) = feed_all(&d, &[packet(&d, 1, &imu_payload(0..2))]);
        assert_eq!((out[0].len(), malformed), (2 * (15 + 8), 0));
        assert_eq!(&out[0][15..23], &[0u8; 8]);
        assert_eq!((out[0][14], &out[0][23 + 6..23 + 14]), (0, &1000u64.to_le_bytes()[..]));
    }

    /// A stream that claims a packet longer than anything arrives doesn't grow without end: past 64 MB waiting, it is
    /// dropped and counted, and the next whole packet is read.
    #[test]
    fn a_packet_that_never_completes_is_dropped_at_the_limit() {
        let mut d = descriptor("none", IMU);
        d.packets.as_mut().unwrap().length = Some(tlmbench_decode::descriptor::LengthField {
            offset: 2,
            dtype: DType::U32,
            counts: Default::default(),
            adjust: 0,
        });
        d.packets.as_mut().unwrap().header_size = 8;
        d.packets.as_mut().unwrap().id.as_mut().unwrap().offset = 6;
        let p = d.packets.clone().unwrap();
        let mut head = vec![0xEB, 0x90, 0, 0, 0, 0, 1, 0];
        head[2..6].copy_from_slice(&900_000_000u32.to_le_bytes());
        let mut stream = head;
        stream.resize(MAX_PENDING + 16, 0);
        let good = encode_packet(&p, false, &[0, 0, 0, 0, 0, 0, 1, 0], &imu_payload(0..1)).unwrap();
        let (_, malformed) = feed_all(&d, &[stream.clone(), good]);
        assert_eq!(malformed as usize, stream.len(), "everything held was let go");
    }
}
