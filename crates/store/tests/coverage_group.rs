//! A group's less-travelled paths (coverage, R6c.3, Nov 26): a file with no samples among files with some, requests that
//! reach only some files, a file partly built, and a held column over a long span.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use tlmbench_core::Viewport;
use tlmbench_core::timemap::TimeMap;
use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec};
use tlmbench_decode::reference;
use tlmbench_store::group::Group;
use tlmbench_store::widgets::WidgetReq;
use tlmbench_store::{Precision, Request, Status, Store};

fn write(path: &Path, name: &str, rate_hz: f64, n: usize, f: impl Fn(usize) -> f32) {
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
    let mut w = std::io::BufWriter::new(fs::File::create(path).unwrap());
    reference::write_header(&mut w, &d).unwrap();
    for i in 0..n {
        w.write_all(&f(i).to_le_bytes()).unwrap();
    }
    w.flush().unwrap();
}

const S: i64 = 1_000_000_000;

#[test]
fn a_file_with_no_samples_does_not_move_the_timeline_and_requests_reach_only_their_files() {
    let d = tempfile::tempdir().unwrap();
    let (pa, pb, pc) = (d.path().join("a.tlm"), d.path().join("b.tlm"), d.path().join("c.tlm"));
    write(&pa, "x", 1000.0, 2000, |i| i as f32); // 0..2 s
    write(&pb, "y", 1000.0, 0, |_| 0.0);
    write(&pc, "z", 100.0, 500, |i| 100.0 + i as f32); // 5 s, placed from 10 s
    let store = Store::new(Some(&d.path().join("cache")));
    let open = |p: &Path| Arc::new(store.open(p, None).unwrap());
    let g = Group::new(
        vec![(open(&pa), TimeMap::identity()), (open(&pb), TimeMap::identity()), (open(&pc), TimeMap::offset(10 * S))],
        vec![1, 2, 3],
    );
    // the empty file adds nothing; the timeline is from a's start to c's end
    let (t0, t1) = g.bounds_now();
    assert_eq!(t0, Some(0));
    assert!((t1.unwrap() - 14_990_000_000).abs() < 20_000_000, "{t1:?}");
    // a map that changes moves the end
    g.set_map(2, TimeMap::offset(20 * S));
    assert!((g.bounds_now().1.unwrap() - 24_990_000_000).abs() < 20_000_000);
    g.set_map(9, TimeMap::identity()); // no such file: nothing happens
    g.set_map(2, TimeMap::offset(10 * S));
    assert_eq!((g.route(0), g.route(1), g.route(2), g.route(3)), (Some((0, 0)), Some((1, 0)), Some((2, 0)), None));

    // a request for z alone and one for x alone: each goes to its own file only
    let vp = |a: i64, b: i64| Viewport { t0: a, t1: b, width_px: 100, raw_max: 0 };
    let req = |channels: Vec<usize>, a, b| Request { channels, gap_factors: vec![], vp: vp(a, b), connect_gaps: false };
    let out = g.view_batch(&[req(vec![2], 10 * S, 15 * S), req(vec![0], 0, 2 * S)], Precision::Exact);
    assert_eq!((out[0].len(), out[0][0].channel), (1, 2));
    assert_eq!((out[1].len(), out[1][0].channel), (1, 0));
    assert!(out[0][0].trace.t.iter().all(|&t| (10 * S..=15 * S).contains(&t)), "on the shared timeline");
    assert!(out[1][0].trace.t.iter().all(|&t| t < 2 * S + 1));
    // readings: x at 1 s, z at 12 s through its map, the empty file's channel gives nothing, one out of range too
    let at = g.readings(&[0, 2, 9], 12 * S);
    assert_eq!(at[0].as_ref().map(|r| r.value), Some(1999.0), "held at its last sample");
    assert_eq!(at[1].as_ref().map(|r| r.index), Some(200));
    assert!(at[2].is_none());
    let none = g.readings(&[1], 5 * S);
    assert!(none[0].is_none(), "b has no sample to give");
    // widgets of one file only: the frame is that file's
    let frame = g
        .widgets_frame(12 * S, (10 * S, 15 * S), &[WidgetReq { channel: 2, minmax: false, run: false, trace: 0 }])
        .unwrap();
    assert_eq!(frame.index, 200);
    assert_eq!(frame.widgets[0].value, Some(300.0));
}

/// A file part-way through its build still answers (its built part from summaries), reports its progress, builds the
/// rest on request, and a column held onto times over more than a million samples is read sample by sample.
#[test]
fn a_file_partly_built_reports_progress_and_a_long_held_column_is_read_by_sample() {
    let d = tempfile::tempdir().unwrap();
    let (pa, pb) = (d.path().join("a.tlm"), d.path().join("b.tlm"));
    let n = (1usize << 20) + (1 << 19); // a block and a half
    write(&pa, "x", 1000.0, 3000, |i| i as f32);
    write(&pb, "y", 1000.0, n, |i| (i % 1000) as f32);
    let store = Store::new(Some(&d.path().join("cache")));
    let a = Arc::new(store.open(&pa, None).unwrap());
    let b = Arc::new(store.open(&pb, None).unwrap());
    let g = Group::new(vec![(a, TimeMap::identity()), (b.clone(), TimeMap::offset(S))], vec![1, 2]);
    // stop b's build after its first block
    let blocks = std::cell::Cell::new(0);
    b.build(&mut |done, _| {
        if done > 0 {
            blocks.set(blocks.get() + 1);
        }
        if blocks.get() == 1 {
            b.cancel();
        }
    })
    .unwrap();
    assert_eq!(b.status(), Status::Building { done: 1 << 20, total: n as u64 });
    match g.status() {
        Status::Building { done, total } => assert_eq!((done, total), (1 << 20, 3000 + n as u64)),
        other => panic!("{other:?}"),
    }
    // the part built answers from its summaries; the answer is on the shared timeline
    let vp = Viewport { t0: S, t1: S + 1_000 * S, width_px: 100, raw_max: 0 };
    let out =
        g.view_batch(&[Request { channels: vec![1], gap_factors: vec![], vp, connect_gaps: false }], Precision::Exact);
    let built_until = out[0][0].built_until.expect("not all of it is built");
    // b is placed 1 s along; its first million samples are built, so the line stops at 1 s + 1048.575 s
    assert!((built_until - (S + 1_048_575_000_000)).abs() <= 2_000_000, "{built_until}");
    // a held column over the whole of b: from the first sample to the last, a million samples apart
    let times = [S + 500_000_000, S + 1_400_700_000_000];
    let col = g.held_column(1, &times);
    assert_eq!(col, [500.0, 700.0]);
    // a build that was cancelled is picked up by the next handle on the file (a cancelled one stays cancelled)
    drop(g);
    let b2 = Arc::new(store.open(&pb, None).unwrap());
    let a2 = Arc::new(store.open(&pa, None).unwrap());
    let g = Group::new(vec![(a2, TimeMap::identity()), (b2, TimeMap::offset(S))], vec![1, 2]);
    let mut last = (0, 0);
    let mut starts = None;
    g.build(&mut |d, t| {
        starts.get_or_insert(d);
        last = (d, t);
    })
    .unwrap();
    assert_eq!(g.status(), Status::Built);
    assert_eq!(last.1, 3000 + n as u64, "progress counts both files together");
    assert!(starts.unwrap() <= 3000 + (1 << 20) as u64);
}
