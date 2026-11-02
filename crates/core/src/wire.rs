//! The binary reply to a viewport query: traces as `f32` arrays the UI views in place (no parsing), the same over
//! Tauri IPC today and over the daemon's socket and WebSocket later.
//!
//! Little-endian, every array 8-byte aligned so `Float32Array` views need no copy:
//!
//! ```text
//! header (40 B):    magic "TLMV" · u16 version · u16 flags · u32 generation · u32 trace count
//!                   · i64 origin_ns · f64 x_scale_ns · i64 built_until_ns
//! per trace (24 B): u32 channel · u32 n · u32 kind · u32 reduce_us · f32 y_min · f32 y_max
//!                   · f32 x[n] · f32 y[n]   (8n bytes, so every trace stays 8-byte aligned)
//! ```
//!
//! Header flags: bit 0, the pyramid is still being built and only samples up to `built_until_ns` are summarized
//! (the UI hatches the rest). Trace kind bits: bit 0 M4 (else raw samples), bit 1 coarse (drawn from whole
//! summaries, replaced by an exact reply), bit 2 answered from the pyramid.
//!
//! A sample's time is `origin_ns + x * x_scale_ns`. With the origin at the viewport's start and the scale at one
//! bucket, x runs over about `0..width_px`, where `f32` is exact enough at any zoom; `f32` can't hold nanoseconds
//! since the epoch. `y_min`/`y_max` cover the samples inside the viewport (NaN when there are none), for autoscale.
//! A NaN in `y` is a gap. `reduce_us` is how long the core took to reduce the trace (the UI's clocks are too coarse to
//! time it).
//!
//! A batch (the reply to one `view` request: every pane of a tab and the overview, under one generation) frames
//! one such message per entry, each with its own viewport and width:
//!
//! ```text
//! header (16 B):    magic "TLMB" · u16 version · u16 0 · u32 generation · u32 entry count
//! per entry (16 B): u32 id · u32 0 · u32 offset · u32 length   (the entry's TLMV message, from the batch's start)
//! messages:         back to back, each 8-byte aligned
//! ```

use crate::{Timestamp, Trace, TraceKind, Viewport};

pub const MAGIC: &[u8; 4] = b"TLMV";
pub const VERSION: u16 = 2;
pub const HEADER_BYTES: usize = 40;
pub const TRACE_HEADER_BYTES: usize = 24;

/// One channel's trace in a reply.
#[derive(Clone, Debug, PartialEq)]
pub struct WireTrace {
    pub channel: u32,
    pub kind: TraceKind,
    pub coarse: bool,
    pub from_pyramid: bool,
    pub reduce_us: u32,
    pub y_min: f32,
    pub y_max: f32,
    pub x: Vec<f32>,
    pub y: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Reply {
    pub generation: u32,
    pub origin_ns: Timestamp,
    pub x_scale_ns: f64,
    /// Samples after this time aren't summarized yet (None: the pyramid is built, or not started).
    pub built_until: Option<Timestamp>,
    pub traces: Vec<WireTrace>,
}

/// One channel's trace to encode.
#[derive(Clone, Copy, Debug)]
pub struct Out<'a> {
    pub channel: u32,
    pub trace: &'a Trace,
    /// Time the core spent reducing it, in microseconds.
    pub reduce_us: u32,
    pub coarse: bool,
    pub from_pyramid: bool,
}

/// Encodes traces reduced for `vp`. `built_until`: see [`Reply::built_until`].
pub fn encode(generation: u32, vp: Viewport, built_until: Option<Timestamp>, traces: &[Out]) -> Vec<u8> {
    let origin = vp.t0;
    let scale = vp.bucket_ns() as f64;
    let size = HEADER_BYTES + traces.iter().map(|o| TRACE_HEADER_BYTES + 8 * o.trace.len()).sum::<usize>();
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&u16::from(built_until.is_some()).to_le_bytes());
    out.extend_from_slice(&generation.to_le_bytes());
    out.extend_from_slice(&(traces.len() as u32).to_le_bytes());
    out.extend_from_slice(&origin.to_le_bytes());
    out.extend_from_slice(&scale.to_le_bytes());
    out.extend_from_slice(&built_until.unwrap_or(i64::MAX).to_le_bytes());
    for &Out { channel, trace: t, reduce_us, coarse, from_pyramid } in traces {
        let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
        for (&ts, &y) in t.t.iter().zip(&t.y) {
            if !y.is_nan() && (vp.t0..vp.t1).contains(&ts) {
                lo = lo.min(y);
                hi = hi.max(y);
            }
        }
        if lo > hi {
            (lo, hi) = (f32::NAN, f32::NAN);
        }
        out.extend_from_slice(&channel.to_le_bytes());
        out.extend_from_slice(&(t.len() as u32).to_le_bytes());
        let kind = u32::from(t.kind == TraceKind::M4) | u32::from(coarse) << 1 | u32::from(from_pyramid) << 2;
        out.extend_from_slice(&kind.to_le_bytes());
        out.extend_from_slice(&reduce_us.to_le_bytes());
        out.extend_from_slice(&lo.to_le_bytes());
        out.extend_from_slice(&hi.to_le_bytes());
        for &ts in &t.t {
            out.extend_from_slice(&(((ts as i128 - origin as i128) as f64 / scale) as f32).to_le_bytes());
        }
        for &y in &t.y {
            out.extend_from_slice(&y.to_le_bytes());
        }
    }
    debug_assert_eq!(out.len(), size);
    out
}

