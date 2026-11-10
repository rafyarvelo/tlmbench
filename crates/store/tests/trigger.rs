//! Live triggers: a crossing in the samples that arrive is an event, found on the channel or on an expression of it
//! without looking at the whole session again.

use std::fs;
use std::io::Write;
use std::path::Path;

use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec};
use tlmbench_decode::reference;
use tlmbench_store::trigger::{Kind, Spec};
use tlmbench_store::{Live, Store, events};

/// One f32 channel `a` at 1 kHz: sin(i / 100) × 5, which falls through 0 at about sample 314 and rises through it
/// at about 628, then every 628 after.
fn descriptor() -> Descriptor {
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
            name: "a".into(),
            dtype: DType::F32,
            offset: 0,
            unit: None,
            scale: 1.0,
            value_offset: 0.0,
            bits: None,
        }],
    }
}

fn append(path: &Path, from: usize, to: usize) {
    let mut f = fs::OpenOptions::new().append(true).open(path).unwrap();
    let buf: Vec<u8> = (from..to).flat_map(|i| ((i as f32 / 100.0).sin() * 5.0).to_le_bytes()).collect();
    f.write_all(&buf).unwrap();
}

fn live(dir: &Path) -> (Store, Live, std::path::PathBuf) {
    let path = dir.join("live.tlm");
    let mut f = fs::File::create(&path).unwrap();
    reference::write_header(&mut f, &descriptor()).unwrap();
    drop(f);
    let store = Store::new(Some(&dir.join("cache")));
    append(&path, 0, 100);
    let live = store.open_live(&path).unwrap();
    (store, live, path)
}

fn spec(channel: &str, kind: Kind, level: f64, repeat: bool) -> Spec {
    Spec { channel: channel.into(), kind, level, pre: 0.1, post: 0.1, repeat }
}

fn events_of(store: &Store, live: &Live) -> Vec<events::Event> {
    events::list(store.root(), live.opened().identity()).unwrap()
}

/// The sample a crossing is at, from its event.
fn at(e: &events::Event) -> i64 {
    e.start.parse::<i64>().unwrap() / 1_000_000
}

#[test]
fn a_single_trigger_fires_once_and_adds_an_event() {
    let dir = tempfile::tempdir().unwrap();
    let (store, live, path) = live(dir.path());
    live.arm_trigger(spec("a", Kind::Rise, 0.0, false), &[]).unwrap();
    assert!(live.trigger_status().armed);
    for (from, to) in [(100, 400), (400, 1500), (1500, 3000)] {
        append(&path, from, to);
        live.refresh().unwrap();
    }
    let ev = events_of(&store, &live);
    assert_eq!(ev.len(), 1, "{ev:?}");
    assert!((at(&ev[0]) - 628).abs() <= 1, "{}", ev[0].start);
    assert_eq!((ev[0].category.as_str(), ev[0].source.as_str()), ("trigger", "rule"));
    assert_eq!(ev[0].name, "Trigger: a rises through 0");
    let s = live.trigger_status();
    assert!(!s.armed && s.count == 1 && !s.pending, "{s:?}");
}

#[test]
fn repeat_fires_each_time_and_a_crossing_inside_the_window_isnt_a_second() {
    let dir = tempfile::tempdir().unwrap();
    let (store, live, path) = live(dir.path());
    // a wide window: every second crossing of a noisy level would fall in the first's
    let mut sp = spec("a", Kind::Fall, 0.0, true);
    sp.post = 0.1;
    live.arm_trigger(sp, &[]).unwrap();
    append(&path, 100, 2200);
    live.refresh().unwrap();
    let ev = events_of(&store, &live);
    // falls at about 314, 942 and 1571, 2199
    assert_eq!(ev.iter().map(at).map(|t| (t as f64 / 314.159).round() as i32).collect::<Vec<_>>(), [1, 3, 5]);
    assert!(live.trigger_status().armed);
    assert_eq!(live.trigger_status().count, 3);
}

#[test]
fn it_reads_an_expression_with_a_window_over_the_new_samples_only() {
    let dir = tempfile::tempdir().unwrap();
    let (store, live, path) = live(dir.path());
    // the mean of the last 50 ms lags the signal by about 25 samples
    live.arm_trigger(spec("rolling_mean(a, 50ms)", Kind::Rise, 0.0, false), &[]).unwrap();
    for (from, to) in [(100, 700), (700, 701), (701, 900)] {
        append(&path, from, to);
        live.refresh().unwrap();
    }
    let ev = events_of(&store, &live);
    assert_eq!(ev.len(), 1);
    assert!((at(&ev[0]) - 653).abs() <= 3, "{}", ev[0].start);
}

#[test]
fn above_and_below_fire_on_entering_the_region() {
    let dir = tempfile::tempdir().unwrap();
    let (store, live, path) = live(dir.path());
    live.arm_trigger(spec("a", Kind::Below, -4.0, false), &[]).unwrap();
    append(&path, 100, 800);
    live.refresh().unwrap();
    let ev = events_of(&store, &live);
    // sin(x) × 5 passes −4 going down at x = π + asin(0.8) ≈ 4.07
    assert_eq!(ev.len(), 1);
    assert!((at(&ev[0]) - 407).abs() <= 2, "{}", ev[0].start);
}

#[test]
fn what_cant_be_computed_in_a_window_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (_, live, _) = live(dir.path());
    let e = live.arm_trigger(spec("cumsum(a)", Kind::Rise, 0.0, false), &[]).unwrap_err();
    assert!(e.contains("can't trigger live"), "{e}");
    let e = live.arm_trigger(spec("nothing", Kind::Rise, 0.0, false), &[]).unwrap_err();
    assert!(e.contains("nothing"), "{e}");
    assert!(!live.trigger_status().armed);
}
