//! Following a growing recording with derived channels, and a trigger that can't run (coverage, R6c.3, Nov 26).

use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec};
use tlmbench_decode::reference;
use tlmbench_expr::Recipe;
use tlmbench_store::Store;
use tlmbench_store::trigger::{Kind, Spec};

fn recipe(name: &str, expr: &str) -> Recipe {
    Recipe { name: name.into(), expr: expr.into(), unit: None, category: None, decimals: None, automatic: true }
}

fn tlm(path: &Path, n: usize) {
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
        fields: vec![Field {
            name: "a".into(),
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
        w.write_all(&(i as f32).to_le_bytes()).unwrap();
    }
    w.flush().unwrap();
}

fn append(path: &Path, from: usize, to: usize) {
    let mut f = fs::OpenOptions::new().append(true).open(path).unwrap();
    for i in from..to {
        f.write_all(&(i as f32).to_le_bytes()).unwrap();
    }
}

/// Recipes over a live file: those that can follow are computed as samples arrive (a number is a flat line, a mistake
/// is missing samples), those that need the whole recording or a channel that isn't there are skipped with why.
#[test]
fn derived_channels_follow_a_growing_file_and_skip_what_cannot_follow() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("live.tlm");
    tlm(&path, 50);
    let store = Store::new(Some(&d.path().join("cache")));
    let live = Arc::new(store.open_live(&path).unwrap());
    assert_eq!(live.path(), path);
    let recipes = [
        recipe("Twice", "a * 2"),
        recipe("Smooth", "rolling_mean(a, 4)"),
        recipe("Const", "40 + 2"),
        recipe("Broken", "past_warn(a)"),
        recipe("Total", "cumsum(a)"),
        recipe("Peak", "max(a)"),
        recipe("Nothing", "nothere * 2"),
    ];
    let o = store.open_live_derived(live.clone(), &recipes).unwrap();
    assert_eq!(o.names, ["Twice", "Smooth", "Const", "Broken"]);
    let why = |n: &str| o.skipped.iter().find(|s| s.0 == n).map(|s| s.1.clone()).unwrap_or_default();
    assert!(why("Total").contains("can't follow a live source"), "{:?}", o.skipped);
    assert!(why("Peak").contains("can't follow a live source"));
    assert!(why("Nothing").contains("nothere"), "{:?}", o.skipped);
    let d = o.live;
    assert!(Arc::ptr_eq(d.base(), &live));

    append(&path, 50, 120);
    assert!(d.refresh().unwrap());
    let joined = d.opened();
    let rec = joined.recording();
    let info = rec.info();
    assert_eq!(info.samples, 120);
    let col = |name: &str| {
        let c = info.channels.iter().position(|c| c.name == name).unwrap();
        rec.read(c, 0, 120).to_f64(1.0, 0.0)
    };
    assert_eq!(col("a")[119], 119.0);
    assert_eq!(col("Twice")[119], 238.0);
    assert!(col("Const").iter().all(|&x| x == 42.0));
    assert!(col("Broken").iter().all(|x| x.is_nan()), "a mistake leaves the samples missing");
    let smooth = col("Smooth");
    assert!((smooth[119] - (116.0 + 117.0 + 118.0 + 119.0) / 4.0).abs() < 1e-9, "{}", smooth[119]);
    // nothing new: the same recording, not grown
    assert!(!d.refresh().unwrap());
    assert_eq!(d.opened().recording().info().samples, 120);
}

/// A trigger on a channel that can't be evaluated live is refused, and one that fails while it runs stops and says why.
#[test]
fn a_trigger_that_cannot_run_live_says_why() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("live.tlm");
    tlm(&path, 50);
    let store = Store::new(Some(&d.path().join("cache")));
    let live = store.open_live(&path).unwrap();
    let spec = |channel: &str| Spec {
        channel: channel.into(),
        kind: Kind::Above,
        level: 5.0,
        pre: 0.1,
        post: 0.1,
        repeat: true,
    };
    let e = live.arm_trigger(spec("cumsum(a)"), &[]).unwrap_err();
    assert!(e.contains("needs the whole recording"), "{e}");
    // an expression that parses and resolves but has nothing to look at: it stops, armed no more
    live.arm_trigger(spec("past_warn(a)"), &[]).unwrap();
    assert!(live.trigger_status().armed);
    append(&path, 50, 100);
    live.refresh().unwrap();
    let st = live.trigger_status();
    assert!(!st.armed);
    assert_eq!(st.error.as_deref(), Some("a has no warn limits"));
    // a number never crosses
    live.arm_trigger(spec("max(a)"), &[]).unwrap_err();
    live.arm_trigger(spec("40 + 2"), &[]).unwrap();
    append(&path, 100, 150);
    live.refresh().unwrap();
    let st = live.trigger_status();
    assert!(st.armed && st.count == 0 && st.error.is_none(), "{st:?}");
}
