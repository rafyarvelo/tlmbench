//! Tests for the store's paths the other tests don't reach (coverage, R6c.3, Nov 26): events that are damaged or
//! capped, an adjustment's "missing when", the XY query's edges, the widgets frame on a growing file and on a channel
//! with too many runs, a recording held in memory, and a recording joined with its derived channels read through
//! every method of the recording trait. Each asserts what is answered, not that a line ran.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tlmbench_core::chunks::Held;
use tlmbench_core::mask::Mask;
use tlmbench_core::{Recording, TimeIndex, Viewport};
use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec};
use tlmbench_decode::reference;
use tlmbench_expr::Recipe;
use tlmbench_store::widgets::WidgetReq;
use tlmbench_store::xy::XyReq;
use tlmbench_store::{Adjust, Error, MemoryPyramid, Opened, Precision, Store, events};

fn cache() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn csv(dir: &Path, name: &str, text: &str) -> PathBuf {
    let p = dir.join(name);
    fs::write(&p, text).unwrap();
    p
}

/// `t,a,b` at 1 kHz: a = i, b = 2i, over `n` rows.
fn ramp_csv(dir: &Path, n: usize) -> PathBuf {
    let mut t = String::from("t,a,b\n");
    for i in 0..n {
        t.push_str(&format!("{:.3},{},{}\n", i as f64 / 1000.0, i, 2 * i));
    }
    csv(dir, "ramp.csv", &t)
}

fn field(name: &str, offset: u64) -> Field {
    Field {
        name: name.into(),
        dtype: DType::F32,
        offset,
        unit: None,
        scale: 1.0,
        value_offset: 0.0,
        category: None,
        bits: None,
        plugin: None,
        size: 0,
        params: None,
    }
}

/// A .tlm of one f32 channel `a` at 1 kHz whose values are `f(i)`, for `n` samples.
fn tlm(path: &Path, n: usize, f: impl Fn(usize) -> f32) {
    let d = Descriptor {
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
        fields: vec![field("a", 0)],
    };
    let mut w = std::io::BufWriter::new(fs::File::create(path).unwrap());
    reference::write_header(&mut w, &d).unwrap();
    for i in 0..n {
        w.write_all(&f(i).to_le_bytes()).unwrap();
    }
    w.flush().unwrap();
}

fn append(path: &Path, from: usize, to: usize, f: impl Fn(usize) -> f32) {
    let mut file = fs::OpenOptions::new().append(true).open(path).unwrap();
    for i in from..to {
        file.write_all(&f(i).to_le_bytes()).unwrap();
    }
}

fn recipe(name: &str, expr: &str) -> Recipe {
    Recipe { name: name.into(), expr: expr.into(), unit: None, category: None, decimals: None, automatic: true }
}

// ---- events -------------------------------------------------------------------------------------------------

/// A marked text channel makes events named for its labels; channels that don't exist and missing values are skipped.
#[test]
fn events_from_channels_name_labels_and_skip_what_is_missing() {
    let data = cache();
    let state = csv(data.path(), "state.csv", "t,state,x\n0,idle,1\n1,idle,2\n2,run,3\n3,idle,4\n4,run,5\n");
    let store = Store::new(Some(cache().path()));
    let o = store.open(&state, None).unwrap();
    let ch = o.recording().info().channels.iter().position(|c| c.name == "state").unwrap();
    let got = events::from_channels(o.recording(), &[99, ch]);
    assert_eq!(got.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["run", "idle", "run"]);
    assert!(got.iter().all(|e| e.channels == ["state"] && e.source == "file" && e.category == "mode"));

    // a missing value is neither a change nor the state: 1, (missing), 2 is one change; and a value that is not a whole
    // number or is negative is named by its number
    let num = csv(data.path(), "num.csv", "t,m\n0,1\n1,\n2,2.5\n3,-1\n4,-1\n");
    let o = store.open(&num, None).unwrap();
    let got = events::from_channels(o.recording(), &[0]);
    let named: Vec<_> = got.iter().filter(|e| e.channels == ["m"]).map(|e| e.name.as_str()).collect();
    assert_eq!(named, ["2.5", "-1"]);
}

/// A noisy column can't flood the list.
#[test]
fn a_column_that_flips_every_sample_makes_a_capped_list() {
    let data = cache();
    let mut t = String::from("t,flip\n");
    for i in 0..(events::MAX_FILE_EVENTS + 50) {
        t.push_str(&format!("{i},{}\n", i % 2));
    }
    let p = csv(data.path(), "flip.csv", &t);
    let o = Store::new(Some(cache().path())).open(&p, None).unwrap();
    let got = events::from_channels(o.recording(), &[0]);
    assert_eq!(got.len(), events::MAX_FILE_EVENTS);
    assert_eq!((got[0].name.as_str(), got[1].name.as_str()), ("1", "0"));
}

