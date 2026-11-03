//! Packet files: records framed in packets (a sync word, a length field, a checksum) with a payload that may be
//! compressed, laid out by a descriptor's `[packets]` table (see [`crate::descriptor::Packets`]).
//!
//! The first open scans the file once and keeps a packet index in the cache (never beside the file): each good
//! packet's offset, stored payload length and record count. A packet whose length is impossible, whose checksum
//! fails, or whose payload doesn't decompress to whole records is skipped and counted, and the scan finds the next
//! one by its sync word (without one, the rest of the file is skipped). Packets are decompressed on demand: queries
//! read decoded chunks, which decode the packets they span in parallel, and anything else reads through a small
//! cache of recently decoded packets.
//!
//! Each record is read as the packet header followed by the record's bytes, so header fields (a packet time, a
//! counter) read like record fields.

use std::borrow::Cow;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tlmbench_core::chunks::{CHUNK_SAMPLES, Chunk, Chunked, Held};
use tlmbench_core::pyramid::{self, Summary};
use tlmbench_core::{
    ChannelInfo, ChannelOp, Channels, Indexed, RateTimes, RecordingInfo, Samples, Shifted, TimeIndex, TimeSource,
    Times, TimesOp, Timestamp,
};

use crate::bytes::{self, Bytes};
use crate::cache::Source;
use crate::descriptor::{Checksum, Compression, Counts, DType, Descriptor, Endian, Packets, Part, TimeSpec};
use crate::error::{Result, format_err, io_err};
use crate::fixed::gather_tile;
use crate::strided::Pod;

const INDEX_VERSION: u32 = 1;
/// The most a packet's payload may hold, stored or decompressed.
const MAX_PAYLOAD: u64 = 1 << 30;
/// Decoded packets kept for reads outside decoded chunks.
const KEEP_DECODED: usize = 32;
/// Packets decompressed at once while the index is built.
const SCAN_BATCH: usize = 4096;

/// A packet decoded by the index build: its records, their first and last times (with a time field), and whether
/// its times are in order.
type Scanned = (u32, Option<(i64, i64)>, bool);

/// One good packet: where it starts, its stored payload's length, and how many records it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    offset: u64,
    stored: u32,
    records: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct IndexManifest {
    version: u32,
    source_size: u64,
    packets: usize,
    records: usize,
    /// Packets skipped: an impossible length, a bad checksum, or a payload that didn't decompress to whole records.
    bad_packets: usize,
    /// Bytes outside good packets.
    skipped_bytes: u64,
    t_first: Option<i64>,
    t_last: Option<i64>,
}

/// A packet file, read through its packet index.
#[derive(Debug)]
pub struct PacketFile {
    bytes: Bytes,
    desc: Descriptor,
    index: Vec<Entry>,
    /// First record of each packet, then the total.
    first: Vec<usize>,
    /// Index into `desc.fields` of each channel.
    channels: Vec<usize>,
    info: RecordingInfo,
    /// Packets skipped as damaged.
    pub bad_packets: usize,
    /// Bytes outside good packets.
    pub skipped_bytes: u64,
    decoded: Mutex<Vec<(usize, Arc<Vec<u8>>)>>,
    time_index: OnceLock<TimeIndex>,
    /// The packet type this recording holds, in a file with several.
    part: Option<Part>,
}

fn packets_of(desc: &Descriptor) -> &Packets {
    desc.packets.as_ref().expect("a packet file's descriptor has [packets]")
}

/// An integer field of `dtype` at the start of `b`, as i64.
fn read_int(b: &[u8], dtype: DType, big: bool) -> i64 {
    macro_rules! read {
        ($t:ty) => {{
            let v: $t = if big { <$t as Pod>::from_be(b) } else { <$t as Pod>::from_le(b) };
            v as i64
        }};
    }
    match dtype {
        DType::I8 => read!(i8),
        DType::I16 => read!(i16),
        DType::I32 => read!(i32),
        DType::I64 => read!(i64),
        DType::U8 => read!(u8),
        DType::U16 => read!(u16),
        DType::U32 => read!(u32),
        DType::U64 => read!(u64),
        DType::F32 | DType::F64 => unreachable!("validated: integer fields"),
    }
}

