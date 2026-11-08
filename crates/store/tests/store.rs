//! The store end to end: builds, reopens, resumes, queries, and never writes beside the data.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use tlmbench_core::{TraceKind, Viewport};
use tlmbench_decode::descriptor::TimeUnit;
use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec};
use tlmbench_decode::reference;
use tlmbench_store::{Limits, Precision, Request, Status, Store};

/// Samples per channel: a little over three 4^10-sample blocks.
const N: usize = 3 * (1 << 20) + 12_345;

fn value(c: usize, i: usize) -> f32 {
    let x = (i as f32 * 0.0007 * (c + 1) as f32).sin() * 10.0 + ((i * 7919 + c) % 101) as f32 * 0.01;
    match i {
        _ if i.is_multiple_of(400_009) => 50.0,
        _ if (2_000_000..2_005_000).contains(&i) => f32::NAN,
        _ => x,
    }
}

/// A two-channel interleaved .tlm at 10 kHz in `dir`.
fn recording(dir: &Path) -> PathBuf {
    let field = |name: &str, offset| Field {
        name: name.into(),
        dtype: DType::F32,
        offset,
        unit: None,
        scale: 1.0,
        value_offset: 0.0,
    };
    let desc = Descriptor {
        sequence: None,
        files: Vec::new(),
        packet_types: Vec::new(),
        packets: None,
        schema: SCHEMA,
        name: None,
        endian: Endian::Little,
        header_size: 0,
        record_size: 8,
        time: TimeSpec::Rate { rate_hz: 10_000.0, t0_ns: 1_700_000_000_000_000_000 },
        fields: vec![field("a", 0), field("b", 4)],
    };
    let path = dir.join("rec.tlm");
    let mut w = std::io::BufWriter::new(fs::File::create(&path).unwrap());
    reference::write_header(&mut w, &desc).unwrap();
    for i in 0..N {
        w.write_all(&value(0, i).to_le_bytes()).unwrap();
        w.write_all(&value(1, i).to_le_bytes()).unwrap();
    }
    w.flush().unwrap();
    path
}

/// Path → (size, modified) for every file under `dir`.
fn snapshot(dir: &Path) -> Vec<(PathBuf, u64, std::time::SystemTime)> {
    let mut out: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| {
            let m = e.metadata().unwrap();
            (e.path(), m.len(), m.modified().unwrap())
        })
        .collect();
    out.sort();
    out
}

/// Equal traces, NaN (a gap) equal to NaN.
fn same(a: &tlmbench_core::Trace, b: &tlmbench_core::Trace) -> bool {
    (a.kind, a.bucket_ns, &a.t) == (b.kind, b.bucket_ns, &b.t)
        && a.y.iter().map(|y| y.to_bits()).eq(b.y.iter().map(|y| y.to_bits()))
}

fn views(first: i64, last: i64) -> Vec<Viewport> {
    let span = last - first;
    let mut out = vec![Viewport { t0: first, t1: last + 1, width_px: 1999, raw_max: 0 }];
    for k in 1..12i64 {
        let t0 = first + span * k / 13 - 777_777 * k;
        out.push(Viewport { t0, t1: t0 + span / (k * k * 3) + 1, width_px: 300 + 97 * k as u32, raw_max: 0 });
    }
    out
}

