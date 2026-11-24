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

/// Every alignment method but correlation (tested above) and plugins, each against a shift it must find (coverage,
/// Nov 24: no Rust test reached them; the app's tests run them against the fake core). The reference sits 1 s along
/// the shared timeline, so each map must carry the reference's own map too.
#[test]
fn each_alignment_method_finds_a_known_shift() {
    use tlmbench_store::align::{Method, estimate};
    use tlmbench_store::events::{self, Event};
    let dir = tempfile::tempdir().unwrap();
    let (pa, pb) = (dir.path().join("a.tlm"), dir.path().join("b.tlm"));
    write(&pa, 20_000, 1.0, 0.0);
    write(&pb, 20_000, 1.0, 0.0);
    let store = Store::new(Some(&dir.path().join("cache")));
    let a = store.open(&pa, None).unwrap();
    let b = store.open(&pb, None).unwrap();
    let s = |x: f64| (x * 1e9).round() as i64;
    let rmap = TimeMap::offset(s(1.0));
    let run = |m: Method| estimate(dir.path(), &a, &rmap, &b, &m);
    let at = |m: Method, own: f64| run(m).unwrap().map.apply(s(own));

    // both start at 0: b's start goes where a's start is, 1 s along
    assert_eq!(at(Method::Starts, 0.0), s(1.0));
    assert_eq!(at(Method::Starts, 5.0), s(6.0));
    // the clocks agree: b takes a's map as it is
    assert_eq!(at(Method::Clock, 5.0), s(6.0));
    // a manual shift adds to the starts
    assert_eq!(at(Method::Manual { seconds: 0.25 }, 0.0), s(1.25));
    // sync pairs (b's own time, the shared time): a straight line through two, and piecewise through three
    let pairs = |p: &[(f64, f64)]| p.iter().map(|&(x, y)| (s(x).to_string(), s(y).to_string())).collect::<Vec<_>>();
    let straight = Method::Sync { pairs: pairs(&[(1.0, 3.0), (9.0, 11.0)]), piecewise: false };
    assert_eq!(at(straight, 5.0), s(7.0));
    let bent = Method::Sync { pairs: pairs(&[(0.0, 0.0), (10.0, 11.0), (20.0, 20.0)]), piecewise: true };
    assert_eq!((at(bent.clone(), 10.0), at(bent, 5.0)), (s(11.0), s(5.5)));
    let half = Method::Sync { pairs: vec![("1.5".into(), "2".into())], piecewise: false };
    assert!(run(half).unwrap_err().to_string().contains("whole number of nanoseconds"));

    // events: "Sync" is in both files (a at 2 s and 12 s, b at 1 s and 11 s), "Only a" in one
    let ev = |name: &str, t: f64, id: &str| Event {
        id: id.into(),
        name: name.into(),
        start: s(t).to_string(),
        duration: "0".into(),
        category: String::new(),
        source: "user".into(),
        channels: Vec::new(),
        by: None,
    };
    for (o, list) in
        [(&a, vec![("Sync", 2.0), ("Sync", 12.0), ("Only a", 4.0)]), (&b, vec![("Sync", 1.0), ("Sync", 11.0)])]
    {
        for (k, (n, t)) in list.into_iter().enumerate() {
            events::save(dir.path(), o.identity(), ev(n, t, &format!("e{k}"))).unwrap();
        }
    }
    let got = run(Method::Events { name: None }).unwrap();
    assert_eq!(got.note.as_deref(), Some("Paired 2 “Sync” events"), "the name both files have, when none is given");
    // b's own 1 s is a's own 2 s, which the shared timeline puts at 3 s
    assert_eq!((got.map.apply(s(1.0)), got.map.apply(s(11.0))), (s(3.0), s(13.0)));
    let one_sided = run(Method::Events { name: Some("Only a".into()) }).unwrap_err().to_string();
    assert!(one_sided.contains("No “Only a” events in both files"), "{one_sided}");

    // a time channel both files record (a GPS time of week, say): b's reads 2 s ahead of a's at the same own time
    let (pc, pd) = (dir.path().join("c.tlm"), dir.path().join("d.tlm"));
    let gps = |offset: f32| (0..2000).map(|i| 100.0 + offset + i as f32 / 100.0).collect::<Vec<f32>>();
    write_values(&pc, "gps", 100.0, &gps(0.0));
    write_values(&pd, "gps", 100.0, &gps(2.0));
    let (c, d) = (store.open(&pc, None).unwrap(), store.open(&pd, None).unwrap());
    let ts = |ch: Option<&str>| {
        estimate(dir.path(), &c, &TimeMap::identity(), &d, &Method::TimeSource { channel: ch.map(Into::into) })
    };
    let got = ts(Some("gps")).unwrap();
    assert_eq!(got.channel.as_deref(), Some("gps"));
    for own in [1.0, 5.0, 15.0] {
        // the moment d's clock calls `own` is when c's channel reads what d's does: c's own + 2 s
        let off = got.map.apply(s(own)) - s(own + 2.0);
        assert!(off.abs() < 1_000_000, "{own}: off by {} ms", off as f64 / 1e6);
    }
    assert!(ts(Some("nope")).unwrap_err().to_string().contains("Both files need a channel named nope"));
}