/// `kind`'s checksum of `data`.
pub fn checksum(kind: Checksum, data: &[u8]) -> u64 {
    match kind {
        Checksum::None => 0,
        Checksum::Sum8 => u64::from(data.iter().fold(0u8, |a, &b| a.wrapping_add(b))),
        Checksum::Crc16Ccitt => {
            let mut crc: u16 = 0xffff;
            for &b in data {
                crc ^= u16::from(b) << 8;
                for _ in 0..8 {
                    crc = if crc & 0x8000 != 0 { (crc << 1) ^ 0x1021 } else { crc << 1 };
                }
            }
            u64::from(crc)
        }
        Checksum::Crc32 => u64::from(crc32fast::hash(data)),
    }
}

/// An unsigned number of `size` bytes at the start of `b`.
fn read_uint(b: &[u8], size: usize, big: bool) -> u64 {
    (0..size).fold(0u64, |v, k| (v << 8) | u64::from(if big { b[k] } else { b[size - 1 - k] }))
}

/// The payload decompressed (borrowed when stored plain); None when it doesn't decompress or holds over 1 GiB.
pub fn decompress(kind: Compression, stored: &[u8]) -> Option<Cow<'_, [u8]>> {
    let limited = |r: &mut dyn Read| -> Option<Vec<u8>> {
        let mut out = Vec::new();
        r.take(MAX_PAYLOAD + 1).read_to_end(&mut out).ok()?;
        (out.len() as u64 <= MAX_PAYLOAD).then_some(out)
    };
    match kind {
        Compression::None => Some(Cow::Borrowed(stored)),
        Compression::Zstd => limited(&mut zstd::stream::read::Decoder::new(stored).ok()?).map(Cow::Owned),
        Compression::Lz4 => limited(&mut lz4_flex::frame::FrameDecoder::new(stored)).map(Cow::Owned),
        Compression::Gzip => limited(&mut flate2::read::GzDecoder::new(stored)).map(Cow::Owned),
    }
}

/// One packet's bytes for records `payload` (whole records): `header` (`header_size` bytes, the caller's fields
/// filled in) gets the sync word and the length, the payload is compressed, and the checksum follows. The inverse
/// of reading; the generator and tests write packet files with it.
pub fn encode_packet(p: &Packets, big: bool, header: &[u8], payload: &[u8]) -> Result<Vec<u8>> {
    use std::io::Write;
    let stored: Vec<u8> = match p.compression {
        Compression::None => payload.to_vec(),
        Compression::Zstd => zstd::encode_all(payload, 3).map_err(io_err("<packet>"))?,
        Compression::Lz4 => {
            let mut w = lz4_flex::frame::FrameEncoder::new(Vec::new());
            w.write_all(payload).map_err(io_err("<packet>"))?;
            w.finish().map_err(|e| format_err("<packet>", e.to_string()))?
        }
        Compression::Gzip => {
            let mut w = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            w.write_all(payload).map_err(io_err("<packet>"))?;
            w.finish().map_err(io_err("<packet>"))?
        }
    };
    let (hs, cks) = (p.header_size as usize, p.checksum.size() as usize);
    let mut out = header[..hs].to_vec();
    let sync = p.sync_bytes()?;
    out[..sync.len()].copy_from_slice(&sync);
    if let Some(l) = &p.length {
        let len = stored.len() as i64 - l.adjust + if l.counts == Counts::Packet { (hs + cks) as i64 } else { 0 };
        let (size, at) = (l.dtype.size() as usize, l.offset as usize);
        for k in 0..size {
            let shift = 8 * if big { size - 1 - k } else { k };
            out[at + k] = (len >> shift) as u8;
        }
    }
    out.extend_from_slice(&stored);
    let c = checksum(p.checksum, &out);
    for k in 0..cks {
        let shift = 8 * if big { cks - 1 - k } else { k };
        out.push((c >> shift) as u8);
    }
    Ok(out)
}

/// A packet found by the framing scan, before its payload is decompressed.
#[derive(Clone, Copy, Debug)]
struct Framed {
    offset: u64,
    stored: u64,
}