/// Events kept for a recording: one that can't be read is reported, whatever the call.
#[test]
fn damaged_or_unreadable_event_files_are_reported() {
    let root = cache();
    let dir = root.path().join("events");
    fs::create_dir(&dir).unwrap();
    let ev = |id: &str, start: &str| events::Event {
        id: id.into(),
        name: id.into(),
        start: start.into(),
        duration: "0".into(),
        ..Default::default()
    };

    // a file with no source in it is the user's; they come back in start order
    fs::write(
        dir.join("a.json"),
        r#"[{"id":"2","name":"late","start":"20","duration":"0"},{"id":"1","name":"early","start":"10","duration":"0"}]"#,
    )
    .unwrap();
    let got = events::list(root.path(), "a").unwrap();
    assert_eq!(
        got.iter().map(|e| (e.id.as_str(), e.source.as_str())).collect::<Vec<_>>(),
        [("1", "user"), ("2", "user")]
    );

    // not JSON: damaged, and neither saving nor deleting writes over it
    fs::write(dir.join("b.json"), "{ not json").unwrap();
    assert!(matches!(events::list(root.path(), "b"), Err(Error::Damaged { .. })));
    assert!(matches!(events::save(root.path(), "b", ev("x", "1")), Err(Error::Damaged { .. })));
    assert!(matches!(events::delete(root.path(), "b", "x"), Err(Error::Damaged { .. })));
    assert_eq!(fs::read_to_string(dir.join("b.json")).unwrap(), "{ not json");

    // a folder where the file should be: not "no events", an error
    fs::create_dir(dir.join("c.json")).unwrap();
    assert!(matches!(events::list(root.path(), "c"), Err(Error::Io { .. })));

    // a cache where the events folder can't be made
    let blocked = cache();
    fs::write(blocked.path().join("events"), "a file").unwrap();
    assert!(matches!(events::save(blocked.path(), "d", ev("x", "1")), Err(Error::Io { .. })));
}

// ---- adjustments ---------------------------------------------------------------------------------------------

/// "Missing when" that holds everywhere hides the channel, that holds nowhere changes nothing, and one that can't be
/// read or run refuses to open.
#[test]
fn a_missing_when_that_is_the_same_everywhere_or_wrong() {
    let data = cache();
    let p = ramp_csv(data.path(), 50);
    let open = |when: &str| {
        Store::new(Some(cache().path()))
            .with_adjust(vec![("a".into(), Adjust { when: Some(when.into()), ..Default::default() })])
            .open(&p, None)
    };
    let col = |o: &Opened| o.recording().read(0, 0, 50).to_f64(1.0, 0.0);
    let all = open("1").unwrap();
    assert!(col(&all).iter().all(|v| v.is_nan()), "true everywhere: no sample is left");
    let none = open("0").unwrap();
    assert_eq!(col(&none)[7], 7.0);
    let some = open("b > 90").unwrap();
    assert!(col(&some)[46].is_nan() && col(&some)[45] == 45.0);
    let e = open("a >").err().expect("a refusal");
    assert!(e.to_string().contains("Missing when"), "{e}");
    let e = open("nosuch > 1").err().expect("a refusal");
    assert!(e.to_string().contains("Missing when"), "{e}");
    // all the recordings of a file take the adjustment too
    let many = Store::new(Some(cache().path()))
        .with_adjust(vec![("a".into(), Adjust { when: Some("b > 90".into()), ..Default::default() })])
        .open_all(&p, None)
        .unwrap();
    assert_eq!(many.len(), 1);
    assert!(many[0].recording().read(0, 0, 50).to_f64(1.0, 0.0)[49].is_nan());
}

// ---- XY ------------------------------------------------------------------------------------------------------

fn xy(x: usize, ys: &[usize], t0: i64, t1: i64) -> XyReq {
    XyReq { x, ys: ys.to_vec(), t0, t1, rect: None, grid: (100, 100), max_points: 1000 }
}

#[test]
fn xy_over_nothing_missing_channels_masks_and_no_pairs() {
    let data = cache();
    let p = ramp_csv(data.path(), 100);
    let o = Store::new(Some(cache().path())).open(&p, None).unwrap();
    // a span that holds no samples
    let r = o.xy_points(&xy(0, &[1], 500_000_000_000, 600_000_000_000));
    assert!(r.range.is_none() && r.series.len() == 1 && r.series[0].x.is_empty());
    // a channel past the file's own, with nothing to read it from, is all gaps: no pair, no range
    let r = o.xy_points(&xy(0, &[7], 0, 1_000_000_000));
    assert!(r.range.is_none() && r.series[0].x.is_empty());
    // the same channel read from another file's samples
    let held = |c: usize, ts: &[i64]| ts.iter().map(|t| *t as f64 / 1e6 + c as f64).collect::<Vec<f64>>();
    let r = o.xy_points_with(&xy(0, &[7], 0, 1_000_000_000), Some(&held));
    assert_eq!(r.range, Some([0.0, 99.0, 7.0, 106.0]));
    // masked samples are gaps in the line, and out of the range
    o.set_mask(1, Some(Mask::from_runs(vec![0..10, 90..100])));
    let r = o.xy_points(&xy(0, &[1], 0, 1_000_000_000));
    assert_eq!(r.range, Some([10.0, 89.0, 20.0, 178.0]));
}

// ---- widgets -------------------------------------------------------------------------------------------------

