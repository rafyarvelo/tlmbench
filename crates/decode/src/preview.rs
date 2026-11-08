//! What the descriptor form shows beside a layout: the first records of a file decoded through a descriptor, so the
//! person sees whether the byte order, offsets, scales and framing are right before saving anything.
//!
//! The preview reads only the first bytes of the file (64 KB) and decodes them with the same code that opens the
//! file, so a problem is worded as the decoder words it and the values are the ones the app will plot. The UI shows
//! what comes back and computes nothing over samples.

use serde::Serialize;

use crate::descriptor::{Bits, DType, Descriptor, Endian, Field, TimeSpec, counter_range};
use crate::packets;
use crate::strided::{decode, wrapped};

/// How much of a file the form reads.
pub const HEAD_BYTES: usize = 64 << 10;
/// Decoded rows shown.
pub const ROWS: usize = 8;
/// Bytes of the file shown as hex, from the first record.
pub const HEX_BYTES: usize = 64;

/// One decoded record.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Row {
    /// Where the record's packet (or the record itself) starts in the file.
    pub offset: usize,
    /// Seconds from the time base, or the record number without one; `None` when the time field can't be read.
    pub time: Option<f64>,
    /// Every field's value (raw × scale + offset), in the descriptor's order; `None` where it isn't a number.
    pub values: Vec<Option<f64>>,
    /// Packets with a checksum: whether it matched.
    pub ok: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Preview {
    /// The descriptor as the TOML the decoders read.
    pub toml: String,
    /// Why the descriptor can't decode anything yet, as the decoder says it.
    pub problem: Option<String>,
    /// Why no rows came out of the bytes (the framing doesn't fit them, or the layout can't be previewed).
    pub error: Option<String>,
    pub rows: Vec<Row>,
    /// Bytes from one record (or packet) to the next, as laid out.
    pub stride: usize,
    /// Where the first record or packet starts.
    pub start: usize,
    /// Bytes skipped before the first packet's sync word.
    pub skipped: usize,
    /// Rows whose packet checksum failed.
    pub bad_checks: usize,
    /// Whether the descriptor has a checksum to check.
    pub checked: bool,
    /// The file's bytes from `hex_at` (the first record), up to [`HEX_BYTES`].
    pub hex_at: usize,
    pub hex: Vec<u8>,
}

/// One field's value in `rec` (a record as read: the packet header, if any, then the record), scaled.
fn value(rec: &[u8], f: &Field, big: bool) -> Option<f64> {
    let b = rec.get(f.offset as usize..)?;
    if b.len() < f.dtype.size() as usize {
        return None;
    }
    macro_rules! read {
        ($t:ty) => {
            decode::<$t>(big, f.bits, b) as f64
        };
    }
    let raw = match f.dtype {
        DType::I8 => read!(i8),
        DType::I16 => read!(i16),
        DType::I32 => read!(i32),
        DType::I64 => read!(i64),
        DType::U8 => read!(u8),
        DType::U16 => read!(u16),
        DType::U32 => read!(u32),
        DType::U64 => read!(u64),
        DType::F32 => decode::<f32>(big, None::<Bits>, b) as f64,
        DType::F64 => decode::<f64>(big, None::<Bits>, b),
    };
    Some(raw * f.scale + f.value_offset)
}

/// The integer a time field holds in `rec`.
fn raw_time(rec: &[u8], f: &Field, big: bool) -> Option<i64> {
    value(rec, &Field { scale: 1.0, value_offset: 0.0, ..f.clone() }, big).map(|v| v as i64)
}

/// The time of each of `recs`, in order (a counter is unwrapped across them).
fn times(d: &Descriptor, recs: &[&[u8]]) -> Vec<Option<f64>> {
    let big = d.endian == Endian::Big;
    let field = |name: &str| d.fields.iter().find(|f| f.name == name);
    match &d.time {
        TimeSpec::Index => (0..recs.len()).map(|k| Some(k as f64)).collect(),
        TimeSpec::Rate { rate_hz, t0_ns } => {
            (0..recs.len()).map(|k| Some(*t0_ns as f64 / 1e9 + k as f64 / rate_hz)).collect()
        }
        TimeSpec::Field { field: name, unit, t0_ns } => {
            let f = field(name);
            recs.iter()
                .map(|r| {
                    let raw = raw_time(r, f?, big)?;
                    Some((*t0_ns as f64 + raw as f64 * unit.ns() as f64) / 1e9)
                })
                .collect()
        }
        TimeSpec::Counter { field: name, clock_hz, wraps_at, t0_ns } => {
            let Some(f) = field(name) else { return vec![None; recs.len()] };
            let range = counter_range(f, *wraps_at).unwrap_or(i64::MAX);
            let (mut wraps, mut prev) = (0i64, None::<i64>);
            recs.iter()
                .map(|r| {
                    let raw = raw_time(r, f, big)?;
                    if let Some(p) = prev
                        && wrapped(p, raw, range) == Ok(true)
                    {
                        wraps += 1;
                    }
                    prev = Some(raw);
                    let ticks = raw as f64 + wraps as f64 * range as f64;
                    Some(*t0_ns as f64 / 1e9 + ticks / clock_hz)
                })
                .collect()
        }
    }
}