/// The framing scan's result.
#[derive(Debug, Default)]
struct Framing {
    packets: Vec<Framed>,
    bad: usize,
    /// Bytes of good packets of other types.
    other_bytes: u64,
}

/// Finds the packets of `part` (every packet without one) in `data` from `start`: their offsets and stored payload
/// lengths, and how many were skipped. Without a length field a packet holds one record, of the type's size.
fn frame(
    data: &[u8],
    start: usize,
    p: &Packets,
    record_size: u64,
    part: Option<&Part>,
    sync: &[u8],
    big: bool,
) -> Framing {
    let hs = p.header_size as usize;
    let cks = p.checksum.size() as usize;
    let finder = (!sync.is_empty()).then(|| memchr::memmem::Finder::new(sync));
    // damaged packets are counted once, by the first type
    let counts = part.is_none_or(|p| p.first);
    let mut out = Framing::default();
    let mut pos = start;
    while pos < data.len() {
        if let Some(f) = &finder {
            match f.find(&data[pos..]) {
                Some(i) => pos += i,
                None => break,
            }
        }
        if pos + hs > data.len() {
            break; // a partial packet at the end
        }
        let id = part.map(|q| read_int(&data[pos + q.field.offset as usize..], q.field.dtype, big));
        let known = id.and_then(|id| part?.known.iter().find(|k| k.0 == id)).map(|k| k.1);
        // without a length field, each packet holds one record
        let stored = match &p.length {
            None => known.unwrap_or(record_size) as i64,
            Some(l) => {
                let raw = read_int(&data[pos + l.offset as usize..], l.dtype, big).saturating_add(l.adjust);
                match l.counts {
                    Counts::Payload => raw,
                    Counts::Packet => raw.saturating_sub((hs + cks) as i64),
                }
            }
        };
        // an id no type claims can't be framed by size, but can by its length field
        let unframed = p.length.is_none() && part.is_some() && known.is_none();
        let end = (!unframed && (0..=MAX_PAYLOAD as i64).contains(&stored)).then(|| pos + hs + cks + stored as usize);
        let good = match end {
            Some(e) if e <= data.len() => {
                let body = pos + hs + stored as usize;
                cks == 0 || read_uint(&data[body..], cks, big) == checksum(p.checksum, &data[pos..body])
            }
            Some(_) if finder.is_none() => break, // runs past the end: a partial last packet
            _ => false,
        };
        match (good, end) {
            (true, Some(e)) => {
                match (part, known) {
                    (Some(q), Some(_)) if id == Some(q.id) => {
                        out.packets.push(Framed { offset: pos as u64, stored: stored as u64 })
                    }
                    (Some(_), Some(_)) => out.other_bytes += (e - pos) as u64,
                    (Some(_), None) => {
                        out.bad += usize::from(counts);
                        out.other_bytes += (e - pos) as u64;
                    }
                    (None, _) => out.packets.push(Framed { offset: pos as u64, stored: stored as u64 }),
                }
                pos = e;
            }
            _ => {
                out.bad += usize::from(counts);
                if finder.is_none() {
                    break; // nothing to find the next packet by
                }
                pos += 1;
            }
        }
    }
    out
}