/// A growing file with no samples yet has no frame; once it has, where a state began is found by looking back.
#[test]
fn a_widgets_frame_of_a_growing_file_looks_back_for_the_run() {
    let data = cache();
    let path = data.path().join("grow.tlm");
    tlm(&path, 0, |_| 0.0);
    let store = Store::new(Some(cache().path()));
    let live = store.open_live(&path).unwrap();
    let req = [WidgetReq { channel: 0, run: true, ..Default::default() }];
    assert!(live.opened().widgets_frame(0, (0, 1_000_000_000), &req).is_none(), "no samples, no frame");
    assert!(live.opened().values_at(&[0], 0).is_none());
    append(&path, 0, 1000, |i| if i < 900 { 1.0 } else { 2.0 });
    assert!(live.refresh().unwrap());
    let f = live.opened().widgets_frame(999_000_000, (0, 1_000_000_000), &req).unwrap();
    assert_eq!((f.index, f.widgets[0].value, f.widgets[0].run_start), (999, Some(2.0), Some(900_000_000)));
    let f = live.opened().widgets_frame(500_000_000, (0, 1_000_000_000), &req).unwrap();
    assert_eq!(f.widgets[0].run_start, Some(0), "the first run starts at the first sample");
}

/// A channel that changes at every sample isn't a state: its runs aren't indexed, and the start of the current one is
/// found by looking back, which for a value that just changed is the sample itself.
#[test]
fn a_channel_with_too_many_runs_finds_a_run_start_by_looking_back() {
    let data = cache();
    let path = data.path().join("flip.tlm");
    let n = (4 << 20) + 100;
    tlm(&path, n, |i| (i % 2) as f32);
    let o = Store::new(Some(cache().path())).open(&path, None).unwrap();
    let req = [WidgetReq { channel: 0, run: true, ..Default::default() }];
    let t = (n as i64 - 1) * 1_000_000;
    let f = o.widgets_frame(t, (0, t + 1), &req).unwrap();
    assert_eq!((f.index, f.widgets[0].run_start), (n - 1, Some(t)));
    // asked again, the answer is the same (the refusal to index is remembered)
    let g = o.widgets_frame(t, (0, t + 1), &req).unwrap();
    assert_eq!(g.widgets[0].run_start, Some(t));
}

// ---- in memory -----------------------------------------------------------------------------------------------

/// A recording and its pyramid handed over in memory answer as the same recording opened from disk.
#[test]
fn a_recording_held_in_memory_answers_like_one_opened_from_disk() {
    let data = cache();
    let n = 5120;
    let p = ramp_csv(data.path(), n);
    let store = Store::new(Some(cache().path()));
    let disk = store.open(&p, None).unwrap();
    disk.build(&mut |_, _| {}).unwrap();

    let rec = tlmbench_decode::open(&p, &Default::default()).unwrap();
    let levels: Vec<Vec<Vec<tlmbench_core::pyramid::Summary>>> =
        rec.summarize_all(0, n).into_iter().map(tlmbench_core::pyramid::build_levels).collect();
    let lens = tlmbench_core::pyramid::level_lengths(n);
    let channels: Vec<Vec<u8>> =
        levels.iter().map(|ls| ls.iter().flat_map(|l| bytemuck::cast_slice::<_, u8>(l).to_vec()).collect()).collect();
    let mem = Store::in_memory(64 << 20);
    assert_eq!(mem.root(), Path::new(""));
    let held = Opened::in_memory(&mem, rec, Some(MemoryPyramid { channels, lens, done: n, gaps: Default::default() }));
    assert!(held.with_pyramid(0, |p| p.is_some()) && held.with_pyramid(1, |p| p.is_some()));
    assert_eq!(held.store().root(), Path::new(""));
    let (t0, t1) = (0, n as i64 * 1_000_000);
    let vp = Viewport { t0, t1, width_px: 40, raw_max: 0 };
    let (a, b) = (held.view(1, vp, 3.0, Precision::Exact), disk.view(1, vp, 3.0, Precision::Exact));
    assert!(a.from_pyramid && b.from_pyramid);
    assert_eq!((a.trace.t, a.trace.y), (b.trace.t, b.trace.y));
    assert_eq!(format!("{:?}", held.stats(&[0, 1], t0, t1)), format!("{:?}", disk.stats(&[0, 1], t0, t1)));

    // without a pyramid every view reads raw samples
    let bare = Opened::in_memory(&mem, tlmbench_decode::open(&p, &Default::default()).unwrap(), None);
    assert!(bare.with_pyramid(0, |p| p.is_none()));
    let v = bare.view(1, Viewport { t0, t1, width_px: 40, raw_max: 0 }, 3.0, Precision::Exact);
    assert!(!v.from_pyramid && !v.trace.y.is_empty());
}

// ---- joined --------------------------------------------------------------------------------------------------

