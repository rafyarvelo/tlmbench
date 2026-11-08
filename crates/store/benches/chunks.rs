//! Zoomed views of an interleaved recording: 20 f32 channels in one record (the Gate 2 layout), 1M samples in view
//! at 1736 px (a raw reduction, about 576 samples a pixel), all 20 channels per view.
//!
//! - `in_place`: no chunk cache, every field read from the mapped records (how views read before the cache).
//! - `cold`: the chunk cache emptied before each view, so each view decodes its chunks (the file's pages stay in
//!   the OS cache).
//! - `warm`: the chunks already decoded.
//!
//! `TLMBENCH_BENCH_SAMPLES` sets the samples per channel (default 4M, a 320 MB file in a temp folder).

use std::hint::black_box;
use std::io::Write;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use tlmbench_core::Viewport;
use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec};
use tlmbench_decode::reference;
use tlmbench_store::{Precision, Store};

const CHANNELS: usize = 20;

fn samples() -> usize {
    std::env::var("TLMBENCH_BENCH_SAMPLES").ok().and_then(|s| s.parse().ok()).unwrap_or(4 << 20)
}

fn write(path: &std::path::Path, n: usize) {
    let fields = (0..CHANNELS)
        .map(|c| Field {
            name: format!("ch{c}"),
            dtype: DType::F32,
            offset: 4 * c as u64,
            unit: None,
            scale: 1.0,
            value_offset: 0.0,
        })
        .collect();
    let desc = Descriptor {
        sequence: None,
        files: Vec::new(),
        packet_types: Vec::new(),
        packets: None,
        schema: SCHEMA,
        name: None,
        endian: Endian::Little,
        header_size: 0,
        record_size: 4 * CHANNELS as u64,
        time: TimeSpec::Rate { rate_hz: 10_000.0, t0_ns: 1_767_225_600_000_000_000 },
        fields,
    };
    let mut w = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    reference::write_header(&mut w, &desc).unwrap();
    let mut x = 0x9E37_79B9_7F4A_7C15u64;
    let mut rec = [0u8; 4 * CHANNELS];
    for i in 0..n {
        for c in 0..CHANNELS {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let v = (i as f32 * 1e-4 * (c + 1) as f32).sin() * 10.0 + (x >> 40) as f32 / (1u64 << 24) as f32;
            rec[4 * c..4 * c + 4].copy_from_slice(&v.to_le_bytes());
        }
        w.write_all(&rec).unwrap();
    }
    w.flush().unwrap();
}

fn bench_zoomed(c: &mut Criterion) {
    let n = samples();
    let dir = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let path = dir.path().join("gate2-like.tlm");
    write(&path, n);
    let all: Vec<usize> = (0..CHANNELS).collect();
    let in_place = Store::new(Some(cache.path())).with_chunk_budget(0);
    let chunked = Store::new(Some(cache.path()));
    let a = in_place.open(&path, None).unwrap();
    let b = chunked.open(&path, None).unwrap();
    let t = |i: usize| a.recording().time_at(i);
    let mid = n / 2;
    let vp = Viewport { t0: t(mid - 500_000), t1: t(mid + 500_000), width_px: 1736, raw_max: 0 };

    let mut g = c.benchmark_group(format!("zoomed_1M_x{CHANNELS}"));
    g.sample_size(30);
    g.bench_function("in_place", |bch| bch.iter(|| black_box(a.views(&all, vp, &[], Precision::Exact))));
    g.bench_function("cold", |bch| {
        bch.iter_batched(
            || chunked.chunk_cache().clear(),
            |()| black_box(b.views(&all, vp, &[], Precision::Exact)),
            BatchSize::PerIteration,
        )
    });
    b.views(&all, vp, &[], Precision::Exact);
    g.bench_function("warm", |bch| bch.iter(|| black_box(b.views(&all, vp, &[], Precision::Exact))));
    g.finish();
}

criterion_group!(benches, bench_zoomed);
criterion_main!(benches);
