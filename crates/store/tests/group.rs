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
        events: Vec::new(),
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
            category: None,
            bits: None,
            plugin: None,
            size: 0,
            params: None,
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

#[test]
fn a_recipe_reads_another_files_channel_through_its_time_map() {
    use tlmbench_expr::Recipe;
    use tlmbench_store::Foreign;
    let dir = tempfile::tempdir().unwrap();
    let (pa, pb) = (dir.path().join("a.tlm"), dir.path().join("b.tlm"));
    write(&pa, 20_000, 1.0, 0.0);
    // b is the same signal logged 2.5 s ahead on its own clock, at 1 kHz too
    write(&pb, 20_000, 1.0, 2.5);
    let store = Store::new(Some(&dir.path().join("cache")));
    let a = store.open(&pa, None).unwrap();
    let b = Arc::new(store.open(&pb, None).unwrap());
    let map = TimeMap::offset(2_500_000_000);
    let foreign = [Foreign { name: "a·2".into(), opened: b, channel: 0, map }];
    let recipe = |name: &str, expr: &str| Recipe {
        name: name.into(),
        expr: expr.into(),
        unit: None,
        category: None,
        decimals: None,
        automatic: false,
    };
    let d = store
        .open_derived_cross(&a, &[recipe("Diff", "a - \"a·2\""), recipe("Unknown", "a - \"a·3\"")], &foreign)
        .unwrap();
    assert_eq!(d.names, ["Diff"], "a name no file has is skipped");
    let o = d.opened.unwrap();
    let diff = o.recording().read(0, 3000, 17_000).to_f64(1.0, 0.0);
    // the same signal at the same shared moments: nothing left but float rounding
    assert!(diff.iter().all(|x| x.abs() < 1e-3), "worst {}", diff.iter().fold(0.0f64, |m, x| m.max(x.abs())));
    // a wrong map shows: shifted by 50 ms the difference is large
    let off = [Foreign {
        name: "a·2".into(),
        opened: Arc::new(store.open(&pb, None).unwrap()),
        channel: 0,
        map: TimeMap::offset(2_450_000_000),
    }];
    let d = store.open_derived_cross(&a, &[recipe("Diff", "a - \"a·2\"")], &off).unwrap();
    let diff = d.opened.unwrap().recording().read(0, 3000, 17_000).to_f64(1.0, 0.0);
    assert!(diff.iter().any(|x| x.abs() > 0.1), "a different map gives a different channel (and its own cache entry)");
}

/// A file of one f32 channel at `rate_hz` with these values.
fn write_values(path: &Path, name: &str, rate_hz: f64, values: &[f32]) {
    let mut d = descriptor(name);
    d.time = TimeSpec::Rate { rate_hz, t0_ns: 0 };
    let mut f = fs::File::create(path).unwrap();
    reference::write_header(&mut f, &d).unwrap();
    f.write_all(&values.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>()).unwrap();
}

#[test]
fn tables_sort_and_filter_and_xy_pairs_by_another_files_held_column() {
    use tlmbench_core::table::{Col, Filter, SortKey};
    use tlmbench_store::xy::XyReq;
    let dir = tempfile::tempdir().unwrap();
    let (pa, pb) = (dir.path().join("a.tlm"), dir.path().join("b.tlm"));
    // a: 100 samples at 1 kHz (0..99 ms), its value the ms; b: 10 at 100 Hz, falling 9..0, placed 20 ms later
    write_values(&pa, "a", 1000.0, &(0..100).map(|i| i as f32).collect::<Vec<_>>());
    write_values(&pb, "b", 100.0, &(0..10).map(|i| (9 - i) as f32).collect::<Vec<_>>());
    let store = Store::new(Some(&dir.path().join("cache")));
    let a = Arc::new(store.open(&pa, None).unwrap());
    let b = Arc::new(store.open(&pb, None).unwrap());
    let g = Group::new(vec![(a, TimeMap::identity()), (b, TimeMap::offset(20_000_000))], vec![1, 2]);
    let all = (0, 100_000_000);
    let go = |_: f64| true;
    // the held value of b at a's row r (ms): NaN before 20 ms, then 9 - (r - 20) / 10
    let held = |r: u32| if r < 20 { f64::NAN } else { f64::from(9 - (r as i32 - 20) / 10) };

    // a filter on b's column keeps a's rows where b's held value passes
    let f = |t: &str| Filter::parse(t).unwrap().unwrap();
    let (first, rows) = g.select_rows(all, vec![(Col::Channel(1), f(">= 8"))], Vec::new(), &go).unwrap();
    assert_eq!(first, 0);
    assert_eq!(rows.ids, (20..40).collect::<Vec<u32>>());

    // a sort by b's column: ascending puts the lowest held value first and the rows before b's start (blank) last
    let by = SortKey { col: Col::Channel(1), descending: false };
    let (_, rows) = g.select_rows(all, Vec::new(), vec![by], &go).unwrap();
    assert_eq!(rows.ids.len(), 100);
    let keys: Vec<f64> = rows.ids.iter().map(|&r| held(r)).collect();
    assert_eq!(keys[0], 2.0);
    assert!(keys[..80].is_sorted(), "{keys:?}");
    assert!(keys[80..].iter().all(|k| k.is_nan()));

    // Apply to charts by b's column hides a's other rows
    let mask = g.filter_mask(all, vec![(Col::Channel(1), f("<= 2"))], &go).unwrap();
    assert_eq!(mask.excluded(), 90);

    // XY of a against b: paired at a's samples on the shared timeline, b held, nothing before b starts
    let q = XyReq { x: 0, ys: vec![1], t0: 0, t1: 100_000_000, rect: None, grid: (10_000, 10_000), max_points: 0 };
    let r = g.xy_points(&q).unwrap();
    assert_eq!(r.series.len(), 1);
    let s = &r.series[0];
    assert_eq!(s.channel, 1);
    assert_eq!(r.range, Some([20.0, 99.0, 2.0, 9.0]));
    assert_eq!(s.x.first(), Some(&20.0));
    for (x, y) in s.x.iter().zip(&s.y) {
        assert_eq!(*y, held(*x as u32), "at {x}");
    }
    // the other way round (X from b) pairs at b's samples, a held at each
    let r = g.xy_points(&XyReq { x: 1, ys: vec![0], ..q }).unwrap();
    assert_eq!(r.series[0].channel, 0);
    assert_eq!(r.series[0].x, (0..8).map(|i| f64::from(9 - i)).collect::<Vec<_>>());
    assert_eq!(r.series[0].y, (0..8).map(|i| f64::from(20 + 10 * i)).collect::<Vec<_>>());
}
