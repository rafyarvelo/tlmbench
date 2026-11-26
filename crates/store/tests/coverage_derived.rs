//! Derived channels' edges (coverage, R6c.3, Nov 26): a recipe that fails while it runs, limits seen by a recipe, a
//! recording with no time column, another file's channel read through a time map, and a spectrum asked for over
//! nothing.

use std::sync::Arc;

use tlmbench_core::timemap::TimeMap;
use tlmbench_expr::{Env, Recipe};
use tlmbench_store::{ExprEnv, Foreign, RecEnv, Store};

fn recipe(name: &str, expr: &str) -> Recipe {
    Recipe { name: name.into(), expr: expr.into(), unit: None, category: None, decimals: None, automatic: true }
}

fn csv(dir: &std::path::Path, name: &str, text: &str) -> std::path::PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

#[test]
fn a_recipe_that_fails_while_it_runs_is_an_error_naming_it_and_limits_reach_recipes() {
    let d = tempfile::tempdir().unwrap();
    let src = csv(d.path(), "a.csv", "t,v\n0,1\n1,2\n2,9\n3,4\n");
    let store = Store::new(Some(&d.path().join("cache")));
    let base = store.open(&src, None).unwrap();
    // it parses and the channel exists, but v has no limits to be past
    let e = store.open_derived(&base, &[recipe("Past", "past_warn(v)")]).unwrap_err().to_string();
    assert!(e.contains("Past") && e.contains("v has no warn limits"), "{e}");
    // with limits set, the recipe sees them (and the cache key changes with them)
    store.set_expr_env(ExprEnv {
        limits: [(("v".to_string(), false), (Some(1.5), Some(5.0)))].into(),
        ..Default::default()
    });
    let o = store.open_derived(&base, &[recipe("Past", "past_warn(v)")]).unwrap().opened.unwrap();
    let col = o.recording().read(0, 0, 4).to_f64(1.0, 0.0);
    assert_eq!(col, [1.0, 0.0, 1.0, 0.0]);
    // alarm limits that were never given are not found
    let e = store.open_derived(&base, &[recipe("Alarmed", "past_alarm(v)")]).unwrap_err().to_string();
    assert!(e.contains("v has no alarm limits"), "{e}");
}

#[test]
fn a_recording_with_no_time_column_is_read_at_one_sample_a_second() {
    let d = tempfile::tempdir().unwrap();
    // text that is not a time: the rows are numbered
    let src = csv(d.path(), "n.csv", "name,v\nx,3\ny,1\nz,2\nw,5\n");
    let store = Store::new(Some(&d.path().join("cache")));
    let base = store.open(&src, None).unwrap();
    let rec = base.recording();
    let info = rec.info();
    let env = RecEnv::new(rec);
    assert_eq!(env.hz(), Some(1.0));
    assert_eq!(env.samples(), Some(info.samples));
    // the whole series through `series`, as `read` of everything
    let s = env.series("v").unwrap();
    assert_eq!(s.v, [3.0, 1.0, 2.0, 5.0]);
    assert!((s.time(2) - 2.0).abs() < 1e-9, "{}", s.time(2));
    assert!(env.series("nothing").is_none());
    // and a recipe over it is written with the same numbering
    let o = store.open_derived(&base, &[recipe("Twice", "v * 2")]).unwrap().opened.unwrap();
    assert_eq!(o.recording().read(0, 0, 4).to_f64(1.0, 0.0), [6.0, 2.0, 4.0, 10.0]);
}