impl PacketFile {
    /// Opens a packet file laid out by `desc` (which has `[packets]`), building its packet index under `root` first
    /// if needed. A file with `[[packet_types]]` opens once per type: `desc` is the type's own layout (see
    /// [`Descriptor::parts`]) and `part` picks its packets.
    pub fn open(path: &Path, desc: Descriptor, part: Option<Part>, format: &str, root: &Path) -> Result<Self> {
        if part.is_none() {
            desc.validate().map_err(|e| format_err(path, e.to_string()))?;
        }
        if desc.packets.is_none() {
            return Err(format_err(path, "the descriptor has no [packets] table"));
        }
        let source = Source::open(path)?;
        let mut key = blake3::Hasher::new();
        key.update(source.fingerprint()?.as_bytes());
        key.update(desc.to_toml().as_bytes());
        if let Some(q) = &part {
            key.update(format!("\npart {}", q.id).as_bytes());
        }
        let dir = root.join("packets").join(&key.finalize().to_hex()[..32]);
        let mut me = Self {
            bytes: bytes::map(path)?,
            index: Vec::new(),
            first: vec![0],
            channels: desc.channels().map(|(i, _)| i).collect(),
            info: RecordingInfo {
                path: path.to_owned(),
                format: format.to_owned(),
                part: part.as_ref().map(|q| q.name.clone()),
                samples: 0,
                time: TimeSource::Index,
                t_first: None,
                t_last: None,
                channels: Vec::new(),
                skipped: Vec::new(),
            },
            bad_packets: 0,
            skipped_bytes: 0,
            decoded: Mutex::default(),
            time_index: OnceLock::new(),
            desc,
            part,
        };
        let manifest = match read_index(&dir) {
            Some((m, index)) if m.source_size == source.len => {
                me.set_index(index);
                m
            }
            _ => me.build_index(&dir, source.len)?,
        };
        // the cache evicts the least recently opened entries first (the store's convention)
        let _ = fs::write(dir.join("opened"), b"");
        me.bad_packets = manifest.bad_packets;
        me.skipped_bytes = manifest.skipped_bytes;
        me.info.channels = me
            .channels
            .iter()
            .map(|&i| {
                let f = &me.desc.fields[i];
                ChannelInfo { name: f.name.clone(), unit: f.unit.clone(), dtype: f.dtype.name() }
            })
            .collect();
        me.info.time = match &me.desc.time {
            TimeSpec::Rate { rate_hz, .. } => TimeSource::Rate { hz: *rate_hz },
            TimeSpec::Field { field, .. } => TimeSource::Column { name: field.clone() },
            TimeSpec::Index => TimeSource::Index,
        };
        let n = me.info.samples;
        (me.info.t_first, me.info.t_last) = match &me.desc.time {
            TimeSpec::Field { .. } => (manifest.t_first, manifest.t_last),
            _ if n > 0 => (Some(me.rate_times().at(0)), Some(me.rate_times().at(n - 1))),
            _ => (None, None),
        };
        Ok(me)
    }

    fn set_index(&mut self, index: Vec<Entry>) {
        let mut n = 0usize;
        self.first = std::iter::once(0)
            .chain(index.iter().map(|e| {
                n += e.records as usize;
                n
            }))
            .collect();
        self.index = index;
        self.info.samples = n;
    }

    fn big(&self) -> bool {
        self.desc.endian == Endian::Big
    }

    /// Bytes of one record as read: the packet header, then the record.
    fn stride(&self) -> usize {
        (packets_of(&self.desc).header_size + self.desc.record_size) as usize
    }

    fn records(&self) -> usize {
        self.first[self.first.len() - 1]
    }

    fn rate_times(&self) -> RateTimes {
        match &self.desc.time {
            TimeSpec::Rate { rate_hz, t0_ns } => RateTimes::from_rate(*t0_ns, *rate_hz, self.records()),
            _ => RateTimes::from_rate(0, 1.0, self.records()),
        }
    }

    /// The records of the packet at `offset` with `stored` payload bytes, as read (header then record, each).
    fn records_of(&self, offset: u64, stored: u64) -> Option<Vec<u8>> {
        let p = packets_of(&self.desc);
        let (hs, at) = (p.header_size as usize, offset as usize);
        let header = &self.bytes[at..at + hs];
        let payload = decompress(p.compression, &self.bytes[at + hs..at + hs + stored as usize])?;
        let rs = self.desc.record_size as usize;
        if payload.len() % rs != 0 {
            return None;
        }
        let mut out = Vec::with_capacity(payload.len() / rs * (hs + rs));
        for r in payload.chunks_exact(rs) {
            out.extend_from_slice(header);
            out.extend_from_slice(r);
        }
        Some(out)
    }

    /// The time of each record in `recs` (records as read), when time is a field.
    fn times_of(&self, recs: &[u8]) -> Vec<Timestamp> {
        let TimeSpec::Field { field, unit, t0_ns } = &self.desc.time else { return Vec::new() };
        let f = self.desc.fields.iter().find(|f| &f.name == field).expect("validated");
        let (off, big) = (f.offset as usize, self.big());
        recs.chunks_exact(self.stride())
            .map(|r| t0_ns.saturating_add(read_int(&r[off..], f.dtype, big).saturating_mul(unit.ns())))
            .collect()
    }

