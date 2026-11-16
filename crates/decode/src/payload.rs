//! Payload plugins: a descriptor names one for a packet type or for one field (`plugin = "name"`, see the design's
//! "Decoder plugins" and `docs/follow-ups.md`, ledger D-ingest-09).
//!
//! The decoders do the framing: sync words, lengths, checksums and decompression stay theirs. A plugin gets bytes and
//! gives values:
//!
//! - **A packet type** with `plugin`: each of its packets' header and payload (after checksum and decompression), a
//!   batch at a time, and it returns rows for the channels it declares, with their times. Such a type has no record
//!   layout, so it decodes into the cache once, as a plugin file does ([`crate::plugin::open_with`]).
//! - **A field** with `plugin` and `size`: that field's bytes of many records at once, and one value per record. The
//!   rest of the recording is read as usual; the plugin's columns are decoded once into the cache and joined to it
//!   ([`WithPluginFields`]).
//!
//! The plugins themselves (WASM components) live in `tlmbench-plugin`, which registers them here.

use std::io::Write;
use std::ops::Range;
use std::path::Path;
use std::sync::{Arc, RwLock};

use tlmbench_core::chunks::{Chunk as DataChunk, Held};
use tlmbench_core::gaps::GapIndex;
use tlmbench_core::mask::Mask;
use tlmbench_core::pyramid::{Pyramid, Stats, Summary};
use tlmbench_core::{Column, Recording, RecordingInfo, TimeIndex, Timestamp, Trace, Viewport};

use crate::bytes::{self, Bytes};
use crate::cache::{self, CachedRecording, MANIFEST_VERSION, Manifest, ManifestChannel, Source};
use crate::descriptor::{DType, Descriptor, Endian, Field, Part, params_json};
use crate::error::{Result, format_err, io_err};
use crate::packets_core::{decompress, frame, packets_of};
use crate::plugin::{Chunk, DecoderSession, PluginInfo, PluginSchema};

/// One file's use of a payload plugin: an instance of its own, so a plugin may keep state between calls.
pub trait PayloadSession: Send {
    /// The channels it makes of packet type `type_name` (params as JSON).
    fn packet_schema(&mut self, type_name: &str, params: &str) -> std::result::Result<PluginSchema, String>;
    /// Rows from some packets of that type, each `(header, payload)`, in file order.
    fn decode_packets(
        &mut self,
        type_name: &str,
        params: &str,
        packets: &[(Vec<u8>, Vec<u8>)],
    ) -> std::result::Result<Chunk, String>;
    /// `count` values of `dtype` (little-endian) from `count` runs of `size` bytes.
    fn decode_field(
        &mut self,
        field: &str,
        params: &str,
        dtype: DType,
        size: u32,
        count: u32,
        data: &[u8],
    ) -> std::result::Result<Vec<u8>, String>;
}

pub trait PayloadPlugin: Send + Sync + std::fmt::Debug {
    fn info(&self) -> &PluginInfo;
    fn session(&self) -> std::result::Result<Box<dyn PayloadSession>, String>;
}

static PAYLOADS: RwLock<Vec<Arc<dyn PayloadPlugin>>> = RwLock::new(Vec::new());

/// Replaces the list of payload plugins (the app does this when its plugins folders are read).
pub fn set_payload_plugins(list: Vec<Arc<dyn PayloadPlugin>>) {
    *PAYLOADS.write().unwrap() = list;
}

pub fn payload_plugins() -> Vec<Arc<dyn PayloadPlugin>> {
    PAYLOADS.read().unwrap().clone()
}

/// The loaded payload plugin called `name`, or an error that says where to look.
fn named(path: &Path, what: &str, name: &str) -> Result<Arc<dyn PayloadPlugin>> {
    payload_plugins().into_iter().find(|p| p.info().name == name).ok_or_else(|| {
        format_err(
            path,
            format!("{what} needs the plugin \"{name}\", which isn't loaded (Settings › Plugins lists them)"),
        )
    })
}

/// Packets handed over at once: at most this many, or about this many payload bytes.
const BATCH_PACKETS: usize = 4096;
const BATCH_BYTES: usize = 16 << 20;
/// Records whose plugin field bytes are handed over at once.
const BATCH_RECORDS: usize = 65_536;