#[test]
fn builds_reopens_and_answers_like_raw_samples() {
    let data = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let path = recording(data.path());
    let before = snapshot(data.path());
    let store = Store::new(Some(cache.path()));

    let opened = store.open(&path, None).unwrap();
    assert_eq!(opened.status(), Status::Missing);
    let mut calls = 0;
    opened
        .build(&mut |done, total| {
            calls += 1;
            assert!(done <= total && total == N as u64);
        })
        .unwrap();
    assert!(calls >= 4, "progress after each block");
    assert_eq!(opened.status(), Status::Built);
    let info = opened.recording().info().clone();
    let (first, last) = (info.t_first.unwrap(), info.t_last.unwrap());
    for vp in views(first, last) {
        for c in 0..2 {
            let v = opened.view(c, vp, 4.0, Precision::Exact);
            assert!(same(&v.trace, &opened.recording().trace(c, vp)), "channel {c}, {vp:?}");
            assert!(v.from_pyramid && v.built_until.is_none());
        }
    }
    let vp = Viewport { t0: first, t1: last + 1, width_px: 1999, raw_max: 0 };
    let coarse = opened.views(&[0, 1], vp, &[4.0, 4.0], Precision::Coarse);
    assert!(coarse.iter().all(|v| v.coarse && v.from_pyramid));
    assert!(coarse[0].trace.y.contains(&50.0), "coarse views keep every spike too");
    let whole = opened.view(0, vp, 4.0, Precision::Exact);
    assert!(!whole.coarse);
    assert_eq!(whole.trace.kind, TraceKind::M4);
    assert!(whole.trace.y.iter().any(|y| y.is_nan()), "the NaN run is a gap");
    assert!(whole.trace.y.contains(&50.0), "every spike is kept");

    // stats against brute force
    let (a, b) = (123_457usize, 2_765_432usize);
    let st = opened.stats(&[1], opened.recording().time_at(a), opened.recording().time_at(b)).unwrap().remove(0);
    let real: Vec<f64> = (a..b).map(|i| value(1, i)).filter(|v| !v.is_nan()).map(f64::from).collect();
    assert_eq!(st.count, real.len() as u64);
    assert_eq!(st.max.unwrap().0, real.iter().copied().fold(f64::MIN, f64::max));
    assert!((st.mean - real.iter().sum::<f64>() / real.len() as f64).abs() < 1e-9);

    // nearest sample
    let t = opened.recording().time_at(1000) + 40_000; // 0.4 of a period after sample 1000
    assert_eq!(opened.nearest(0, t).map(|(i, _, _)| i), Some(1000));
    drop(opened);

    // a second open reads the built pyramid without building
    let again = store.open(&path, None).unwrap();
    assert_eq!(again.status(), Status::Built);
    assert_eq!(snapshot(data.path()), before, "nothing written beside the data");
}

#[test]
fn an_interrupted_build_resumes_where_it_stopped() {
    let data = tempfile::tempdir().unwrap();
    let path = recording(data.path());
    let (c1, c2) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());

    // build in one go in c1, for reference
    let whole = Store::new(Some(c1.path())).open(&path, None).unwrap();
    whole.build(&mut |_, _| {}).unwrap();

    // in c2, stop after the second block
    let store = Store::new(Some(c2.path()));
    let first = store.open(&path, None).unwrap();
    let blocks = std::cell::Cell::new(0);
    first
        .build(&mut |done, _| {
            if done > 0 {
                blocks.set(blocks.get() + 1);
            }
            if blocks.get() == 2 {
                first.cancel();
            }
        })
        .unwrap();
    assert_eq!(first.status(), Status::Building { done: 2 << 20, total: N as u64 });
    // views are exact meanwhile: from the built blocks, raw beyond
    let info = first.recording().info().clone();
    for vp in views(info.t_first.unwrap(), info.t_last.unwrap()) {
        let v = first.view(1, vp, 4.0, Precision::Exact);
        assert!(same(&v.trace, &first.recording().trace(1, vp)), "{vp:?}");
        assert!(v.built_until.is_some());
    }
    drop(first);
    let building: Vec<_> = store.entries().unwrap().into_iter().filter(|e| e.building).collect();
    assert_eq!(building.len(), 1);

    let second = store.open(&path, None).unwrap();
    let mut starts = None;
    second
        .build(&mut |done, _| {
            starts.get_or_insert(done);
        })
        .unwrap();
    assert_eq!(starts, Some(2 << 20), "resumed after two blocks");
    assert_eq!(second.status(), Status::Built);
    let dir = |root: &Path| root.join("pyr/v2").join(second.key());
    for f in ["c0.pyr", "c1.pyr", "gaps.bin"] {
        assert_eq!(fs::read(dir(c1.path()).join(f)).unwrap(), fs::read(dir(c2.path()).join(f)).unwrap(), "{f}");
    }
}

