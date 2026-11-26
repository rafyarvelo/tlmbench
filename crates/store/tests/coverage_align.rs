//! The alignment methods' refusals and notes (coverage, R6c.3, Nov 26): a file with no samples, no shared channel, a
//! channel that matches nothing, files that don't overlap, the plugin path with a fake estimator, and how an offset
//! is worded. The success paths are in `group.rs`.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use tlmbench_core::timemap::TimeMap;
use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec};
use tlmbench_decode::reference;
use tlmbench_store::align::{
    EstimatorPlugin, Method, PLUGIN_SAMPLES, PluginEstimate, Series, estimate, estimators, set_estimators,
};
use tlmbench_store::{Opened, Store};

fn write(path: &Path, name: &str, rate_hz: f64, values: &[f32]) {
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
        time: TimeSpec::Rate { rate_hz, t0_ns: 0 },
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
    };
    let mut f = fs::File::create(path).unwrap();
    reference::write_header(&mut f, &d).unwrap();
    f.write_all(&values.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>()).unwrap();
}

struct Pair {
    dir: tempfile::TempDir,
    a: Opened,
    b: Opened,
}

fn pair(an: &str, av: &[f32], bn: &str, bv: &[f32], hz: f64) -> Pair {
    let dir = tempfile::tempdir().unwrap();
    let (pa, pb) = (dir.path().join("a.tlm"), dir.path().join("b.tlm"));
    write(&pa, an, hz, av);
    write(&pb, bn, hz, bv);
    let store = Store::new(Some(&dir.path().join("cache")));
    let (a, b) = (store.open(&pa, None).unwrap(), store.open(&pb, None).unwrap());
    Pair { dir, a, b }
}

fn err(p: &Pair, m: Method) -> String {
    estimate(p.dir.path(), &p.a, &TimeMap::identity(), &p.b, &m).unwrap_err().to_string()
}

fn ramp(n: usize) -> Vec<f32> {
    (0..n).map(|i| i as f32).collect()
}

#[test]
fn a_file_with_no_samples_cannot_be_aligned() {
    let p = pair("a", &ramp(100), "a", &[], 1000.0);
    for m in [
        Method::Starts,
        Method::Clock,
        Method::Manual { seconds: 1.0 },
        Method::Xcorr { channel: None, windows: None },
        Method::TimeSource { channel: None },
    ] {
        assert!(err(&p, m).contains("no samples"), "a file with nothing in it");
    }
}

#[test]
fn matching_refuses_files_that_share_no_channel_or_no_event_name() {
    let p = pair("a", &ramp(2000), "b", &ramp(2000), 1000.0);
    let none = "No channel with the same name in both files";
    assert!(err(&p, Method::Xcorr { channel: None, windows: None }).contains(none));
    assert!(err(&p, Method::TimeSource { channel: None }).contains(none));
    assert!(
        err(&p, Method::Xcorr { channel: Some("a".into()), windows: None })
            .contains("Both files need a channel named a")
    );
    assert!(err(&p, Method::Events { name: None }).contains("No event with the same name in both files"));
}

#[test]
fn a_channel_that_matches_nothing_is_refused_with_its_correlation() {
    // a slow sine against pseudo-random noise: no peak
    let sine: Vec<f32> = (0..20_000).map(|i| (i as f32 * 0.01).sin()).collect();
    let mut x = 12345u32;
    let noise: Vec<f32> = (0..20_000)
        .map(|_| {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (x >> 16) as f32 / 65_536.0 - 0.5
        })
        .collect();
    let p = pair("a", &sine, "a", &noise, 1000.0);
    let e = err(&p, Method::Xcorr { channel: Some("a".into()), windows: None });
    assert!(e.starts_with("No clear match on a (r = "), "{e}");
}

#[test]
fn a_time_channel_that_never_overlaps_is_refused() {
    // a reads 0..2000, b reads 5000..7000: nothing of b's values is in a's range
    let b: Vec<f32> = (0..2000).map(|i| 5000.0 + i as f32).collect();
    let p = pair("t", &ramp(2000), "t", &b, 1000.0);
    assert!(err(&p, Method::TimeSource { channel: Some("t".into()) }).contains("The files' t values don't overlap"));
}