/// Decodes the first records of `head` (the start of a file) through `d`.
pub fn preview(head: &[u8], d: &Descriptor) -> Preview {
    let mut out = Preview { toml: d.to_toml(), ..Preview::default() };
    if let Err(e) = d.validate() {
        out.problem = Some(match e {
            crate::Error::Descriptor(msg) => msg,
            other => other.to_string(),
        });
        return out;
    }
    if !d.packet_types.is_empty() {
        out.error = Some(
            "This descriptor has packet types: each opens as its own recording, so open the file to see them.".into(),
        );
        return out;
    }
    let big = d.endian == Endian::Big;
    // each record as read, with where its packet or itself starts
    let mut recs: Vec<(usize, Vec<u8>, Option<bool>)> = Vec::new();
    if let Some(p) = &d.packets {
        let found = match packets::head_packets(head, d, ROWS) {
            Ok(f) => f,
            Err(e) => {
                out.error = Some(e.to_string());
                return out;
            }
        };
        let hs = p.header_size as usize;
        let rs = d.record_size as usize;
        let checked = p.checksum.size() > 0;
        'all: for (at, bytes, ok) in &found.packets {
            for r in bytes.chunks_exact(hs + rs) {
                if recs.len() == ROWS {
                    break 'all;
                }
                recs.push((*at, r.to_vec(), checked.then_some(*ok)));
            }
        }
        out.stride = found.first_len;
        out.skipped = found.skipped;
        out.checked = checked;
        out.start = found.packets.first().map_or(d.header_size as usize, |f| f.0);
    } else {
        let (hs, rs) = (d.header_size as usize, d.record_size as usize);
        out.stride = rs;
        out.start = hs;
        for k in 0..ROWS {
            let at = hs + k * rs;
            let Some(r) = head.get(at..at + rs) else { break };
            recs.push((at, r.to_vec(), None));
        }
    }
    if recs.is_empty() {
        out.error = Some("No whole record found in the first bytes: check the framing.".into());
        return out;
    }
    let refs: Vec<&[u8]> = recs.iter().map(|r| r.1.as_slice()).collect();
    let t = times(d, &refs);
    out.rows = recs
        .iter()
        .zip(t)
        .map(|((offset, r, ok), time)| Row {
            offset: *offset,
            time,
            values: d.fields.iter().map(|f| value(r, f, big).filter(|v| v.is_finite())).collect(),
            ok: *ok,
        })
        .collect();
    out.bad_checks = out.rows.iter().filter(|r| r.ok == Some(false)).count();
    out.hex_at = out.rows[0].offset;
    out.hex = head.get(out.hex_at..).unwrap_or_default().iter().take(HEX_BYTES).copied().collect();
    out
}