/// Decodes a reply (tests, and Rust clients of the daemon later).
pub fn decode(b: &[u8]) -> Option<Reply> {
    let u32_at = |o: usize| Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?));
    let f32_at = |o: usize| Some(f32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?));
    if b.get(..4)? != MAGIC || u16::from_le_bytes(b.get(4..6)?.try_into().ok()?) != VERSION {
        return None;
    }
    let generation = u32_at(8)?;
    let count = u32_at(12)? as usize;
    let origin_ns = i64::from_le_bytes(b.get(16..24)?.try_into().ok()?);
    let x_scale_ns = f64::from_le_bytes(b.get(24..32)?.try_into().ok()?);
    let building = u16::from_le_bytes(b.get(6..8)?.try_into().ok()?) & 1 == 1;
    let until = i64::from_le_bytes(b.get(32..40)?.try_into().ok()?);
    let mut traces = Vec::with_capacity(count.min(1024));
    let mut o = HEADER_BYTES;
    for _ in 0..count {
        let n = u32_at(o + 4)? as usize;
        let floats = |at: usize| (0..n).map(|i| f32_at(at + 4 * i)).collect::<Option<Vec<f32>>>();
        traces.push(WireTrace {
            channel: u32_at(o)?,
            kind: if u32_at(o + 8)? & 1 == 1 { TraceKind::M4 } else { TraceKind::Raw },
            coarse: u32_at(o + 8)? & 2 != 0,
            from_pyramid: u32_at(o + 8)? & 4 != 0,
            reduce_us: u32_at(o + 12)?,
            y_min: f32_at(o + 16)?,
            y_max: f32_at(o + 20)?,
            x: floats(o + TRACE_HEADER_BYTES)?,
            y: floats(o + TRACE_HEADER_BYTES + 4 * n)?,
        });
        o += TRACE_HEADER_BYTES + 8 * n;
    }
    let built_until = building.then_some(until);
    (o == b.len()).then_some(Reply { generation, origin_ns, x_scale_ns, built_until, traces })
}

pub const BATCH_MAGIC: &[u8; 4] = b"TLMB";
pub const BATCH_VERSION: u16 = 1;
pub const BATCH_HEADER_BYTES: usize = 16;
pub const BATCH_ENTRY_BYTES: usize = 16;

/// The replies of one batch, by entry id, in request order.
#[derive(Clone, Debug, PartialEq)]
pub struct Batch {
    pub generation: u32,
    pub entries: Vec<(u32, Reply)>,
}

