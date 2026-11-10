//! Two files on one timeline: a known offset and clock drift are removed by a time map, to within a sample.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use tlmbench_core::Viewport;
use tlmbench_core::timemap::{self, TimeMap};
use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec};
use tlmbench_decode::reference;
use tlmbench_store::group::Group;
use tlmbench_store::{Precision, Request, Store};

fn descriptor(name: &str) -> Descriptor {
    Descriptor {
        sequence: None,
        files: Vec::new(),
        packet_types: Vec::new(),
        packets: None,
        schema: SCHEMA,
        name: None,
        endian: Endian::Little,
        header_size: 0,
        record_size: 4,
        time: TimeSpec::Rate { rate_hz: 1000.0, t0_ns: 0 },
        fields: vec![Field {
            name: name.into(),
            dtype: DType::F32,
            offset: 0,
            unit: None,
            scale: 1.0,
            value_offset: 0.0,
            bits: None,
        }],
    }
}

/// The signal at true time `s` seconds.
fn signal(s: f64) -> f32 {
    (((0.3 * s + 0.02 * s * s) * std::f64::consts::TAU).sin() * 5.0) as f32
}

/// `n` samples at 1 kHz on the file's own clock, where own time `t` is true time `drift × t + offset`.
fn write(path: &Path, n: usize, drift: f64, offset: f64) {
    let mut f = fs::File::create(path).unwrap();
    reference::write_header(&mut f, &descriptor("a")).unwrap();
    let buf: Vec<u8> = (0..n).flat_map(|i| signal(drift * (i as f64 / 1000.0) + offset).to_le_bytes()).collect();
    f.write_all(&buf).unwrap();
}

#[test]
fn drift_and_offset_align_to_within_a_sample() {
    let dir = tempfile::tempdir().unwrap();
    let (pa, pb) = (dir.path().join("a.tlm"), dir.path().join("b.tlm"));
    write(&pa, 20_000, 1.0, 0.0);
    // b's clock runs 200 ppm slow (true = own × 1.0002) and starts 1.2345 s ahead
    write(&pb, 20_000, 1.0002, 1.2345);
    let store = Store::new(Some(&dir.path().join("cache")));
    let a = Arc::new(store.open(&pa, None).unwrap());
    let b = Arc::new(store.open(&pb, None).unwrap());

    // two sync points name the same moments in both clocks: b's own 0 s and 15 s
    let ns = |s: f64| (s * 1e9).round() as i64;
    let pairs = [(ns(0.0), ns(1.2345)), (ns(15.0), ns(15.0 * 1.0002 + 1.2345))];
    let est = timemap::from_pairs(&pairs, false).unwrap();
    // (b's true time is a's own time: the shared timeline is a's clock; the map takes b's own time to it)
    let g = Group::new(vec![(a, TimeMap::identity()), (b, est.map)], vec![1, 2]);
    assert_eq!(g.info().channels.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["a", "a·2"]);

    // the nearest samples at the same shared times agree to within a sample's worth of the signal's slope
    for t in [3.0, 7.7, 11.1] {
        let r = g.readings(&[0, 1], ns(t));
        let (ra, rb) = (r[0].as_ref().unwrap(), r[1].as_ref().unwrap());
        assert!((ra.value - rb.value).abs() < 0.1, "{t}: {} vs {}", ra.value, rb.value);
        assert!((ra.t - rb.t).abs() <= 1_000_000, "within a sample: {} {}", ra.t, rb.t);
    }

    // a view returns times on the shared timeline
    let req = Request {
        channels: vec![0, 1],
        gap_factors: vec![],
        vp: Viewport { t0: ns(5.0), t1: ns(6.0), width_px: 100, raw_max: 0 },
        connect_gaps: false,
    };
    let views = g.view_batch(&[req], Precision::Exact).pop().unwrap();
    assert_eq!(views.iter().map(|v| v.channel).collect::<Vec<_>>(), [0, 1]);
    for v in &views {
        assert!(
            v.trace.t.iter().all(|&t| t >= ns(4.99) && t <= ns(6.01)),
            "{:?}",
            &v.trace.t[..3.min(v.trace.t.len())]
        );
    }
}

#[test]
fn a_known_shift_is_found_by_correlation_and_applied() {
    let dir = tempfile::tempdir().unwrap();
    let (pa, pb) = (dir.path().join("a.tlm"), dir.path().join("b.tlm"));
    write(&pa, 20_000, 1.0, 0.0);
    write(&pb, 20_000, 1.0, 2.5);
    let store = Store::new(Some(&dir.path().join("cache")));
    let a = store.open(&pa, None).unwrap();
    let b = store.open(&pb, None).unwrap();
    let grid = |o: &tlmbench_store::Opened| {
        let rec = o.recording();
        let n = rec.info().samples;
        let t = rec.read_times(0, n);
        let v = rec.read(0, 0, n).to_f64(1.0, 0.0);
        timemap::Grid::resample(&t, &v, 4000).unwrap()
    };
    let (ga, gb) = (grid(&a), grid(&b));
    let (shift, r) = timemap::xcorr(&ga, &gb, 1000).unwrap();
    assert!(r > 0.99);
    // b's moment own is true own + 2.5, which a's clock calls own + 2.5: the shift is 2.5 s, to within a sample
    assert!((shift - 2_500_000_000).abs() < 1_000_000 * 5, "{shift}"); // a grid step is 5 ms here
}

#[test]
fn windowed_correlation_recovers_offset_and_drift_to_a_sample() {
    use tlmbench_store::align::{Method, estimate};
    let dir = tempfile::tempdir().unwrap();
    let (pa, pb) = (dir.path().join("a.tlm"), dir.path().join("b.tlm"));
    write(&pa, 20_000, 1.0, 0.0);
    write(&pb, 20_000, 1.0002, 1.2345);
    let store = Store::new(Some(&dir.path().join("cache")));
    let a = store.open(&pa, None).unwrap();
    let b = store.open(&pb, None).unwrap();
    let got =
        estimate(dir.path(), &a, &TimeMap::identity(), &b, &Method::Xcorr { channel: None, windows: Some(3) }).unwrap();
    assert!(got.quality > 0.9, "{}", got.quality);
    assert!(got.note.as_deref().unwrap().starts_with("Matched on a:"), "{:?}", got.note);
    for own in [0.5, 8.0, 19.5] {
        let want = (own * 1.0002 + 1.2345) * 1e9;
        let have = got.map.apply((own * 1e9) as i64) as f64;
        assert!((have - want).abs() < 1.5e6, "{own}: off by {} ms", (have - want) / 1e6);
    }
}
