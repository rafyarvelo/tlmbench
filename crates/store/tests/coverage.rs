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