/// The note says how far the file moved, in the unit that reads best; a file that moved back says so with a minus.
#[test]
fn an_offset_is_worded_in_milliseconds_seconds_minutes_or_hours() {
    // value v is read at own time v/hz seconds, so b's values being `shift` above a's puts b `shift/hz` s earlier
    // in a's clock: the shift of the time channel is +shift/hz
    let note = |hz: f64, shift: f32, n: usize, rn: usize| {
        let a: Vec<f32> = (0..rn).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..n).map(|i| shift + i as f32).collect();
        let p = pair("t", &a, "t", &b, hz);
        estimate(p.dir.path(), &p.a, &TimeMap::identity(), &p.b, &Method::TimeSource { channel: None })
            .unwrap()
            .note
            .unwrap()
    };
    assert_eq!(note(1000.0, 500.0, 2000, 4000), "t: +500.0 ms");
    assert_eq!(note(1000.0, 1500.0, 2000, 6000), "t: +1.50 s");
    assert_eq!(note(1000.0, 90_000.0, 2000, 100_000), "t: +1.50 min");
    assert_eq!(note(1.0, 7200.0, 2000, 10_000), "t: +2.00 h");
    // b's values below a's: a minus sign
    assert_eq!(note(1000.0, -500.0, 2000, 4000), "t: −500.0 ms");
}

#[derive(Debug)]
struct Fake {
    result: std::result::Result<PluginEstimate, String>,
    needs: std::result::Result<Vec<String>, String>,
    /// How many samples of the first channel the plugin was given, reference then target.
    seen: std::sync::Mutex<Vec<(usize, usize)>>,
}

impl EstimatorPlugin for Fake {
    fn name(&self) -> &str {
        "fake"
    }
    fn needs(&self, _: &str) -> std::result::Result<Vec<String>, String> {
        self.needs.clone()
    }
    fn estimate(
        &self,
        reference: &[Series],
        target: &[Series],
        _: &TimeMap,
        _: &str,
    ) -> std::result::Result<PluginEstimate, String> {
        // the target's first time is on its own clock, the reference's on the shared one (through the map)
        assert_eq!((reference.len(), target.len()), (1, 1));
        assert_eq!(reference[0].t[0], 10_000_000_000, "the reference is read through its map");
        assert_eq!(target[0].t[0], 0);
        self.seen.lock().unwrap().push((reference[0].t.len(), target[0].t.len()));
        self.result.clone()
    }
}

/// One test only, because the list of estimators is the process's: a fake plugin through every way the core can
/// refuse or accept what a plugin says.
#[test]
fn an_estimator_plugin_is_found_by_name_asked_what_it_needs_and_its_answer_checked() {
    let p = pair("t", &ramp(100), "t", &ramp(100), 1000.0);
    let rmap = TimeMap::offset(10_000_000_000);
    let run = |name: &str| {
        estimate(p.dir.path(), &p.a, &rmap, &p.b, &Method::Plugin { name: name.into(), params: "{}".into() })
    };
    set_estimators(Vec::new());
    assert!(estimators().is_empty());
    assert!(run("fake").unwrap_err().to_string().contains("No alignment plugin named fake"));

    let good = PluginEstimate {
        knots: vec![(0, 5), (1000, 1005)],
        residuals: vec![(0, 0.5)],
        quality: 0.75,
        note: Some("found it".into()),
    };
    let with = |result, needs| set_estimators(vec![Arc::new(Fake { result, needs, seen: Default::default() })]);

    with(Ok(good.clone()), Ok(vec!["t".into()]));
    assert_eq!(estimators().len(), 1);
    let got = run("fake").unwrap();
    assert_eq!((got.map.apply(0), got.map.apply(1000)), (5, 1005));
    assert_eq!((got.quality, got.note.as_deref(), got.channel.clone()), (0.75, Some("found it"), None));
    assert_eq!(got.residuals, vec![(0, 0.5)]);

    with(Ok(good.clone()), Err("needs a number".into()));
    assert_eq!(run("fake").unwrap_err().to_string(), "fake: needs a number");
    with(Ok(good.clone()), Ok(vec!["nope".into()]));
    assert!(run("fake").unwrap_err().to_string().contains("Both files need a channel named nope"));
    with(Err("no luck".into()), Ok(vec!["t".into()]));
    assert_eq!(run("fake").unwrap_err().to_string(), "fake: no luck");
    let broken = PluginEstimate { knots: vec![(5, 0), (1, 1)], ..good.clone() };
    with(Ok(broken), Ok(vec!["t".into()]));
    assert!(run("fake").unwrap_err().to_string().starts_with("fake returned a bad map"));

    // a file with nothing in it is refused before the plugin runs
    let empty = pair("t", &ramp(100), "t", &[], 1000.0);
    with(Ok(good.clone()), Ok(vec!["t".into()]));
    let e = estimate(
        empty.dir.path(),
        &empty.a,
        &rmap,
        &empty.b,
        &Method::Plugin { name: "fake".into(), params: String::new() },
    );
    assert!(e.unwrap_err().to_string().contains("no samples"));

    // a channel of more than PLUGIN_SAMPLES samples is thinned to that many, evenly spread
    let big = PLUGIN_SAMPLES + 1;
    let long = pair("t", &ramp(big), "t", &ramp(100), 1000.0);
    let fake = Arc::new(Fake { result: Ok(good), needs: Ok(vec!["t".into()]), seen: Default::default() });
    set_estimators(vec![fake.clone()]);
    estimate(long.dir.path(), &long.a, &rmap, &long.b, &Method::Plugin { name: "fake".into(), params: String::new() })
        .unwrap();
    assert_eq!(*fake.seen.lock().unwrap(), vec![(PLUGIN_SAMPLES, 100)]);
    set_estimators(Vec::new());
}