/// Opens packet type `part` of `path`, which a payload plugin decodes (`desc` is the type's own descriptor, see
/// [`Descriptor::parts`]), decoding it into the cache under `root` first if it isn't there.
pub fn open_type(path: &Path, desc: &Descriptor, part: &Part, format: &str, root: &Path) -> Result<CachedRecording> {
    let (name, params) = part.plugin.clone().expect("a type a plugin decodes");
    let plugin = named(path, &format!("packet type \"{}\"", part.name), &name)?;
    let info = plugin.info().clone();
    let source = Source::open(path)?;
    let mut key = blake3::Hasher::new();
    key.update(source.fingerprint()?.as_bytes());
    key.update(desc.to_toml().as_bytes());
    key.update(format!("\npart {}\0{}\0{}\0{params}", part.id, info.name, info.version).as_bytes());
    let dir = root.join("payload").join(&key.finalize().to_hex()[..32]);
    let (manifest, bad) = match cache::read_manifest(&dir) {
        Some(m) => {
            let bad = std::fs::read_to_string(dir.join("bad")).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            (m, bad)
        }
        None => {
            let mut bad = 0;
            let m = cache::build_entry(&dir, |tmp| {
                let mut session = PacketSession::start(path, desc, part, plugin.as_ref(), &params)?;
                let m = crate::plugin::build_from(path, &source, &info.name, &mut session, tmp)?;
                bad = session.bad;
                std::fs::write(tmp.join("bad"), bad.to_string()).map_err(io_err(tmp))?;
                Ok(m)
            })?;
            (m, bad)
        }
    };
    let _ = std::fs::write(dir.join("opened"), b"");
    let mut rec = cache::load(path, &dir, manifest, format)?;
    let i = rec.info_mut();
    i.part = Some(part.name.clone());
    i.notes.push(format!("Decoded by the plugin \"{}\" {}", info.name, info.version));
    if bad > 0 {
        i.notes.push(format!("{bad} damaged packets skipped (length, checksum or compression)"));
    }
    Ok(rec)
}

/// A packet type's packets, a batch at a time, through a payload plugin.
struct PacketSession {
    data: Bytes,
    /// Each packet's start and stored payload length.
    packets: Vec<(u64, u64)>,
    next: usize,
    header: usize,
    compression: crate::descriptor::Compression,
    session: Box<dyn PayloadSession>,
    type_name: String,
    params: String,
    schema: PluginSchema,
    bad: usize,
}

impl PacketSession {
    fn start(path: &Path, desc: &Descriptor, part: &Part, plugin: &dyn PayloadPlugin, params: &str) -> Result<Self> {
        let who = |msg: String| format_err(path, format!("{}: {msg}", plugin.info().name));
        let data = bytes::map(path)?;
        let p = packets_of(desc).clone();
        let sync = p.sync_bytes().map_err(|e| format_err(path, e.to_string()))?;
        let big = desc.endian == Endian::Big;
        let framing = frame(&data, desc.header_size as usize, &p, 0, Some(part), &sync, big);
        let mut session = plugin.session().map_err(who)?;
        let schema = session.packet_schema(&part.name, params).map_err(who)?;
        Ok(Self {
            packets: framing.packets.iter().map(|f| (f.offset, f.stored)).collect(),
            data,
            next: 0,
            header: p.header_size as usize,
            compression: p.compression,
            session,
            type_name: part.name.clone(),
            params: params.to_owned(),
            schema,
            bad: framing.bad,
        })
    }
}

impl DecoderSession for PacketSession {
    fn schema(&self) -> &PluginSchema {
        &self.schema
    }

    fn next(&mut self, _max_rows: u32) -> std::result::Result<Option<Chunk>, String> {
        // a batch whose packets gave no rows is not the end: go on to the next
        while self.next < self.packets.len() {
            let mut batch = Vec::new();
            let mut bytes = 0;
            while self.next < self.packets.len() && batch.len() < BATCH_PACKETS && bytes < BATCH_BYTES {
                let (at, stored) = self.packets[self.next];
                self.next += 1;
                let (at, hs) = (at as usize, self.header);
                match decompress(self.compression, &self.data[at + hs..at + hs + stored as usize]) {
                    Some(payload) => {
                        bytes += payload.len() + hs;
                        batch.push((self.data[at..at + hs].to_vec(), payload.into_owned()));
                    }
                    None => self.bad += 1,
                }
            }
            if batch.is_empty() {
                continue;
            }
            let chunk = self.session.decode_packets(&self.type_name, &self.params, &batch)?;
            if chunk.rows > 0 {
                return Ok(Some(chunk));
            }
        }
        Ok(None)
    }
}

/// How the plugin fields' bytes are read from a recording: records `a..b` as read, `stride` bytes each.
pub(crate) trait RecordBytes {
    fn stride(&self) -> usize;
    fn record_bytes(&self, a: usize, b: usize) -> Vec<u8>;
}