/// A recording joined with its derived channels reads each channel from the part that has it, through every method.
#[test]
fn a_joined_recording_reads_each_channel_from_its_part() {
    let data = cache();
    let n = 3000;
    let p = ramp_csv(data.path(), n);
    let store = Store::new(Some(cache().path()));
    let base = Arc::new(store.open(&p, None).unwrap());
    let d = store.open_derived(&base, &[recipe("Sum", "a + b"), recipe("Twice", "a * 2")]).unwrap();
    let der = Arc::new(d.opened.unwrap());
    let joined = store.join(base.clone(), der.clone()).unwrap();
    assert!(joined.is_joined());
    joined.build(&mut |_, _| {}).unwrap();
    assert_eq!(joined.status(), tlmbench_store::Status::Built, "building the joined one builds both parts");

    let (j, b, d) = (joined.recording(), base.recording(), der.recording());
    assert_eq!(j.info().channels.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["a", "b", "Sum", "Twice"]);
    let held = Held::default();
    let (t0, t1) = (j.time_at(0), j.time_at(n - 1) + 1);
    let vp = Viewport { t0, t1, width_px: 60, raw_max: 0 };
    let part = |c: usize| -> (&dyn Recording, usize) { if c < 2 { (b, c) } else { (d, c - 2) } };
    let dbg = |x: &dyn std::fmt::Debug| format!("{x:?}");
    let mask = Mask::from_runs(vec![100..200, 2500..2600]);

    for c in 0..4 {
        let (r, k) = part(c);
        assert_eq!(j.read(c, 10, 90).to_f64(1.0, 0.0), r.read(k, 10, 90).to_f64(1.0, 0.0), "read {c}");
        assert_eq!(j.value_at(c, 123, &held), r.value_at(k, 123, &held), "value_at {c}");
        assert_eq!(j.scale_offset(c), r.scale_offset(k));
        assert_eq!(j.chunked(c), r.chunked(k));
        assert_eq!(dbg(&j.missing_mask(c)), dbg(&r.missing_mask(k)), "missing_mask {c}");
        assert_eq!(dbg(&j.summarize(c, 0, 1024)), dbg(&r.summarize(k, 0, 1024)), "summarize {c}");
        assert_eq!(dbg(&j.view(c, vp, None, &[], &held)), dbg(&r.view(k, vp, None, &[], &held)), "view {c}");
        assert_eq!(
            dbg(&j.view_masked(c, vp, &mask, &[], &held)),
            dbg(&r.view_masked(k, vp, &mask, &[], &held)),
            "view_masked {c}"
        );
        // the pyramids: the joined one has each part's
        let want = |f: &dyn Fn(&dyn Recording, usize, &tlmbench_core::pyramid::Pyramid) -> String| {
            let (po, kk) = if c < 2 { (&base, c) } else { (&der, c - 2) };
            po.with_pyramid(kk, |py| f(r, k, py.expect("built")))
        };
        let got = |f: &dyn Fn(&dyn Recording, usize, &tlmbench_core::pyramid::Pyramid) -> String| {
            joined.with_pyramid(c, |py| f(j, c, py.expect("built")))
        };
        let stats =
            |r: &dyn Recording, k: usize, py: &tlmbench_core::pyramid::Pyramid| dbg(&r.stats(k, py, 300, 2200, &held));
        assert_eq!(got(&stats), want(&stats), "stats {c}");
        let sm = |r: &dyn Recording, k: usize, py: &tlmbench_core::pyramid::Pyramid| {
            dbg(&r.stats_masked(k, py, 0, n, &mask, &held))
        };
        assert_eq!(got(&sm), want(&sm), "stats_masked {c}");
        let coarse = |r: &dyn Recording, k: usize, py: &tlmbench_core::pyramid::Pyramid| {
            dbg(&r.view_coarse(k, vp, py, &[], &held))
        };
        assert_eq!(got(&coarse), want(&coarse), "view_coarse {c}");
    }

    // what the parts share is the base's
    assert_eq!(j.read_times(5, 50), b.read_times(5, 50));
    assert_eq!((j.time_at(77), j.lower_bound(77_000_000)), (b.time_at(77), b.lower_bound(77_000_000)));
    assert_eq!(dbg(&j.gap_index()), dbg(&b.gap_index()));
    let (mut g1, mut g2) = (j.gap_index(), j.gap_index());
    j.extend_gap_index(&mut g1, 0);
    b.extend_gap_index(&mut g2, 0);
    assert_eq!(dbg(&g1), dbg(&g2));
    j.prefetch(&[0..10, 20..30]);
    let sums = j.summarize_all(0, 1024);
    assert_eq!(sums.len(), 4);
    assert_eq!(dbg(&sums[3]), dbg(&d.summarize(1, 0, 1024)));
    let mut id = b.layout_id();
    id.push(b'+');
    id.extend(d.layout_id());
    assert_eq!(j.layout_id(), id);
    // none of these decoders has chunked channels: asked for none of their chunks, each channel answers with none
    let chunks = j.decode_chunks(&[0, 2, 3, 1], 0..0);
    assert_eq!(chunks.len(), 4);
    assert!(chunks.iter().all(Vec::is_empty));
    // the time index is the base's (a recording without a time column keeps none)
    j.set_time_index(TimeIndex(vec![5, 6, 7]));
    assert_eq!(j.time_index(), b.time_index());
}

/// Derived channels that aren't one per sample of the recording can't be joined to it.
#[test]
fn a_recording_is_not_joined_to_channels_of_another_length() {
    let data = cache();
    let (p, q) = (ramp_csv(data.path(), 300), csv(data.path(), "short.csv", "t,a,b\n0,1,2\n0.001,2,3\n"));
    let store = Store::new(Some(cache().path()));
    let (long, short) = (Arc::new(store.open(&p, None).unwrap()), Arc::new(store.open(&q, None).unwrap()));
    let e = store.join(long, short).err().expect("a refusal");
    assert!(e.to_string().contains("300 samples but its derived channels 2"), "{e}");
}

// ---- triggers ------------------------------------------------------------------------------------------------

use tlmbench_store::trigger::{Kind, Spec};

fn trigger_spec(channel: &str, kind: Kind, level: f64, post: f64, repeat: bool) -> Spec {
    Spec { channel: channel.into(), kind, level, pre: 0.1, post, repeat }
}