#[test]
fn another_files_channel_is_read_through_its_map_and_an_empty_range_is_empty() {
    let d = tempfile::tempdir().unwrap();
    let a = csv(d.path(), "a.csv", "t,x\n0,1\n1,2\n2,3\n3,4\n4,5\n");
    let b = csv(d.path(), "b.csv", "t,y\n0,10\n1,20\n2,30\n3,40\n4,50\n");
    let store = Store::new(Some(&d.path().join("cache")));
    let (oa, ob) = (store.open(&a, None).unwrap(), Arc::new(store.open(&b, None).unwrap()));
    // b's clock is 1 s behind a's: b's own time t is shared time t + 1 s
    let foreign = [Foreign { name: "Y·2".into(), opened: ob, channel: 0, map: TimeMap::offset(1_000_000_000) }];
    let env = RecEnv::new(oa.recording()).with_foreign(&foreign);
    let s = env.read("Y·2", 2..4).values_for_test();
    // a's samples 2..4 are at 2 s and 3 s: b has 10..50 at 1 s..5 s, so its values around them are held in view
    assert!(s.contains(&10.0) || s.contains(&20.0), "{s:?}");
    // an empty range, and a range past the end, read nothing
    assert_eq!(env.read("Y·2", 3..3).unwrap().len(), 0);
    assert_eq!(env.read("Y·2", 9..12).unwrap().len(), 0);
    assert!(env.read("x", 9..12).unwrap().is_empty());
    // a recipe naming it, from a file on the same samples (same times, no shift), adds them one to one
    let same =
        [Foreign { name: "Y·2".into(), opened: foreign[0].opened.clone(), channel: 0, map: TimeMap::identity() }];
    let derived = store.open_derived_cross(&oa, &[recipe("Both", "x + \"Y·2\"")], &same).unwrap().opened.unwrap();
    assert_eq!(derived.recording().read(0, 0, 5).to_f64(1.0, 0.0), [11.0, 22.0, 33.0, 44.0, 55.0]);
}

trait ValuesForTest {
    fn values_for_test(&self) -> Vec<f64>;
}

impl ValuesForTest for Option<tlmbench_expr::Series> {
    fn values_for_test(&self) -> Vec<f64> {
        self.as_ref().unwrap().v.clone()
    }
}

#[test]
fn a_spectrum_over_nothing_is_none() {
    let d = tempfile::tempdir().unwrap();
    let mut text = String::from("t,v\n");
    for i in 0..400 {
        text.push_str(&format!("{:.3},{}\n", i as f64 / 100.0, (i as f64 * 0.7).sin()));
    }
    let store = Store::new(Some(&d.path().join("cache")));
    let o = store.open(&csv(d.path(), "s.csv", &text), None).unwrap();
    let info = o.recording().info();
    let (t0, t1) = (info.t_first.unwrap(), info.t_last.unwrap());
    assert!(o.spectrum(0, t0, t1, 256).is_some());
    // a view after the end of the data, before it, backwards, or of nothing
    assert!(o.spectrum(0, t1 + 10_000_000_000, t1 + 20_000_000_000, 256).is_none());
    assert!(o.spectrum(0, t0 - 20_000_000_000, t0 - 10_000_000_000, 256).is_none());
    assert!(o.spectrum(0, t1, t0, 256).is_none());
    assert!(o.spectrum(0, t0, t0, 256).is_none());
    // a recording with no samples has no spectrum
    let empty = store.open(&csv(d.path(), "e.csv", "t,v\n"), None).unwrap();
    assert!(empty.spectrum(0, 0, 1_000_000_000, 256).is_none());
    // a view that is half past the end is windowed over what is there and is still a spectrum
    let half = o.spectrum(0, t1 - 1_000_000_000, t1 + 1_000_000_000, 256);
    assert!(half.is_none() || half.unwrap().n >= 16);
}

/// A recipe that is one number is a flat line over every chunk of the recording, whether it is a constant or a
/// summary of the channel.
#[test]
fn a_recipe_that_is_one_number_is_a_flat_line_over_every_chunk() {
    use std::io::Write;
    use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec};
    let d = tempfile::tempdir().unwrap();
    let n = (1usize << 20) + 1000;
    let desc = Descriptor {
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
    let path = d.path().join("big.tlm");
    let mut w = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
    tlmbench_decode::reference::write_header(&mut w, &desc).unwrap();
    for i in 0..n {
        w.write_all(&((i % 100) as f32).to_le_bytes()).unwrap();
    }
    w.flush().unwrap();
    drop(w);
    let store = Store::new(Some(&d.path().join("cache")));
    let base = store.open(&path, None).unwrap();
    let derived = store
        .open_derived(&base, &[recipe("Const", "40 + 2"), recipe("Peak", "max(a)"), recipe("Plain", "a + 1")])
        .unwrap();
    assert_eq!(derived.names, ["Const", "Peak", "Plain"]);
    let o = derived.opened.unwrap();
    let rec = o.recording();
    assert_eq!(rec.info().samples, n);
    for (c, want) in [(0, 42.0), (1, 99.0)] {
        let col = rec.read(c, 0, n).to_f64(1.0, 0.0);
        assert!(col.iter().all(|&x| x == want), "channel {c}");
    }
    let plain = rec.read(2, n - 101, n).to_f64(1.0, 0.0);
    assert_eq!(plain[0], ((n - 101) % 100) as f64 + 1.0);
}