/// The plugin fields of `desc` (it has some), decoded from `base` into the cache under `root`: join them to the
/// recording with [`WithPluginFields::new`].
pub(crate) fn decode_fields<R: Recording + RecordBytes>(
    base: &R,
    desc: &Descriptor,
    root: &Path,
) -> Result<CachedRecording> {
    let path = base.info().path.clone();
    let fields: Vec<&Field> = desc.plugin_fields().collect();
    let mut plugins = Vec::new();
    for f in &fields {
        let name = f.plugin.as_deref().expect("a plugin field");
        plugins.push(named(&path, &format!("field \"{}\"", f.name), name)?);
    }
    let source = Source::open(&path)?;
    let mut key = blake3::Hasher::new();
    key.update(source.fingerprint()?.as_bytes());
    key.update(desc.to_toml().as_bytes());
    key.update(format!("\npart {:?}", base.info().part).as_bytes());
    for p in &plugins {
        key.update(format!("\0{}\0{}", p.info().name, p.info().version).as_bytes());
    }
    let dir = root.join("payload").join(&key.finalize().to_hex()[..32]);
    let manifest = match cache::read_manifest(&dir) {
        Some(m) => m,
        None => cache::build_entry(&dir, |tmp| write_fields(base, &fields, &plugins, &source, tmp))?,
    };
    let _ = std::fs::write(dir.join("opened"), b"");
    cache::load(&path, &dir, manifest, &base.info().format)
}

fn write_fields<R: Recording + RecordBytes>(
    base: &R,
    fields: &[&Field],
    plugins: &[Arc<dyn PayloadPlugin>],
    source: &Source,
    tmp: &Path,
) -> Result<Manifest> {
    let path = &base.info().path;
    let n = base.info().samples;
    let stride = base.stride();
    let create = |name: &str| -> Result<std::io::BufWriter<std::fs::File>> {
        Ok(std::io::BufWriter::new(std::fs::File::create(tmp.join(name)).map_err(io_err(tmp))?))
    };
    let mut sessions = Vec::new();
    for p in plugins {
        sessions.push(p.session().map_err(|e| format_err(path, format!("{}: {e}", p.info().name)))?);
    }
    let names: Vec<String> = (0..fields.len()).map(|k| format!("c{k}.{}", fields[k].dtype.name())).collect();
    let mut outs = names.iter().map(|n| create(n)).collect::<Result<Vec<_>>>()?;
    let mut t_out = create("t.i64")?;
    let (mut t_first, mut t_last) = (None, None);
    for a in (0..n).step_by(BATCH_RECORDS) {
        let b = (a + BATCH_RECORDS).min(n);
        let recs = base.record_bytes(a, b);
        let count = recs.len() / stride.max(1);
        let times = base.read_times(a, a + count);
        t_first = t_first.or(times.first().copied());
        t_last = times.last().copied().or(t_last);
        t_out.write_all(bytemuck::cast_slice(&times)).map_err(io_err(tmp))?;
        for ((f, s), (w, p)) in fields.iter().zip(sessions.iter_mut()).zip(outs.iter_mut().zip(plugins)) {
            let (off, size) = (f.offset as usize, f.size as usize);
            let mut data = Vec::with_capacity(count * size);
            for r in recs.chunks_exact(stride).take(count) {
                data.extend_from_slice(&r[off..off + size]);
            }
            let params = params_json(f.params.as_ref());
            let who = |msg: String| format_err(path, format!("{} (field \"{}\"): {msg}", p.info().name, f.name));
            let col = s.decode_field(&f.name, &params, f.dtype, size as u32, count as u32, &data).map_err(who)?;
            if col.len() != count * f.dtype.size() as usize {
                return Err(who(format!("returned {} bytes for {count} values of {}", col.len(), f.dtype.name())));
            }
            w.write_all(&col).map_err(io_err(tmp))?;
        }
    }
    for mut w in outs.into_iter().chain([t_out]) {
        w.flush().map_err(io_err(tmp))?;
    }
    Ok(Manifest {
        version: MANIFEST_VERSION,
        source_size: source.len,
        rows: n,
        skipped_rows: 0,
        time: Some("time".into()),
        channels: fields
            .iter()
            .zip(names)
            .map(|(f, file)| ManifestChannel {
                name: f.name.clone(),
                unit: f.unit.clone(),
                category: None,
                file,
                dtype: f.dtype,
                labels: Vec::new(),
            })
            .collect(),
        skipped_columns: Vec::new(),
        t_first,
        t_last,
        rate: None,
        backwards: Default::default(),
        notes: Vec::new(),
    })
}

/// A recording and the columns its plugin fields decoded to: the recording's channels, then the plugin fields (the
/// columns share its times, written beside them).
#[derive(Debug)]
pub struct WithPluginFields {
    base: Box<dyn Recording>,
    extra: CachedRecording,
    info: RecordingInfo,
    n: usize,
}