/// 0 for 100 samples, then 10 for 100, and so on: it goes above any level between at samples 100, 300, 500...
/// and below it at 200, 400...
fn square(i: usize) -> f32 {
    if (i / 100) % 2 == 1 { 10.0 } else { 0.0 }
}

fn followed(dir: &Path, f: impl Fn(usize) -> f32) -> (Store, tlmbench_store::Live, PathBuf) {
    let path = dir.join("live.tlm");
    tlm(&path, 100, &f);
    let store = Store::new(Some(&dir.join("cache")));
    let live = store.open_live(&path).unwrap();
    (store, live, path)
}

fn crossings(store: &Store, live: &tlmbench_store::Live) -> Vec<(i64, String)> {
    let ev = events::list(store.root(), live.opened().identity()).unwrap();
    ev.iter().map(|e| (e.start.parse::<i64>().unwrap() / 1_000_000, e.name.clone())).collect()
}

/// Entering the region above or below a level is a crossing; each kind names itself in its event, and a level
/// that is not whole is written as it is.
#[test]
fn above_and_below_cross_where_they_enter_the_region() {
    let data = cache();
    let (store, live, path) = followed(data.path(), square);
    live.arm_trigger(trigger_spec("a", Kind::Above, 5.0, 0.05, true), &[]).unwrap();
    append(&path, 100, 700, square);
    live.refresh().unwrap();
    assert_eq!(
        crossings(&store, &live),
        [(300, "Trigger: a goes above 5".to_owned()), (500, "Trigger: a goes above 5".into())],
        "the crossing at 100 is between the samples before it was armed and the first after: not seen"
    );
    let s = live.trigger_status();
    assert!(s.armed && s.count == 2 && s.error.is_none());
    assert_eq!(s.rule.as_deref(), Some("prev(\"a\") <= 5 && \"a\" > 5"));

    let data = cache();
    let (store, live, path) = followed(data.path(), square);
    live.arm_trigger(trigger_spec("a", Kind::Below, 2.5, 0.05, true), &[]).unwrap();
    append(&path, 100, 700, square);
    live.refresh().unwrap();
    assert_eq!(
        crossings(&store, &live),
        [
            (200, "Trigger: a goes below 2.5".to_owned()),
            (400, "Trigger: a goes below 2.5".into()),
            (600, "Trigger: a goes below 2.5".into())
        ]
    );
    assert_eq!(live.trigger_status().rule.as_deref(), Some("prev(\"a\") >= 2.5 && \"a\" < 2.5"));
}

/// Crossings inside the window of the one before aren't new ones; a disarmed trigger sees nothing more.
#[test]
fn a_crossing_in_the_window_of_the_last_is_not_another_and_a_disarmed_trigger_stops() {
    let data = cache();
    let (store, live, path) = followed(data.path(), square);
    live.arm_trigger(trigger_spec("a", Kind::Above, 5.0, 0.5, true), &[]).unwrap();
    append(&path, 100, 700, square);
    live.refresh().unwrap();
    // 300 starts a half second window that holds 500 (200 ms later): only 300 counts
    assert_eq!(crossings(&store, &live).len(), 1);
    assert_eq!(live.trigger_status().count, 1);
    assert!(live.trigger_status().pending, "the window of the last crossing isn't over");

    live.disarm_trigger();
    assert!(!live.trigger_status().armed);
    append(&path, 700, 1500, square);
    live.refresh().unwrap();
    assert_eq!(live.trigger_status().count, 1);
    assert_eq!(crossings(&store, &live).len(), 1);
    // armed again: it looks from where it is
    live.arm_trigger(trigger_spec("a", Kind::Above, 5.0, 0.01, true), &[]).unwrap();
    append(&path, 1500, 1900, square);
    live.refresh().unwrap();
    assert_eq!(crossings(&store, &live).len(), 2, "the one at 1700");
}

/// Missing samples are looked past, and a number that never changes never crosses.
#[test]
fn missing_samples_are_skipped_and_a_constant_never_crosses() {
    let data = cache();
    let gappy = |i: usize| {
        if (150..160).contains(&i) {
            f32::NAN
        } else if i >= 160 {
            10.0
        } else {
            0.0
        }
    };
    let (store, live, path) = followed(data.path(), gappy);
    live.arm_trigger(trigger_spec("a", Kind::Above, 5.0, 0.05, false), &[]).unwrap();
    append(&path, 100, 300, gappy);
    live.refresh().unwrap();
    // the first value after the gap is compared with the last one before it
    assert_eq!(crossings(&store, &live), [(160, "Trigger: a goes above 5".to_owned())]);

    let data = cache();
    let (store, live, path) = followed(data.path(), |_| 1.0);
    live.arm_trigger(trigger_spec("3", Kind::Rise, 2.0, 0.05, true), &[]).unwrap();
    append(&path, 100, 300, |_| 1.0);
    live.refresh().unwrap();
    let s = live.trigger_status();
    assert!(s.armed && s.count == 0 && s.error.is_none(), "{s:?}");
    assert!(crossings(&store, &live).is_empty());
}

// ---- the recipe library --------------------------------------------------------------------------------------

use tlmbench_store::Library;