/// Zoomed views of an interleaved file read through decoded chunks: the same traces, stats and values as in place,
/// with the chunk cache within its budget.
#[test]
fn chunked_reads_answer_like_reads_in_place() {
    let data = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let path = recording(data.path());
    let chunk_bytes = 4 * tlmbench_core::chunks::CHUNK_SAMPLES as u64;
    let chunked = Store::new(Some(cache.path())).with_chunk_budget(80 * chunk_bytes);
    let in_place = Store::new(Some(cache.path())).with_chunk_budget(0);
    let a = chunked.open(&path, None).unwrap();
    a.build(&mut |_, _| {}).unwrap();
    let b = in_place.open(&path, None).unwrap();
    let (first, last) = (a.recording().info().t_first.unwrap(), a.recording().info().t_last.unwrap());
    for vp in views(first, last) {
        for p in [Precision::Exact, Precision::Coarse] {
            for (x, y) in a.views(&[0, 1], vp, &[4.0, 4.0], p).iter().zip(&b.views(&[0, 1], vp, &[4.0, 4.0], p)) {
                assert!(same(&x.trace, &y.trace), "{vp:?} {p:?}");
            }
        }
        assert!(chunked.chunk_cache().bytes() <= 80 * chunk_bytes);
    }
    assert!(!chunked.chunk_cache().is_empty(), "zoomed views decoded chunks");
    assert!(in_place.chunk_cache().is_empty());
    // a 1M-sample view: chunks 1 to 16 of each channel, decoded together
    chunked.chunk_cache().clear();
    let t = |i: usize| a.recording().time_at(i);
    a.views(&[0, 1], Viewport { t0: t(70_000), t1: t(1_070_000), width_px: 1736, raw_max: 0 }, &[], Precision::Exact);
    assert_eq!(chunked.chunk_cache().len(), 2 * 16);
    let (i, j) = (12_345, 2_999_999);
    let st = |o: &tlmbench_store::Opened| o.stats(&[0, 1], t(i), t(j)).unwrap();
    for (x, y) in st(&a).iter().zip(&st(&b)) {
        assert_eq!((x.count, x.min, x.max), (y.count, y.min, y.max));
        assert!((x.mean - y.mean).abs() < 1e-9);
    }
    let at = t(2_100_000) + 3;
    assert_eq!(a.values_at(&[0, 1], at), b.values_at(&[0, 1], at));
    drop(a);
    assert!(chunked.chunk_cache().is_empty(), "closing forgets its chunks");
}

/// A batch answers each entry (its own channels, span and width) as a view of it alone would.
#[test]
fn a_batch_answers_like_its_entries_alone() {
    let data = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let path = recording(data.path());
    let store = Store::new(Some(cache.path()));
    let o = store.open(&path, None).unwrap();
    o.build(&mut |_, _| {}).unwrap();
    let t = |i: usize| o.recording().time_at(i);
    let zoomed = |w| Viewport { t0: t(1_000_000), t1: t(1_400_000), width_px: w, raw_max: 0 };
    let whole = Viewport { t0: t(0), t1: t(N - 1) + 1, width_px: 1200, raw_max: 0 };
    let requests = [
        Request { channels: vec![0], gap_factors: vec![4.0], vp: zoomed(900) },
        Request { channels: vec![1, 0], gap_factors: vec![], vp: zoomed(437) },
        Request { channels: vec![1], gap_factors: vec![4.0], vp: whole },
    ];
    for p in [Precision::Coarse, Precision::Exact] {
        let batch = o.view_batch(&requests, p);
        assert_eq!(batch.len(), 3);
        for (r, views) in requests.iter().zip(&batch) {
            let alone = o.views(&r.channels, r.vp, &r.gap_factors, p);
            assert_eq!(views.iter().map(|v| v.channel).collect::<Vec<_>>(), r.channels);
            assert!(views.iter().zip(&alone).all(|(a, b)| same(&a.trace, &b.trace) && a.coarse == b.coarse));
        }
    }
}