/// Sync pairs that can't make a map, and events that pair up to one that can't.
#[test]
fn sync_pairs_and_paired_events_that_make_no_map_are_refused() {
    use tlmbench_store::events::{self, Event};
    let p = pair("a", &ramp(2000), "a", &ramp(2000), 1000.0);
    let pairs = |v: &[(i64, i64)]| Method::Sync {
        pairs: v.iter().map(|&(a, b)| (a.to_string(), b.to_string())).collect(),
        piecewise: false,
    };
    assert!(!err(&p, pairs(&[])).is_empty(), "no pairs");
    assert!(!err(&p, pairs(&[(5, 1), (5, 9)])).is_empty(), "one file time twice");
    assert!(!err(&p, pairs(&[(0, 10), (10, 0)])).is_empty(), "time running backwards");
    // two events of the same name at one moment in the target pair to one file time twice
    let ev = |id: &str, t: i64| Event {
        id: id.into(),
        name: "Sync".into(),
        start: t.to_string(),
        duration: "0".into(),
        source: "user".into(),
        ..Default::default()
    };
    for (o, times) in [(&p.a, [2_000_000_000, 12_000_000_000]), (&p.b, [1_000_000_000, 1_000_000_000])] {
        for (k, t) in times.into_iter().enumerate() {
            events::save(p.dir.path(), o.identity(), ev(&format!("e{k}"), t)).unwrap();
        }
    }
    assert!(!err(&p, Method::Events { name: Some("Sync".into()) }).is_empty());
}

/// A damaged events file for a recording is an error of the events method, not an empty list.
#[test]
fn a_damaged_event_file_stops_matching_on_events() {
    let p = pair("a", &ramp(100), "a", &ramp(100), 1000.0);
    let dir = p.dir.path().join("events");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{}.json", p.a.identity())), "{").unwrap();
    assert!(!err(&p, Method::Events { name: None }).is_empty());
}

/// One sample can't be matched; a target longer than two minutes is matched in three windows when none is asked for.
#[test]
fn one_sample_is_not_enough_and_a_long_file_is_matched_in_three_windows() {
    let one = pair("a", &[1.0], "a", &[1.0], 1000.0);
    assert!(err(&one, Method::Xcorr { channel: None, windows: None }).contains("Not enough samples to match"));

    let sig = |i: usize| {
        let s = i as f64 / 1000.0;
        (((0.3 * s + 0.002 * s * s) * std::f64::consts::TAU).sin() * 5.0) as f32
    };
    let v: Vec<f32> = (0..130_000).map(sig).collect();
    let p = pair("a", &v, "a", &v, 1000.0);
    let run = |w| {
        estimate(p.dir.path(), &p.a, &TimeMap::identity(), &p.b, &Method::Xcorr { channel: None, windows: w }).unwrap()
    };
    let (auto, one_window) = (run(None), run(Some(1)));
    assert_eq!(auto.residuals.len(), 3, "a window at each third of the file");
    assert_eq!(one_window.residuals.len(), 1);
    assert!(auto.note.as_deref().unwrap().starts_with("Matched on a: "), "{:?}", auto.note);
    assert!(auto.quality > 0.9, "{}", auto.quality);
    // the same file against itself moves nothing
    for t in [0, 60_000_000_000, 120_000_000_000] {
        assert!((auto.map.apply(t) - t).abs() < 2_000_000, "{t}");
    }
}