#[test]
fn the_recipe_library_names_files_renames_and_deletes() {
    let root = cache();
    let lib = Library::new(root.path());
    let dir = root.path().join("recipes");
    let r = |name: &str, expr: &str| recipe(name, expr);

    assert!(matches!(lib.save(&r("  ", "a"), None), Err(Error::Invalid(_))));
    // a name with no letters or digits still gets a file; two names that slug alike get one each
    lib.save(&r("!!!", "a + 1"), None).unwrap();
    lib.save(&r("Total Power", "a * 2"), None).unwrap();
    lib.save(&r("total-power", "a * 3"), None).unwrap();
    assert!(
        dir.join("recipe.toml").exists()
            && dir.join("total-power.toml").exists()
            && dir.join("total-power-2.toml").exists()
    );
    assert_eq!(lib.list().iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["!!!", "Total Power", "total-power"]);
    assert_eq!(lib.get("total-power").unwrap().expr, "a * 3");
    assert!(lib.get("nothing").is_none());

    // editing one in place keeps its file; renaming removes the old name's
    lib.save(&r("total-power", "a * 4"), Some("total-power")).unwrap();
    assert_eq!(lib.get("total-power").unwrap().expr, "a * 4");
    assert!(dir.join("total-power-2.toml").exists());
    lib.save(&r("Power", "a * 4"), Some("total-power")).unwrap();
    assert!(lib.get("total-power").is_none() && lib.get("Power").is_some());
    assert!(!dir.join("total-power-2.toml").exists());

    // a file of two recipes loses one, then goes with the other
    let pair = dir.join("pair.toml");
    fs::write(&pair, tlmbench_expr::recipe::to_toml(&[r("One", "a"), r("Two", "b")])).unwrap();
    assert_eq!(lib.list().len(), 5);
    lib.delete("One").unwrap();
    assert!(pair.exists() && lib.get("One").is_none() && lib.get("Two").is_some());
    lib.delete("Two").unwrap();
    assert!(!pair.exists());
    lib.delete("never-existed").unwrap();
}

#[test]
fn the_recipe_library_reports_files_it_cannot_read_and_imports_text() {
    let root = cache();
    let lib = Library::new(root.path());
    let dir = root.path().join("recipes");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("broken.toml"), "this is = not [valid").unwrap();
    fs::write(dir.join("notes.txt"), "not a recipe file").unwrap();
    assert!(lib.list().is_empty());
    let problems = lib.problems();
    assert_eq!(problems.len(), 1);
    assert!(problems[0].0.ends_with("broken.toml") && !problems[0].1.is_empty());
    // deleting skips the file it can't read; saving a recipe of the same slug as a broken file's takes another name
    lib.delete("anything").unwrap();
    lib.save(&recipe("Broken", "a"), None).unwrap();
    assert!(dir.join("broken-2.toml").exists());

    let text = tlmbench_expr::recipe::to_toml(&[recipe("Imported", "a + b"), recipe("Also", "a - b")]);
    assert_eq!(lib.import(&text).unwrap(), ["Imported", "Also"]);
    assert_eq!(lib.get("Imported").unwrap().expr, "a + b");
    assert!(lib.import("not [toml").is_err());
}

/// A recipe's preview over the first samples: one number, a series, or nothing at all.
#[test]
fn a_recipe_check_summarizes_numbers_series_and_empty_results() {
    let data = cache();
    let p = ramp_csv(data.path(), 100);
    let o = Store::new(Some(cache().path())).open(&p, None).unwrap();
    let rec = o.recording();
    let flat = tlmbench_store::check(rec, "mean(a)").unwrap();
    assert_eq!((flat.samples, flat.number), (100, Some(49.5)));
    let centred = tlmbench_store::check(rec, "a - mean(a)").unwrap();
    assert_eq!((centred.values, centred.min, centred.max, centred.number), (100, Some(-49.5), Some(49.5), None));
    assert!((centred.mean.unwrap()).abs() < 1e-9);
    let nothing = tlmbench_store::check(rec, "a / 0 - a / 0").unwrap();
    assert_eq!(
        (nothing.samples, nothing.values, nothing.min, nothing.mean, nothing.number),
        (100, 0, None, None, None)
    );
    assert!(tlmbench_store::check(rec, "a +").is_err());
    assert!(tlmbench_store::check(rec, "nosuch + 1").is_err());
}

// ---- the shared configuration read through a tree -----------------------------------------------------------

use std::collections::BTreeMap;
use tlmbench_store::config::{Tree, from_tree};

struct Mem(BTreeMap<&'static str, &'static str>);

impl Tree for Mem {
    fn text(&self, rel: &str) -> Option<String> {
        self.0.get(rel).map(|s| s.to_string())
    }
    fn dirs(&self, rel: &str) -> Vec<String> {
        let prefix = format!("{rel}/");
        let mut v: Vec<String> = self
            .0
            .keys()
            .filter_map(|k| k.strip_prefix(&prefix))
            .filter_map(|k| k.split_once('/').map(|(d, _)| d.to_owned()))
            .collect();
        v.dedup();
        v
    }
    fn files(&self, rel: &str, ext: &str) -> Vec<String> {
        let prefix = format!("{rel}/");
        self.0
            .keys()
            .filter_map(|k| k.strip_prefix(&prefix))
            .filter(|k| !k.contains('/') && k.ends_with(ext))
            .map(str::to_owned)
            .collect()
    }
}