/// Whether a file's first bytes look binary rather than text: a zero byte, or bytes that are not UTF-8 (a cut in
/// the middle of a character at the end doesn't count).
pub fn looks_binary(head: &[u8]) -> bool {
    if head.contains(&0) {
        return true;
    }
    match std::str::from_utf8(head) {
        Ok(_) => false,
        Err(e) => e.error_len().is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packets::encode_packet;

    /// The prototype's ESC logger: 24-byte frames, sync A55A, CRC-16, no length field, little-endian.
    const ESC: &str = r#"schema = 1
name = "ESC telemetry"
endian = "little"
record_size = 20
[time]
source = "field"
field = "timestamp_us"
unit = "us"
[[fields]]
name = "seq"
type = "u16"
offset = 2
[[fields]]
name = "timestamp_us"
type = "u32"
offset = 4
[[fields]]
name = "battery_v"
type = "u16"
offset = 14
unit = "V"
scale = 0.001
[packets]
sync = "A55A"
header_size = 2
checksum = "crc16-ccitt"
"#;

    fn esc_bytes(n: u32) -> Vec<u8> {
        let d = Descriptor::parse(ESC).unwrap();
        let p = d.packets.as_ref().unwrap();
        let mut out = Vec::new();
        for k in 0..n {
            let mut rec = vec![0u8; 20];
            rec[0..2].copy_from_slice(&(k as u16).to_le_bytes());
            rec[2..6].copy_from_slice(&(120_000_000 + k * 10_000).to_le_bytes());
            rec[12..14].copy_from_slice(&(24_000 + k as u16).to_le_bytes());
            out.extend(encode_packet(p, false, &[0, 0], &rec).unwrap());
        }
        out
    }

    #[test]
    fn packets_decode_with_checksums_and_scaled_values() {
        let d = Descriptor::parse(ESC).unwrap();
        let data = esc_bytes(20);
        let pv = preview(&data, &d);
        assert_eq!((pv.problem.as_deref(), pv.error.as_deref()), (None, None));
        assert_eq!((pv.rows.len(), pv.stride, pv.start, pv.skipped, pv.bad_checks, pv.checked), (8, 24, 0, 0, 0, true));
        let r = &pv.rows[1];
        assert_eq!((r.offset, r.ok), (24, Some(true)));
        assert_eq!(r.time, Some(120.01));
        assert_eq!(r.values[2], Some(24.001));
        assert_eq!(pv.hex.len(), HEX_BYTES);
        assert_eq!(&pv.hex[..4], &[0xA5, 0x5A, 0, 0]);
        assert!(pv.toml.contains("sync = \"A55A\""));
        // the wrong byte order garbles the values and fails every checksum, the rows stay
        let mut big = d.clone();
        big.endian = Endian::Big;
        let bad = preview(&data, &big);
        assert_eq!(bad.bad_checks, 8);
        assert!(bad.rows.iter().all(|r| r.ok == Some(false)));
        assert_ne!(bad.rows[0].values[2], pv.rows[0].values[2]);
    }

    #[test]
    fn a_problem_is_worded_as_the_decoder_words_it() {
        let mut d = Descriptor::parse(ESC).unwrap();
        d.fields[2].offset = 21;
        let pv = preview(&esc_bytes(3), &d);
        assert_eq!(
            pv.problem.as_deref(),
            Some("field \"battery_v\" (u16 at offset 21) runs past the 22-byte packet header and record")
        );
        assert!(pv.rows.is_empty());
        let mut d = Descriptor::parse(ESC).unwrap();
        d.packets.as_mut().unwrap().sync = Some("XYZ".into());
        assert!(preview(&[], &d).problem.unwrap().contains("hex"));
    }

    #[test]
    fn fixed_records_after_a_file_header_with_each_time_base() {
        let mut d = Descriptor::parse(
            "schema = 1\nendian = \"big\"\nheader_size = 4\nrecord_size = 6\n[time]\nsource = \"counter\"\nfield = \"c\"\nclock_hz = 100.0\nwraps_at = 256\n\
             [[fields]]\nname = \"c\"\ntype = \"u8\"\noffset = 0\n[[fields]]\nname = \"flags\"\ntype = \"u8\"\noffset = 1\nbits = { shift = 2, width = 3 }\n\
             [[fields]]\nname = \"x\"\ntype = \"i32\"\noffset = 2\nscale = 0.5\n",
        )
        .unwrap();
        let mut data = vec![9u8; 4];
        for k in 0..10u32 {
            data.push((250 + k * 2) as u8); // wraps after the third record
            data.push(0b0001_0100);
            data.extend((k as i32 - 5).to_be_bytes());
        }
        let pv = preview(&data, &d);
        assert_eq!((pv.rows.len(), pv.stride, pv.start), (8, 6, 4));
        // 250, 252, 254, 0 (wrapped: 256), 2 (258): seconds at 100 Hz
        assert_eq!(pv.rows[0].time, Some(2.5));
        assert_eq!(pv.rows[3].time, Some(2.56));
        assert_eq!(pv.rows[4].time, Some(2.58));
        assert_eq!(pv.rows[0].values[1], Some(5.0)); // bits 2 to 4 of 0b0001_0100
        assert_eq!(pv.rows[2].values[2], Some(-1.5)); // (2 - 5) × 0.5
        d.time = TimeSpec::Rate { rate_hz: 10.0, t0_ns: 0 };
        assert_eq!(preview(&data, &d).rows[2].time, Some(0.2));
        d.time = TimeSpec::Index;
        assert_eq!(preview(&data, &d).rows[2].time, Some(2.0));
        // too short for a record: the framing is wrong
        assert!(preview(&data[..8], &d).error.unwrap().contains("No whole record"));
    }

    #[test]
    fn text_and_zeros_are_told_apart_from_binary() {
        assert!(!looks_binary(b"time,a,b\n0,1,2\n"));
        assert!(!looks_binary("héllo".as_bytes()));
        assert!(!looks_binary(&"é".as_bytes()[..1]));
        assert!(looks_binary(&[0xA5, 0x5A, 0, 0, 1, 2]));
        assert!(looks_binary(&[0xFF, 0xFE, 0x41]));
    }
}
