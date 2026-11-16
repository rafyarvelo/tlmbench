//! The pure part of the packet reader: framing (sync word, length field), checksums, payload decompression and a
//! packet's records as read. No files, threads or cache here, so the app's packet files (`packets.rs`) and the
//! browser-only page (`crates/web`, which includes this file as source) frame a file by the same rules.
//! Pure Rust codecs only for the page: lz4_flex, flate2 on miniz_oxide, and zstd (the page's build patches it to the
//! `ruzstd` shim in `crates/web-parquet/zstd-shim`).

use std::borrow::Cow;
use std::io::Read;

use crate::descriptor::{Bits, Checksum, Compression, Counts, DType, Descriptor, Endian, Packets, Part};
use crate::error::Result;
use crate::strided::decode;

/// The most a packet's payload may hold, stored or decompressed.
pub(crate) const MAX_PAYLOAD: u64 = 1 << 30;

pub(crate) fn packets_of(desc: &Descriptor) -> &Packets {
    desc.packets.as_ref().expect("a packet file's descriptor has [packets]")
}

/// An integer field of `dtype` at the start of `b`, as i64.
pub(crate) fn read_int(b: &[u8], dtype: DType, big: bool) -> i64 {
    read_bits(b, dtype, None, big)
}

/// [`read_int`] of the run of bits a bitfield takes.
pub(crate) fn read_bits(b: &[u8], dtype: DType, bits: Option<Bits>, big: bool) -> i64 {
    macro_rules! read {
        ($t:ty) => {{
            let v: $t = decode(big, bits, b);
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
pub(crate) fn read_uint(b: &[u8], size: usize, big: bool) -> u64 {
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

/// One packet's bytes around a `stored` (already compressed) payload: `header` (`header_size` bytes, the caller's
/// fields filled in) gets the sync word and the length, and the checksum follows. `packets::encode_packet` compresses
/// first; the browser's tests frame plain payloads with it.
pub(crate) fn frame_packet(p: &Packets, big: bool, header: &[u8], stored: &[u8]) -> Result<Vec<u8>> {
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
    out.extend_from_slice(stored);
    let c = checksum(p.checksum, &out);
    for k in 0..cks {
        let shift = 8 * if big { cks - 1 - k } else { k };
        out.push((c >> shift) as u8);
    }
    Ok(out)
}

/// A packet found by the framing scan, before its payload is decompressed.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Framed {
    pub(crate) offset: u64,
    pub(crate) stored: u64,
    /// Whether its checksum matched (always, unless the scan was asked to keep packets that fail it).
    pub(crate) ok: bool,
}

/// The framing scan's result.
#[derive(Debug, Default)]
pub(crate) struct Framing {
    pub(crate) packets: Vec<Framed>,
    pub(crate) bad: usize,
    /// Bytes of good packets of other types.
    pub(crate) other_bytes: u64,
}

/// Finds the packets of `part` (every packet without one) in `data` from `start`: their offsets and stored payload
/// lengths, and how many were skipped. Without a length field a packet holds one record, of the type's size.
pub(crate) fn frame(
    data: &[u8],
    start: usize,
    p: &Packets,
    record_size: u64,
    part: Option<&Part>,
    sync: &[u8],
    big: bool,
) -> Framing {
    frame_checked(data, start, p, record_size, part, sync, big, false)
}

/// [`frame`], optionally keeping the packets whose checksum fails (marked `ok: false`): the descriptor form's
/// preview shows them.
#[allow(clippy::too_many_arguments)]
pub(crate) fn frame_checked(
    data: &[u8],
    start: usize,
    p: &Packets,
    record_size: u64,
    part: Option<&Part>,
    sync: &[u8],
    big: bool,
    keep_bad: bool,
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
        let mut sum_ok = true;
        let good = match end {
            Some(e) if e <= data.len() => {
                let body = pos + hs + stored as usize;
                sum_ok = cks == 0 || read_uint(&data[body..], cks, big) == checksum(p.checksum, &data[pos..body]);
                sum_ok || keep_bad
            }
            Some(_) if finder.is_none() => break, // runs past the end: a partial last packet
            _ => false,
        };
        match (good, end) {
            (true, Some(e)) => {
                match (part, known) {
                    (Some(q), Some(_)) if id == Some(q.id) => {
                        out.packets.push(Framed { offset: pos as u64, stored: stored as u64, ok: sum_ok })
                    }
                    (Some(_), Some(_)) => out.other_bytes += (e - pos) as u64,
                    (Some(_), None) => {
                        out.bad += usize::from(counts);
                        out.other_bytes += (e - pos) as u64;
                    }
                    (None, _) => out.packets.push(Framed { offset: pos as u64, stored: stored as u64, ok: sum_ok }),
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

/// The records of the packet at `offset` of `data` with `stored` payload bytes, as read: the packet header, then
/// the record, for each. `None` when the payload doesn't decompress to whole records.
pub(crate) fn packet_records(data: &[u8], p: &Packets, record_size: u64, offset: u64, stored: u64) -> Option<Vec<u8>> {
    let (hs, at) = (p.header_size as usize, offset as usize);
    let header = &data[at..at + hs];
    let payload = decompress(p.compression, &data[at + hs..at + hs + stored as usize])?;
    let rs = record_size as usize;
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

/// What the descriptor form's preview reads from the first bytes of a file laid out as packets: each packet's
/// start, its records as read (header then record), whether its checksum matched, and how many bytes came before
/// the first packet.
pub(crate) struct HeadPackets {
    pub packets: Vec<(usize, Vec<u8>, bool)>,
    pub skipped: usize,
    /// How many bytes the first packet takes in all.
    pub first_len: usize,
}

/// The first `max` packets in `head` that decode (a descriptor without `[[packet_types]]`).
pub(crate) fn head_packets(head: &[u8], d: &Descriptor, max: usize) -> Result<HeadPackets> {
    let p = packets_of(d);
    let sync = p.sync_bytes()?;
    let big = d.endian == Endian::Big;
    let framing = frame_checked(head, d.header_size as usize, p, d.record_size, None, &sync, big, true);
    let mut out = HeadPackets { packets: Vec::new(), skipped: 0, first_len: 0 };
    for f in framing.packets.iter().take(max) {
        let Some(recs) = packet_records(head, p, d.record_size, f.offset, f.stored) else { continue };
        if out.packets.is_empty() {
            out.skipped = f.offset as usize - d.header_size as usize;
            out.first_len = (p.header_size + p.checksum.size() + f.stored) as usize;
        }
        out.packets.push((f.offset as usize, recs, f.ok));
    }
    Ok(out)
}