impl WithPluginFields {
    pub(crate) fn new(base: Box<dyn Recording>, extra: CachedRecording) -> Self {
        let mut info = base.info().clone();
        let n = info.channels.len();
        info.channels.extend(Recording::info(&extra).channels.iter().cloned());
        Self { base, extra, info, n }
    }

    fn at(&self, c: usize) -> (&dyn Recording, usize) {
        if c < self.n { (self.base.as_ref(), c) } else { (&self.extra, c - self.n) }
    }
}

impl Recording for WithPluginFields {
    fn info(&self) -> &RecordingInfo {
        &self.info
    }
    fn view(&self, channel: usize, vp: Viewport, pyr: Option<&Pyramid>, breaks: &[usize], held: &Held) -> Trace {
        let (r, c) = self.at(channel);
        r.view(c, vp, pyr, breaks, held)
    }
    fn view_masked(&self, channel: usize, vp: Viewport, mask: &Mask, breaks: &[usize], held: &Held) -> Trace {
        let (r, c) = self.at(channel);
        r.view_masked(c, vp, mask, breaks, held)
    }
    fn stats_masked(&self, channel: usize, pyr: &Pyramid, a: usize, b: usize, mask: &Mask, held: &Held) -> Stats {
        let (r, c) = self.at(channel);
        r.stats_masked(c, pyr, a, b, mask, held)
    }
    fn view_coarse(&self, channel: usize, vp: Viewport, pyr: &Pyramid, breaks: &[usize], held: &Held) -> Trace {
        let (r, c) = self.at(channel);
        r.view_coarse(c, vp, pyr, breaks, held)
    }
    fn summarize(&self, channel: usize, start: usize, end: usize) -> Vec<Summary> {
        let (r, c) = self.at(channel);
        r.summarize(c, start, end)
    }
    fn prefetch(&self, ranges: &[Range<usize>]) {
        self.base.prefetch(ranges);
    }
    fn summarize_all(&self, start: usize, end: usize) -> Vec<Vec<Summary>> {
        let mut all = self.base.summarize_all(start, end);
        all.extend(self.extra.summarize_all(start, end));
        all
    }
    fn stats(&self, channel: usize, pyr: &Pyramid, a: usize, b: usize, held: &Held) -> Stats {
        let (r, c) = self.at(channel);
        r.stats(c, pyr, a, b, held)
    }
    fn value_at(&self, channel: usize, i: usize, held: &Held) -> f64 {
        let (r, c) = self.at(channel);
        r.value_at(c, i, held)
    }
    fn read(&self, channel: usize, start: usize, end: usize) -> Column {
        let (r, c) = self.at(channel);
        r.read(c, start, end)
    }
    fn read_times(&self, start: usize, end: usize) -> Vec<Timestamp> {
        self.base.read_times(start, end)
    }
    fn scale_offset(&self, channel: usize) -> (f64, f64) {
        let (r, c) = self.at(channel);
        r.scale_offset(c)
    }
    fn missing_mask(&self, channel: usize) -> Mask {
        let (r, c) = self.at(channel);
        r.missing_mask(c)
    }
    fn chunked(&self, channel: usize) -> bool {
        let (r, c) = self.at(channel);
        r.chunked(c)
    }
    fn set_time_index(&self, index: TimeIndex) {
        self.base.set_time_index(index.clone());
        self.extra.set_time_index(index);
    }
    fn time_index(&self) -> TimeIndex {
        self.base.time_index()
    }
    fn decode_chunks(&self, channels: &[usize], chunks: Range<usize>) -> Vec<Vec<DataChunk>> {
        let (mut own, mut more) = (Vec::new(), Vec::new());
        for &c in channels {
            if c < self.n { own.push(c) } else { more.push(c - self.n) }
        }
        let mut a = self.base.decode_chunks(&own, chunks.clone()).into_iter();
        let mut b = self.extra.decode_chunks(&more, chunks).into_iter();
        channels.iter().map(|&c| if c < self.n { a.next() } else { b.next() }.expect("one per channel")).collect()
    }
    fn time_at(&self, i: usize) -> Timestamp {
        self.base.time_at(i)
    }
    fn lower_bound(&self, t: Timestamp) -> usize {
        self.base.lower_bound(t)
    }
    fn gap_index(&self) -> GapIndex {
        self.base.gap_index()
    }
    fn extend_gap_index(&self, gaps: &mut GapIndex, from: usize) {
        self.base.extend_gap_index(gaps, from);
    }
    fn layout_id(&self) -> Vec<u8> {
        let mut id = self.base.layout_id();
        id.extend_from_slice(b"+plugin-fields");
        id.extend(self.extra.layout_id());
        id
    }
}