    /// Scans the file and decompresses every packet (in parallel, a batch at a time) to count its records and check
    /// that times never go backwards; writes the index under `dir` (in a temporary folder renamed into place).
    fn build_index(&mut self, dir: &Path, source_size: u64) -> Result<IndexManifest> {
        let path = self.info.path.clone();
        let p = packets_of(&self.desc).clone();
        let sync = p.sync_bytes().map_err(|e| format_err(&path, e.to_string()))?;
        let part = self.part.as_ref();
        let framing =
            frame(&self.bytes, self.desc.header_size as usize, &p, self.desc.record_size, part, &sync, self.big());
        let mut index = Vec::with_capacity(framing.packets.len());
        let mut bad = framing.bad;
        let (mut t_first, mut t_last) = (None, None::<i64>);
        let field_time = matches!(self.desc.time, TimeSpec::Field { .. });
        for batch in framing.packets.chunks(SCAN_BATCH) {
            let decoded: Vec<Option<Scanned>> = batch
                .par_iter()
                .map(|f| {
                    let recs = self.records_of(f.offset, f.stored)?;
                    let n = u32::try_from(recs.len() / self.stride()).ok()?;
                    if !field_time || n == 0 {
                        return Some((n, None, true));
                    }
                    let t = self.times_of(&recs);
                    let ordered = t.windows(2).all(|w| w[0] <= w[1]);
                    Some((n, Some((t[0], t[t.len() - 1])), ordered))
                })
                .collect();
            for (f, d) in batch.iter().zip(decoded) {
                let Some((records, span, ordered)) = d else {
                    bad += 1;
                    continue;
                };
                if let Some((a, b)) = span {
                    if !ordered || t_last.is_some_and(|l| a < l) {
                        return Err(format_err(
                            &path,
                            format!("time goes backwards in the packet at byte {}", f.offset),
                        ));
                    }
                    t_first = t_first.or(Some(a));
                    t_last = Some(b);
                }
                index.push(Entry { offset: f.offset, stored: f.stored as u32, records });
            }
        }
        let other_bytes = framing.other_bytes;
        let used: u64 = index.iter().map(|e| p.header_size + p.checksum.size() + u64::from(e.stored)).sum::<u64>();
        let records: usize = index.iter().map(|e| e.records as usize).sum();
        let manifest = IndexManifest {
            version: INDEX_VERSION,
            source_size,
            packets: index.len(),
            records,
            bad_packets: bad,
            skipped_bytes: source_size.saturating_sub(used + other_bytes),
            t_first,
            t_last,
        };
        write_index(dir, &manifest, &index)?;
        self.set_index(index);
        Ok(manifest)
    }

    /// Packet `k`'s records as read, through the cache of recently decoded packets.
    fn packet(&self, k: usize) -> Arc<Vec<u8>> {
        let mut d = self.decoded.lock().unwrap();
        if let Some(pos) = d.iter().position(|(j, _)| *j == k) {
            let hit = d.remove(pos);
            d.insert(0, hit.clone());
            return hit.1;
        }
        drop(d);
        let e = self.index[k];
        // the index lists only packets that decoded when it was built
        let rec = Arc::new(self.records_of(e.offset, u64::from(e.stored)).unwrap_or_default());
        let mut d = self.decoded.lock().unwrap();
        d.insert(0, (k, rec.clone()));
        d.truncate(KEEP_DECODED);
        rec
    }

    /// The packet holding record `i`.
    fn packet_of(&self, i: usize) -> usize {
        self.first.partition_point(|&f| f <= i) - 1
    }

    /// Records `a..b` as read, decoding the packets they span (not through the cache).
    fn read_records(&self, a: usize, b: usize) -> Vec<u8> {
        let stride = self.stride();
        let mut out = Vec::with_capacity(b.saturating_sub(a) * stride);
        if a >= b {
            return out;
        }
        let mut k = self.packet_of(a);
        while k < self.index.len() && self.first[k] < b {
            let e = self.index[k];
            let recs = self.records_of(e.offset, u64::from(e.stored)).unwrap_or_default();
            let (s, t) = (a.max(self.first[k]) - self.first[k], b.min(self.first[k + 1]) - self.first[k]);
            out.extend_from_slice(&recs[(s * stride).min(recs.len())..(t * stride).min(recs.len())]);
            k += 1;
        }
        out
    }

