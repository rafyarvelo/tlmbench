//! Recordings that grow: a live session's file followed while it is appended to answers views, values and stats
//! exactly as the same file opened whole at that length.

use std::fs;
use std::io::Write;
use std::path::Path;

use tlmbench_core::Viewport;
use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec, TimeUnit};
use tlmbench_decode::reference;
use tlmbench_store::{Opened, Precision, Request, Store};

fn value(c: usize, i: usize) -> f32 {
    match i {
        _ if i.is_multiple_of(70_001) => 40.0,
        _ if (300_000..300_500).contains(&i) => f32::NAN,
        _ => (i as f32 * 0.001 * (c + 1) as f32).sin() * 5.0 + ((i * 31 + c) % 17) as f32 * 0.05,
    }
}

/// Two f32 channels at 1 kHz, or (with `column`) an i64 time field with a jump every 250k samples.
fn descriptor(column: bool) -> Descriptor {
    let field = |name: &str, dtype, offset| Field {
        name: name.into(),
        dtype,
        offset,
        unit: None,
        scale: 1.0,
        value_offset: 0.0,
    };
    let (time, fields, size) = if column {
        let t = TimeSpec::Field { field: "t".into(), unit: TimeUnit::Ns, t0_ns: 0 };
        (t, vec![field("t", DType::I64, 0), field("a", DType::F32, 8), field("b", DType::F32, 12)], 16)
    } else {
        (TimeSpec::Rate { rate_hz: 1000.0, t0_ns: 0 }, vec![field("a", DType::F32, 0), field("b", DType::F32, 4)], 8)
    };
    Descriptor {
        sequence: None,
        files: Vec::new(),
        packets: None,
        schema: SCHEMA,
        name: None,
        endian: Endian::Little,
        header_size: 0,
        record_size: size,
        time,
        fields,
    }
}

fn append(path: &Path, column: bool, from: usize, to: usize) {
    let mut f = fs::OpenOptions::new().append(true).open(path).unwrap();
    let mut buf = Vec::new();
    for i in from..to {
        if column {
            let t = i as i64 * 1_000_000 + (i as i64 / 250_000) * 5_000_000_000;
            buf.extend_from_slice(&t.to_le_bytes());
        }
        buf.extend_from_slice(&value(0, i).to_le_bytes());
        buf.extend_from_slice(&value(1, i).to_le_bytes());
    }
    f.write_all(&buf).unwrap();
}

fn same(live: &Opened, whole: &Opened) {
    let info = whole.recording().info();
    let (t0, t1) = (info.t_first.unwrap(), info.t_last.unwrap() + 1);
    assert_eq!(live.recording().info().samples, info.samples);
    let span = t1 - t0;
    for (a, b, w) in [
        (t0, t1, 1500),
        (t0, t1, 7),
        (t0 + span / 3, t0 + span / 2, 900),
        (t1 - span / 50, t1, 600),
        (t1 - 3_000_000, t1 + 1_000_000_000, 300),
    ] {
        let req =
            Request { channels: vec![0, 1], gap_factors: vec![4.0, 4.0], vp: Viewport { t0: a, t1: b, width_px: w } };
        for precision in [Precision::Exact, Precision::Coarse] {
            let (l, o) = (
                live.view_batch(std::slice::from_ref(&req), precision),
                whole.view_batch(std::slice::from_ref(&req), Precision::Exact),
            );
            for (lv, ov) in l[0].iter().zip(&o[0]) {
                if precision == Precision::Exact {
                    assert!(
                        lv.trace.t == ov.trace.t
                            && lv.trace.y.iter().zip(&ov.trace.y).all(|(p, q)| p.to_bits() == q.to_bits()),
                        "{a}..{b} @ {w}"
                    );
                }
                assert!(lv.built_until.is_none());
            }
        }
    }
    // counts and extremes exactly; mean and spread to rounding (the sums add in another order)
    let close = |x: f64, y: f64| (x.is_nan() && y.is_nan()) || (x - y).abs() <= 1e-9 * x.abs().max(y.abs()).max(1.0);
    for (a, b) in [(t0, t1), (t0 + span / 4, t1 - 1), (t1 - span / 100, t1)] {
        let (l, o) = (live.stats(&[0, 1], a, b).unwrap(), whole.stats(&[0, 1], a, b).unwrap());
        for (x, y) in l.iter().zip(&o) {
            assert!(
                (x.count, x.min, x.max) == (y.count, y.min, y.max) && close(x.mean, y.mean) && close(x.std, y.std),
                "{a}..{b}: {x:?} {y:?}"
            );
        }
    }
    let mid = t0 + span * 2 / 3;
    assert_eq!(live.values_at(&[0, 1], mid), whole.values_at(&[0, 1], mid));
}

fn follow(column: bool) {
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("cache");
    let path = dir.path().join("live.tlm");
    let mut f = fs::File::create(&path).unwrap();
    reference::write_header(&mut f, &descriptor(column)).unwrap();
    drop(f);
    let store = Store::new(Some(&cache));
    append(&path, column, 0, 100);
    let live = store.open_live(&path).unwrap();
    // growth in uneven steps: past 4^9 samples (the top live level), and a record cut in half at one point
    let mut n = 100;
    for to in [1_000, 70_000, 600_001, 1_100_000] {
        append(&path, column, n, to);
        n = to;
        assert!(live.refresh().unwrap());
        let whole = store.open(&path, None).unwrap();
        whole.build(&mut |_, _| {}).unwrap();
        same(&live.opened(), &whole);
    }
    fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(&[1, 2, 3]).unwrap();
    assert!(!live.refresh().unwrap(), "half a record isn't a sample yet");
    let dirs = || fs::read_dir(cache.join("live")).map(|d| d.count()).unwrap_or(0);
    assert_eq!(dirs(), 1);
    drop(live);
    assert_eq!(dirs(), 0, "the growing pyramid goes with the Live");
}

#[test]
fn a_growing_fixed_rate_file_answers_like_the_whole_file() {
    follow(false);
}

#[test]
fn a_growing_file_with_a_time_column_answers_like_the_whole_file() {
    follow(true);
}