/// Frames encoded replies (see [`encode`]) as one batch: `(id, message)` per entry.
pub fn encode_batch(generation: u32, entries: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let table = BATCH_HEADER_BYTES + BATCH_ENTRY_BYTES * entries.len();
    let mut out = Vec::with_capacity(table + entries.iter().map(|(_, m)| m.len().next_multiple_of(8)).sum::<usize>());
    out.extend_from_slice(BATCH_MAGIC);
    out.extend_from_slice(&BATCH_VERSION.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&generation.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    let mut at = table;
    for (id, m) in entries {
        for v in [*id, 0, at as u32, m.len() as u32] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        at += m.len().next_multiple_of(8);
    }
    for (_, m) in entries {
        out.extend_from_slice(m);
        out.resize(out.len().next_multiple_of(8), 0);
    }
    out
}

/// Decodes a batch and every reply in it.
pub fn decode_batch(b: &[u8]) -> Option<Batch> {
    let u32_at = |o: usize| Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?));
    if b.get(..4)? != BATCH_MAGIC || u16::from_le_bytes(b.get(4..6)?.try_into().ok()?) != BATCH_VERSION {
        return None;
    }
    let (generation, count) = (u32_at(8)?, u32_at(12)? as usize);
    let mut entries = Vec::with_capacity(count.min(1024));
    for k in 0..count {
        let e = BATCH_HEADER_BYTES + BATCH_ENTRY_BYTES * k;
        let (id, at, len) = (u32_at(e)?, u32_at(e + 8)? as usize, u32_at(e + 12)? as usize);
        if !at.is_multiple_of(8) {
            return None;
        }
        entries.push((id, decode(b.get(at..at.checked_add(len)?)?)?));
    }
    Some(Batch { generation, entries })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RateTimes, Times, m4};

    #[test]
    fn round_trip() {
        let n = 100_001;
        let times = RateTimes::from_rate(1_767_225_600_000_000_000, 10_000.0, n);
        let values: Vec<f32> = (0..n).map(|i| if i == 500 { f32::NAN } else { (i as f32 * 0.01).sin() }).collect();
        let vp = Viewport { t0: times.at(10), t1: times.at(90_000), width_px: 777 };
        let a = m4(&times, &values, vp);
        let raw_vp = Viewport { t0: times.at(10), t1: times.at(30), width_px: 777 };
        let b = m4(&times, &values, raw_vp);
        let outs = [
            Out { channel: 3, trace: &a, reduce_us: 1234, coarse: true, from_pyramid: true },
            Out { channel: 9, trace: &b, reduce_us: 5, coarse: false, from_pyramid: false },
        ];
        let bytes = encode(42, vp, Some(vp.t0 + 99), &outs);
        assert_eq!(bytes.len() % 8, 0);
        let r = decode(&bytes).unwrap();
        assert_eq!((r.generation, r.origin_ns, r.x_scale_ns), (42, vp.t0, vp.bucket_ns() as f64));
        assert_eq!(r.built_until, Some(vp.t0 + 99));
        assert!(r.traces[0].coarse && r.traces[0].from_pyramid && !r.traces[1].coarse && !r.traces[1].from_pyramid);
        assert_eq!(r.traces.len(), 2);
        let w = &r.traces[0];
        assert_eq!((w.channel, w.kind, w.reduce_us, w.x.len()), (3, TraceKind::M4, 1234, a.len()));
        // times come back to within a bucket's millionth
        for (x, &t) in w.x.iter().zip(&a.t) {
            let back = r.origin_ns as f64 + *x as f64 * r.x_scale_ns;
            assert!((back - t as f64).abs() <= r.x_scale_ns * 1e-3, "{back} vs {t}");
        }
        assert!(w.y.iter().zip(&a.y).all(|(p, q)| p == q || (p.is_nan() && q.is_nan())));
        assert!(w.y_min >= -1.0 && w.y_max <= 1.0 && w.y_min < w.y_max);
        assert_eq!(r.traces[1].kind, TraceKind::Raw);
        assert_eq!(r.traces[1].y, b.y);
    }

    #[test]
    fn batch_round_trip() {
        let times = RateTimes::from_rate(1_767_225_600_000_000_000, 1000.0, 50_000);
        let values: Vec<f32> = (0..50_000).map(|i| (i as f32 * 0.02).cos()).collect();
        let (a, b) = (
            Viewport { t0: times.at(100), t1: times.at(40_000), width_px: 640 },
            Viewport { t0: times.at(100), t1: times.at(40_000), width_px: 311 },
        );
        let whole = Viewport { t0: times.at(0), t1: times.at(49_999) + 1, width_px: 1200 };
        let (ta, tb, tw) = (m4(&times, &values, a), m4(&times, &values, b), m4(&times, &values, whole));
        let one = |vp, t: &Trace, ch| {
            let out = Out { channel: ch, trace: t, reduce_us: 7, coarse: false, from_pyramid: true };
            encode(9, vp, None, &[out])
        };
        // the second pane has three channels: odd lengths keep every message 8-byte aligned anyway
        let three = encode(
            9,
            b,
            Some(b.t0 + 5),
            &[0, 1, 2].map(|ch| Out { channel: ch, trace: &tb, reduce_us: 1, coarse: true, from_pyramid: false }),
        );
        let parts = vec![(1, one(a, &ta, 4)), (2, three), (u32::MAX, one(whole, &tw, 4))];
        let bytes = encode_batch(9, &parts);
        assert_eq!(bytes.len() % 8, 0);
        let got = decode_batch(&bytes).unwrap();
        assert_eq!(got.generation, 9);
        let ids: Vec<u32> = got.entries.iter().map(|e| e.0).collect();
        assert_eq!(ids, [1, 2, u32::MAX]);
        for ((_, r), (_, m)) in got.entries.iter().zip(&parts) {
            assert_eq!(Some(r), decode(m).as_ref());
        }
        assert_eq!(got.entries[1].1.traces.len(), 3);
        assert_eq!(got.entries[1].1.x_scale_ns, b.bucket_ns() as f64);
        assert_eq!(got.entries[2].1.traces[0].y.len(), tw.len());
        assert_eq!(decode_batch(&encode_batch(3, &[])), Some(Batch { generation: 3, entries: vec![] }));
        assert!(decode_batch(&bytes[..bytes.len() - 8]).is_none());
        assert!(decode_batch(&parts[0].1).is_none(), "a lone reply isn't a batch");
    }

    #[test]
    fn rejects_truncated_or_foreign_bytes() {
        let bytes = encode(1, Viewport { t0: 0, t1: 10, width_px: 10 }, None, &[]);
        assert_eq!(decode(&bytes).map(|r| r.built_until), Some(None));
        assert!(decode(&bytes[..39]).is_none());
        assert!(decode(b"NOPE").is_none());
    }
}