#[test]
fn a_folder_read_through_a_tree_gives_categories_widgets_and_icons_and_skips_the_rest() {
    let tree = Mem(BTreeMap::from([
        (
            "categories.toml",
            "schema = 1\n[[category]]\nid = \"fault\"\nname = \"Fault\"\ncolor = \"#f00\"\nicon = \"warn\"\n",
        ),
        ("icons/warn.svg", "<svg/>"),
        ("icons/notes.txt", "not an icon"),
        // a code widget with its inputs and parameters
        (
            "widgets/attitude/widget.toml",
            "schema = 1\nname = \"Attitude\"\nkind = \"code\"\nentry = \"main.js\"\nby = \"me\"\nicon = \"i.svg\"\n\
             [[input]]\nrole = \"pitch\"\nlabel = \"Pitch\"\n[[input]]\nrole = \"roll\"\n\
             [[param]]\nname = \"max\"\ntype = \"number\"\ndefault = \"10\"\n[[param]]\nname = \"mode\"\ntype = \"choice\"\nchoices = [\"a\", \"b\"]\n",
        ),
        ("widgets/attitude/main.js", "export default 1"),
        ("widgets/attitude/i.svg", "<svg id='i'/>"),
        // an svg widget with an angle, whose icon can't be a path
        (
            "widgets/dial/widget.toml",
            "schema = 1\nname = \"Dial\"\nkind = \"svg\"\nentry = \"dial.svg\"\nangle = [-135.0, 135.0]\nicon = \"../x.svg\"\n",
        ),
        ("widgets/dial/dial.svg", "<svg id='dial'/>"),
        // the ways a widget is refused
        ("widgets/nofile/other.txt", "x"),
        ("widgets/badtoml/widget.toml", "not [toml"),
        ("widgets/oldschema/widget.toml", "schema = 9\nname = \"Old\"\nkind = \"svg\"\nentry = \"a.svg\"\n"),
        ("widgets/oldschema/a.svg", "<svg/>"),
        ("widgets/slash/widget.toml", "schema = 1\nname = \"S\"\nkind = \"svg\"\nentry = \"sub/a.svg\"\n"),
        ("widgets/dotted/widget.toml", "schema = 1\nname = \"D\"\nkind = \"svg\"\nentry = \".hidden.svg\"\n"),
        ("widgets/noentry/widget.toml", "schema = 1\nname = \"N\"\nkind = \"svg\"\nentry = \"missing.svg\"\n"),
        ("widgets/oddkind/widget.toml", "schema = 1\nname = \"O\"\nkind = \"python\"\nentry = \"a.py\"\n"),
        ("widgets/oddkind/a.py", "print(1)"),
    ]));
    let got = from_tree(&tree);
    assert_eq!(got.categories.len(), 1);
    assert_eq!(
        (got.categories[0].id.as_str(), got.categories[0].icon.as_deref(), got.categories[0].show_on_charts),
        ("fault", Some("warn"), true)
    );
    assert_eq!(got.icons.len(), 1);
    assert_eq!((got.icons[0].name.as_str(), got.icons[0].svg.as_str()), ("warn", "<svg/>"));
    let ids: Vec<&str> = got.widgets.iter().map(|w| w.id.as_str()).collect();
    assert_eq!(ids, ["attitude", "dial"], "only the readable ones");
    let att = &got.widgets[0];
    assert_eq!(
        (att.name.as_str(), att.by.as_deref(), att.code.as_deref(), att.svg.as_deref()),
        ("Attitude", Some("me"), Some("export default 1"), None)
    );
    assert_eq!(att.icon.as_deref(), Some("<svg id='i'/>"));
    assert_eq!(
        att.inputs.iter().map(|i| (i.role.as_str(), i.label.as_deref())).collect::<Vec<_>>(),
        [("pitch", Some("Pitch")), ("roll", None)]
    );
    assert_eq!(
        (att.params[0].name.as_str(), att.params[0].kind.as_str(), att.params[0].default.as_deref()),
        ("max", "number", Some("10"))
    );
    assert_eq!(att.params[1].choices, ["a", "b"]);
    let dial = &got.widgets[1];
    assert_eq!(
        (dial.svg.as_deref(), dial.angle, dial.icon.as_deref(), dial.code.as_deref()),
        (Some("<svg id='dial'/>"), Some([-135.0, 135.0]), None, None)
    );

    // a categories file of another schema, or damaged, gives none
    let old = Mem(BTreeMap::from([(
        "categories.toml",
        "schema = 2\n[[category]]\nid = \"a\"\nname = \"A\"\ncolor = \"#fff\"\n",
    )]));
    assert!(from_tree(&old).categories.is_empty());
    let bad = Mem(BTreeMap::from([("categories.toml", "[[category")]));
    assert!(from_tree(&bad).categories.is_empty());
}

// ---- the pyramid cache ---------------------------------------------------------------------------------------

/// `t,a,b` with times that are not at a steady rate, so the time is a stored column (and has a `times.bin`).
fn uneven_csv(dir: &Path) -> PathBuf {
    let mut t = String::from("t,a,b\n");
    for i in 0..3000usize {
        t.push_str(&format!("{},{},{}\n", i * i % 10_007 + i * 10_007, i, 2 * i));
    }
    csv(dir, "uneven.csv", &t)
}