    /// Record `i` as read.
    fn record(&self, i: usize) -> (Arc<Vec<u8>>, usize) {
        let k = self.packet_of(i);
        (self.packet(k), (i - self.first[k]) * self.stride())
    }
}

/// One field of every record, decoded through the packet cache.
#[derive(Debug)]
struct PacketField<'a, V> {
    file: &'a PacketFile,
    offset: usize,
    _v: std::marker::PhantomData<V>,
}

impl<V: Pod> PacketField<'_, V> {
    fn decode(&self, b: &[u8]) -> V {
        if self.file.big() { V::from_be(b) } else { V::from_le(b) }
    }
}

impl<V: Pod> Samples for PacketField<'_, V> {
    type Item = V;

    fn len(&self) -> usize {
        self.file.records()
    }

    fn get(&self, i: usize) -> V {
        let (p, at) = self.file.record(i);
        self.decode(&p[at + self.offset..])
    }

    fn range(&self, start: usize, end: usize) -> impl DoubleEndedIterator<Item = V> + ExactSizeIterator + Clone {
        let stride = self.file.stride();
        let recs = self.file.read_records(start, end);
        let v: Vec<V> = recs.chunks_exact(stride).map(|r| self.decode(&r[self.offset..])).collect();
        v.into_iter()
    }
}

/// The times of a time field, decoded through the packet cache.
#[derive(Debug)]
struct PacketTimes<'a> {
    file: &'a PacketFile,
}

impl Times for PacketTimes<'_> {
    fn len(&self) -> usize {
        self.file.records()
    }

    fn at(&self, i: usize) -> Timestamp {
        let (p, at) = self.file.record(i);
        self.file.times_of(&p[at..at + self.file.stride()])[0]
    }

    fn lower_bound(&self, t: Timestamp) -> usize {
        // first the packet, by its first record's time, then the record within it
        let (mut lo, mut hi) = (0, self.file.index.len());
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.at(self.file.first[mid]) < t {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let k = lo.saturating_sub(1);
        if k >= self.file.index.len() {
            return self.len();
        }
        let times = self.file.times_of(&self.file.packet(k));
        self.file.first[k] + times.partition_point(|&x| x < t)
    }
}

impl Channels for PacketFile {
    fn describe(&self) -> &RecordingInfo {
        &self.info
    }

    fn dispatch<O: ChannelOp>(&self, channel: usize, op: O) -> O::Out {
        self.dispatch_held(channel, &Held::default(), op)
    }