/// Each channel breaks its line at its own gap factor, from the one gap index (nothing is rebuilt).
#[test]
fn gap_factors_apply_per_channel() {
    let data = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let field = |name: &str, dtype, offset| Field {
        name: name.into(),
        dtype,
        offset,
        unit: None,
        scale: 1.0,
        value_offset: 0.0,
    };
    let desc = Descriptor {
        sequence: None,
        files: Vec::new(),
        packet_types: Vec::new(),
        packets: None,
        schema: SCHEMA,
        name: None,
        endian: Endian::Little,
        header_size: 0,
        record_size: 12,
        time: TimeSpec::Field { field: "t".into(), unit: TimeUnit::Us, t0_ns: 0 },
        fields: vec![field("t", DType::I64, 0), field("a", DType::F32, 8)],
    };
    let path = data.path().join("gaps.tlm");
    let mut w = std::io::BufWriter::new(fs::File::create(&path).unwrap());
    reference::write_header(&mut w, &desc).unwrap();
    // 100 µs steps, with a 3× jump before sample 3000 and a 6× jump before sample 6000
    let mut t = 0i64;
    for i in 0..10_000 {
        t += match i {
            3000 => 300,
            6000 => 600,
            _ => 100,
        };
        w.write_all(&t.to_le_bytes()).unwrap();
        w.write_all(&((i % 17) as f32).to_le_bytes()).unwrap();
    }
    w.flush().unwrap();
    drop(w);
    let store = Store::new(Some(cache.path()));
    let o = store.open(&path, None).unwrap();
    o.build(&mut |_, _| {}).unwrap();
    // the time column's chunk index is kept with the pyramid, and searches through it find the same samples
    let index = fs::read(cache.path().join("pyr/v2").join(o.key()).join("times.bin")).unwrap();
    assert_eq!(index.len(), 8);
    drop(o);
    let o = store.open(&path, None).unwrap();
    assert_eq!(o.status(), Status::Built);
    let vp = Viewport { t0: 0, t1: t * 1000 + 1, width_px: 100, raw_max: 0 };
    let gaps = |f: f64| o.view(0, vp, f, Precision::Exact).trace.y.iter().filter(|y| y.is_nan()).count();
    assert_eq!((gaps(2.0), gaps(4.0), gaps(8.0)), (2, 1, 0));
    let both = o.views(&[0, 0], vp, &[2.0, 8.0], Precision::Exact);
    let nan = |k: usize| both[k].trace.y.iter().filter(|y| y.is_nan()).count();
    assert_eq!((nan(0), nan(1)), (2, 0));
    assert_eq!(gaps(1.0), 2, "factors below 2 count as 2");
}

#[test]
fn eviction_keeps_what_is_in_use() {
    let data = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let path = recording(data.path());
    let mut store = Store::new(Some(cache.path()));
    let opened = store.open(&path, None).unwrap();
    opened.build(&mut |_, _| {}).unwrap();
    // a stale entry from an older recording, in an older layout
    let stale = cache.path().join("pyr/v1/0123456789abcdef0123456789abcdef");
    fs::create_dir_all(&stale).unwrap();
    fs::write(stale.join("c0.pyr"), vec![0u8; 4096]).unwrap();

    store.limits = Limits { cap: 1, min_free: 0 };
    let removed = store.evict().unwrap();
    assert_eq!(removed, vec![stale]);
    assert!(cache.path().join("pyr/v2").join(opened.key()).is_dir(), "in use: kept");
    drop(opened);
    let removed = store.evict().unwrap();
    assert_eq!(removed.len(), 1, "no longer in use: evicted");
}