/// The cache entry of `path` in `root`, built.
fn built_entry(root: &Path, path: &Path) -> PathBuf {
    let o = Store::new(Some(root)).open(path, None).unwrap();
    o.build(&mut |_, _| {}).unwrap();
    root.join("pyr/v2").join(o.key())
}

/// A cache entry that was cut short, changed or is missing a file is refused with what is wrong, never read.
#[test]
fn a_damaged_cache_entry_is_refused() {
    let data = cache();
    let path = uneven_csv(data.path());
    type Damage = (&'static str, fn(&Path), &'static str);
    let damages: [Damage; 5] = [
        (
            "c0.pyr cut short",
            |d| fs::OpenOptions::new().write(true).open(d.join("c0.pyr")).unwrap().set_len(10).unwrap(),
            "expected",
        ),
        (
            "manifest of another recording",
            |d| {
                let text = fs::read_to_string(d.join("manifest.json")).unwrap();
                let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
                v["samples"] = 7.into();
                fs::write(d.join("manifest.json"), v.to_string()).unwrap();
            },
            "manifest doesn't match",
        ),
        ("gaps.bin of the wrong size", |d| fs::write(d.join("gaps.bin"), [1, 2, 3]).unwrap(), "wrong size"),
        (
            "times.bin of the wrong size",
            |d| {
                let mut f = fs::OpenOptions::new().append(true).open(d.join("times.bin")).unwrap();
                f.write_all(&[0; 8]).unwrap();
            },
            "wrong size",
        ),
        ("c0.pyr missing", |d| fs::remove_file(d.join("c0.pyr")).unwrap(), "c0.pyr"),
    ];
    for (what, damage, why) in damages {
        let root = cache();
        let dir = built_entry(root.path(), &path);
        assert!(dir.join("times.bin").exists(), "a stored time column has times.bin");
        damage(&dir);
        let e = Store::new(Some(root.path())).open(&path, None).err().unwrap_or_else(|| panic!("{what}: opened"));
        assert!(matches!(e, Error::Damaged { .. } | Error::Io { .. }), "{what}: {e}");
        assert!(e.to_string().contains(why), "{what}: {e}");
    }
}

/// An entry that turns out damaged by the time it is built is made again.
#[test]
fn a_build_replaces_an_entry_that_was_damaged_meanwhile() {
    let data = cache();
    let path = uneven_csv(data.path());
    let root = cache();
    let store = Store::new(Some(root.path()));
    let waiting = store.open(&path, None).unwrap(); // nothing built yet
    assert_eq!(waiting.status(), tlmbench_store::Status::Missing);
    let dir = built_entry(root.path(), &path); // another process builds it...
    fs::OpenOptions::new().write(true).open(dir.join("c1.pyr")).unwrap().set_len(10).unwrap(); // ...and it is damaged
    waiting.build(&mut |_, _| {}).unwrap();
    assert_eq!(waiting.status(), tlmbench_store::Status::Built);
    let again = Store::new(Some(root.path())).open(&path, None).unwrap();
    assert_eq!(again.status(), tlmbench_store::Status::Built, "the entry on disk is whole again");
    assert_eq!(
        format!("{:?}", again.stats(&[0, 1], 0, i64::MAX)),
        format!("{:?}", waiting.stats(&[0, 1], 0, i64::MAX))
    );
}

/// While another process builds an entry, a build here follows it: it ends when the entry is whole, or when
/// cancelled, and it never builds the same entry twice.
#[test]
fn a_build_waits_for_another_process_and_follows_it() {
    let data = cache();
    let path = uneven_csv(data.path());
    let ref_cache = cache();
    let reference = built_entry(ref_cache.path(), &path); // the entry as another process would leave it

    let root = cache();
    let o = Arc::new(Store::new(Some(root.path())).open(&path, None).unwrap());
    let lock_path = root.path().join("pyr/v2").join(format!("{}.lock", o.key()));
    let lock = fs::File::options().create(true).truncate(false).write(true).open(&lock_path).unwrap();
    lock.lock().unwrap(); // "the other process" is building

    // cancelled while waiting: it returns, and nothing is built
    let waiter = {
        let o = o.clone();
        std::thread::spawn(move || o.build(&mut |_, _| {}))
    };
    std::thread::sleep(std::time::Duration::from_millis(300));
    o.cancel();
    waiter.join().unwrap().unwrap();
    assert_eq!(o.status(), tlmbench_store::Status::Missing);
    assert!(!root.path().join("pyr/v2").join(o.key()).join("manifest.json").exists());
    drop(o);

    // the other process finishes: the entry appears whole, and the waiting build takes it as built
    let o = Arc::new(Store::new(Some(root.path())).open(&path, None).unwrap());
    let waiter = {
        let o = o.clone();
        std::thread::spawn(move || o.build(&mut |_, _| {}))
    };
    std::thread::sleep(std::time::Duration::from_millis(300));
    let dest = root.path().join("pyr/v2").join(o.key());
    fs::create_dir_all(&dest).unwrap();
    for f in ["c0.pyr", "c1.pyr", "gaps.bin", "times.bin", "manifest.json"] {
        fs::copy(reference.join(f), dest.join(f)).unwrap();
    }
    waiter.join().unwrap().unwrap();
    assert_eq!(o.status(), tlmbench_store::Status::Built);
    drop(lock);
}