    fn dispatch_held<O: ChannelOp>(&self, channel: usize, held: &Held, op: O) -> O::Out {
        struct WithTimes<'a, O> {
            op: O,
            file: &'a PacketFile,
            dtype: DType,
            offset: usize,
            held: &'a Held,
        }
        impl<O: ChannelOp> TimesOp for WithTimes<'_, O> {
            type Out = O::Out;
            fn run<T: Times + ?Sized>(self, times: &T) -> O::Out {
                fn run<V: Pod, O: ChannelOp, T: Times + ?Sized>(w: WithTimes<O>, times: &T) -> O::Out {
                    let base = PacketField::<V> { file: w.file, offset: w.offset, _v: std::marker::PhantomData };
                    w.op.run(times, &Chunked { base: &base, held: w.held })
                }
                match self.dtype {
                    DType::I8 => run::<i8, _, _>(self, times),
                    DType::I16 => run::<i16, _, _>(self, times),
                    DType::I32 => run::<i32, _, _>(self, times),
                    DType::I64 => run::<i64, _, _>(self, times),
                    DType::U8 => run::<u8, _, _>(self, times),
                    DType::U16 => run::<u16, _, _>(self, times),
                    DType::U32 => run::<u32, _, _>(self, times),
                    DType::U64 => run::<u64, _, _>(self, times),
                    DType::F32 => run::<f32, _, _>(self, times),
                    DType::F64 => run::<f64, _, _>(self, times),
                }
            }
        }
        let f = &self.desc.fields[self.channels[channel]];
        self.dispatch_times(WithTimes { op, file: self, dtype: f.dtype, offset: f.offset as usize, held })
    }

    fn chunked(&self, _channel: usize) -> bool {
        true
    }

    /// Each chunk's packets decoded once (chunks on several threads), then every channel gathered from them.
    fn decode_chunks(&self, channels: &[usize], chunks: std::ops::Range<usize>) -> Vec<Vec<Chunk>> {
        let (stride, big, n) = (self.stride(), self.big(), self.records());
        let per_chunk: Vec<Vec<Chunk>> = chunks
            .into_par_iter()
            .map(|k| {
                let (a, b) = ((k * CHUNK_SAMPLES).min(n), ((k + 1) * CHUNK_SAMPLES).min(n));
                let recs = self.read_records(a, b);
                let len = recs.len() / stride;
                channels
                    .iter()
                    .map(|&c| {
                        let f = &self.desc.fields[self.channels[c]];
                        let mut words = vec![0u64; (len * f.dtype.size() as usize).div_ceil(8)];
                        gather_tile(f.dtype, &recs, stride, f.offset as usize, big, &mut words, 0);
                        Chunk::new(words, len)
                    })
                    .collect()
            })
            .collect();
        let mut out: Vec<Vec<Chunk>> = channels.iter().map(|_| Vec::with_capacity(per_chunk.len())).collect();
        for row in per_chunk {
            for (o, c) in out.iter_mut().zip(row) {
                o.push(c);
            }
        }
        out
    }

    fn dispatch_times<O: TimesOp>(&self, op: O) -> O::Out {
        match &self.desc.time {
            TimeSpec::Field { .. } => {
                let times = PacketTimes { file: self };
                match self.time_index.get() {
                    Some(index) => op.run(&Indexed { times: &times, index }),
                    None => op.run(&times),
                }
            }
            _ => op.run(&self.rate_times()),
        }
    }

    fn set_time_index(&self, index: TimeIndex) {
        if matches!(self.desc.time, TimeSpec::Field { .. }) && index.0.len() == self.records().div_ceil(CHUNK_SAMPLES) {
            let _ = self.time_index.set(index);
        }
    }

    /// The block's packets decoded once (in parallel), then each channel gathered and summarized on the slice path.
    fn summarize_all(&self, start: usize, end: usize) -> Vec<Vec<Summary>> {
        let (stride, big) = (self.stride(), self.big());
        let recs: Vec<u8> = (start..end)
            .step_by(CHUNK_SAMPLES)
            .collect::<Vec<_>>()
            .into_par_iter()
            .map(|a| self.read_records(a, (a + CHUNK_SAMPLES).min(end)))
            .collect::<Vec<_>>()
            .concat();
        struct Op<'a> {
            me: &'a PacketFile,
            recs: &'a [u8],
            start: usize,
            stride: usize,
            big: bool,
        }
        impl TimesOp for Op<'_> {
            type Out = Vec<Vec<Summary>>;
            fn run<T: Times + ?Sized>(self, times: &T) -> Vec<Vec<Summary>> {
                fn one<V: Pod, T: Times + ?Sized>(op: &Op, times: &T, offset: usize, dtype: DType) -> Vec<Summary> {
                    let n = op.recs.len() / op.stride;
                    let mut words = vec![0u64; (n * V::SIZE).div_ceil(8)];
                    gather_tile(dtype, op.recs, op.stride, offset, op.big, &mut words, 0);
                    let data = &bytemuck::cast_slice::<u64, V>(&words)[..n];
                    pyramid::summarize(times, &Shifted { offset: op.start, data }, op.start, op.start + n)
                }
                self.me
                    .channels
                    .par_iter()
                    .map(|&fi| {
                        let f = &self.me.desc.fields[fi];
                        let o = f.offset as usize;
                        match f.dtype {
                            DType::I8 => one::<i8, T>(&self, times, o, f.dtype),
                            DType::I16 => one::<i16, T>(&self, times, o, f.dtype),
                            DType::I32 => one::<i32, T>(&self, times, o, f.dtype),
                            DType::I64 => one::<i64, T>(&self, times, o, f.dtype),
                            DType::U8 => one::<u8, T>(&self, times, o, f.dtype),
                            DType::U16 => one::<u16, T>(&self, times, o, f.dtype),
                            DType::U32 => one::<u32, T>(&self, times, o, f.dtype),
                            DType::U64 => one::<u64, T>(&self, times, o, f.dtype),
                            DType::F32 => one::<f32, T>(&self, times, o, f.dtype),
                            DType::F64 => one::<f64, T>(&self, times, o, f.dtype),
                        }
                    })
                    .collect()
            }
        }
        match &self.desc.time {
            // the block's own times, so the summaries' first and last times need no packet lookups
            TimeSpec::Field { .. } => {
                let local = self.times_of(&recs);
                let times = ShiftedTimes { offset: start, data: &local, len: self.records() };
                Op { me: self, recs: &recs, start, stride, big }.run(&times)
            }
            _ => Op { me: self, recs: &recs, start, stride, big }.run(&self.rate_times()),
        }
    }

    fn transform(&self, channel: usize) -> (f64, f64) {
        let f = &self.desc.fields[self.channels[channel]];
        (f.scale, f.value_offset)
    }

    fn layout_id(&self) -> Vec<u8> {
        let mut id = format!("packets\n{}\n", self.info.format).into_bytes();
        if let Some(q) = &self.part {
            id.extend(format!("part {}\n", q.id).into_bytes());
        }
        id.extend(self.desc.to_toml().into_bytes());
        id
    }
}