/// Two files in one tab: exact statistics over a shared span (each file's own samples through its map), widget values
/// read through the map, and the build's progress weighted by each file's samples (coverage, Nov 24).
#[test]
fn a_group_answers_stats_widgets_and_progress_across_its_files() {
    use tlmbench_store::Status;
    use tlmbench_store::widgets::WidgetReq;
    let dir = tempfile::tempdir().unwrap();
    let (pa, pb) = (dir.path().join("a.tlm"), dir.path().join("b.tlm"));
    // a counts 0, 1, 2… and b 1000, 1001…, both at 1 kHz; b's own time t is shared time t + 5 s
    write_values(&pa, "x", 1000.0, &(0..10_000).map(|i| i as f32).collect::<Vec<_>>());
    write_values(&pb, "y", 1000.0, &(0..10_000).map(|i| 1000.0 + i as f32).collect::<Vec<_>>());
    let store = Store::new(Some(&dir.path().join("cache")));
    let a = Arc::new(store.open(&pa, None).unwrap());
    let b = Arc::new(store.open(&pb, None).unwrap());
    let s = |x: f64| (x * 1e9).round() as i64;
    let g = Group::new(vec![(a.clone(), TimeMap::identity()), (b, TimeMap::offset(s(5.0)))], vec![1, 2]);

    assert_eq!(g.status(), Status::Missing);
    assert_eq!(g.stats(&[0, 1], s(6.0), s(8.0)), None, "no statistics before the pyramids");
    a.build(&mut |_, _| {}).unwrap();
    assert_eq!(g.status(), Status::Building { done: 10_000, total: 20_000 }, "one file of two, by samples");
    let mut seen = Vec::new();
    g.build(&mut |d, t| seen.push((d, t))).unwrap();
    assert_eq!(g.status(), Status::Built);
    assert!(seen.iter().all(|&(d, t)| t == 20_000 && d <= t), "progress is over both files together: {seen:?}");

    // shared [6 s, 8 s): a's samples 6000..8000, and b's own [1 s, 3 s), samples 1000..3000 (values 2000..3999)
    let st = g.stats(&[0, 1], s(6.0), s(8.0)).unwrap();
    let mm = |k: usize| (st[k].count, st[k].min.unwrap().0, st[k].max.unwrap().0, st[k].mean);
    assert_eq!(mm(0), (2000, 6000.0, 7999.0, 6999.5));
    assert_eq!(mm(1), (2000, 2000.0, 3999.0, 2999.5));

    // a widget frame at shared 7 s: each file answers through its own map (min and max from the view's start)
    let req = |channel| WidgetReq { channel, minmax: true, run: false, trace: 0 };
    let f = g.widgets_frame(s(7.0), (s(6.0), s(8.0)), &[req(0), req(1)]).unwrap();
    let w = |k: usize| (f.widgets[k].value, f.widgets[k].min, f.widgets[k].max);
    assert_eq!(w(0), (Some(7000.0), Some(6000.0), Some(7000.0)));
    assert_eq!(w(1), (Some(3000.0), Some(2000.0), Some(3000.0)));
}
