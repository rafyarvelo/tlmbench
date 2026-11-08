//! M4 straight from raw samples: the Phase 1 viewport query. Design target: under 5 ms per trace.
//!
//! One fixed-rate channel of 100M f32 samples (the Phase 1 gate size, 400 MB), viewed whole, at 10% and at 0.1%,
//! on all threads and on one. Plus 10M samples with a timestamp column (binary searches instead of arithmetic),
//! and the Phase 2 pyramid: building it, and the same views answered from it, exactly and coarse.
//! `TLMBENCH_BENCH_SAMPLES` overrides the size.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use tlmbench_core::pyramid::{self, Pyramid};
use tlmbench_core::{RateTimes, Times, Timestamp, Viewport, m4};

fn samples(default: usize) -> usize {
    std::env::var("TLMBENCH_BENCH_SAMPLES").ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}

/// Sine + noise + occasional spikes, like the synthetic recordings.
fn signal(n: usize) -> Vec<f32> {
    let mut x = 0x9E37_79B9_7F4A_7C15u64;
    (0..n)
        .map(|i| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let noise = (x >> 40) as f32 / (1u64 << 24) as f32 - 0.5;
            let spike = if x.is_multiple_of(100_000) { 25.0 } else { 0.0 };
            (i as f32 * 1e-4).sin() * 10.0 + noise + spike
        })
        .collect()
}

fn views(times: &impl Times) -> [(&'static str, Timestamp, Timestamp); 3] {
    let (start, end) = (times.at(0), times.at(times.len() - 1) + 1);
    let span = end - start;
    let mid = start + span / 2;
    [
        ("full", start, end),
        ("10pct", mid - span / 20, mid + span / 20),
        ("0.1pct", mid - span / 2000, mid + span / 2000),
    ]
}

fn bench_rate(c: &mut Criterion) {
    let n = samples(100_000_000);
    let values = signal(n);
    let times = RateTimes::from_rate(1_700_000_000_000_000_000, 10_000.0, n);
    let one_thread = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap();

    let mut g = c.benchmark_group(format!("m4_rate_{}M", n / 1_000_000));
    g.sample_size(20);
    for (view, t0, t1) in views(&times) {
        let in_view = times.lower_bound(t1) - times.lower_bound(t0);
        g.throughput(Throughput::Elements(in_view as u64));
        for width_px in [1000u32, 2000, 4000] {
            let vp = Viewport { t0, t1, width_px, raw_max: 0 };
            g.bench_with_input(BenchmarkId::new(format!("{view}/all_threads"), width_px), &vp, |b, &vp| {
                b.iter(|| black_box(m4(&times, &values, vp)))
            });
            if width_px == 2000 {
                g.bench_with_input(BenchmarkId::new(format!("{view}/one_thread"), width_px), &vp, |b, &vp| {
                    one_thread.install(|| b.iter(|| black_box(m4(&times, &values, vp))))
                });
            }
        }
    }
    g.finish();
}

fn bench_column(c: &mut Criterion) {
    let n = samples(100_000_000) / 10;
    let values = signal(n);
    let times: Vec<Timestamp> =
        (0..n as i64).map(|i| 1_700_000_000_000_000_000 + i * 100_000 + (i % 7) * 1000).collect();

    let mut g = c.benchmark_group(format!("m4_column_{}M", n / 1_000_000));
    g.sample_size(20);
    for (view, t0, t1) in views(&times) {
        let vp = Viewport { t0, t1, width_px: 2000, raw_max: 0 };
        g.throughput(Throughput::Elements((times.lower_bound(t1) - times.lower_bound(t0)) as u64));
        g.bench_with_input(BenchmarkId::new(view, 2000), &vp, |b, &vp| b.iter(|| black_box(m4(&times, &values, vp))));
    }
    g.finish();
}

fn bench_pyramid(c: &mut Criterion) {
    let n = samples(100_000_000);
    let values = signal(n);
    let times = RateTimes::from_rate(1_700_000_000_000_000_000, 10_000.0, n);
    let mut g = c.benchmark_group(format!("pyramid_{}M", n / 1_000_000));
    g.sample_size(10);
    g.throughput(Throughput::Elements(n as u64));
    g.bench_function("build", |b| {
        b.iter(|| black_box(pyramid::build_levels(pyramid::summarize(&times, &values, 0, n))))
    });
    let levels = pyramid::build_levels(pyramid::summarize(&times, &values, 0, n));
    let pyr = Pyramid::new(n, levels.iter().map(Vec::as_slice).collect());
    for (view, t0, t1) in views(&times) {
        let vp = Viewport { t0, t1, width_px: 2000, raw_max: 0 };
        g.throughput(Throughput::Elements((times.lower_bound(t1) - times.lower_bound(t0)) as u64));
        g.bench_with_input(BenchmarkId::new(format!("{view}/m4"), 2000), &vp, |b, &vp| {
            b.iter(|| black_box(pyramid::m4(&times, &values, &pyr, vp, &[])))
        });
        g.bench_with_input(BenchmarkId::new(format!("{view}/coarse"), 2000), &vp, |b, &vp| {
            b.iter(|| black_box(pyramid::m4_coarse(&times, &values, &pyr, vp, &[])))
        });
    }
    g.finish();
}

criterion_group!(benches, bench_rate, bench_column, bench_pyramid);
criterion_main!(benches);