/// Times `offset..offset + data.len()` of a recording of `len` samples; only those may be asked for.
#[derive(Debug)]
struct ShiftedTimes<'a> {
    offset: usize,
    data: &'a [Timestamp],
    len: usize,
}

impl Times for ShiftedTimes<'_> {
    fn len(&self) -> usize {
        self.len
    }

    fn at(&self, i: usize) -> Timestamp {
        self.data[i - self.offset]
    }

    fn lower_bound(&self, t: Timestamp) -> usize {
        self.offset + self.data.partition_point(|&x| x < t)
    }
}

fn read_index(dir: &Path) -> Option<(IndexManifest, Vec<Entry>)> {
    let m: IndexManifest = serde_json::from_slice(&fs::read(dir.join("manifest.json")).ok()?).ok()?;
    let raw = fs::read(dir.join("index.bin")).ok()?;
    if m.version != INDEX_VERSION || raw.len() != m.packets * 16 {
        return None;
    }
    let index = raw
        .as_chunks::<16>()
        .0
        .iter()
        .map(|c| Entry {
            offset: u64::from_le_bytes(c[..8].try_into().unwrap()),
            stored: u32::from_le_bytes(c[8..12].try_into().unwrap()),
            records: u32::from_le_bytes(c[12..].try_into().unwrap()),
        })
        .collect();
    Some((m, index))
}

/// Writes the index into a temporary folder beside `dir`, then renames it into place.
fn write_index(dir: &Path, m: &IndexManifest, index: &[Entry]) -> Result<()> {
    let parent = dir.parent().expect("cache dirs have a parent");
    fs::create_dir_all(parent).map_err(io_err(parent))?;
    let tmp = parent.join(format!(".{}.tmp-{}", dir.file_name().unwrap().to_string_lossy(), std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).map_err(io_err(&tmp))?;
    let raw: Vec<u8> = index
        .iter()
        .flat_map(|e| [&e.offset.to_le_bytes()[..], &e.stored.to_le_bytes(), &e.records.to_le_bytes()].concat())
        .collect();
    let written = fs::write(tmp.join("index.bin"), raw)
        .and_then(|_| fs::write(tmp.join("manifest.json"), serde_json::to_vec_pretty(m).expect("serializes")));
    if let Err(e) = written {
        let _ = fs::remove_dir_all(&tmp);
        return Err(io_err(&tmp)(e));
    }
    if fs::rename(&tmp, dir).is_err() {
        // another process built it first
        let _ = fs::remove_dir_all(&tmp);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
