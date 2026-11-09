//! Derived channels end to end: recipes evaluated chunk by chunk into a cache entry that opens like a recording.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use tlmbench_core::Viewport;
use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec};
use tlmbench_decode::reference;
use tlmbench_expr::{Recipe, eval_chunked, parse};
use tlmbench_store::{Precision, RecEnv, Store};

/// A little over a chunk (2^20 samples), so evaluation runs in two.
const N: usize = (1 << 20) + 54_321;

fn value(c: usize, i: usize) -> f32 {
    if (700_000..700_050).contains(&i) {
        return f32::NAN;
    }
    (i as f32 * 0.003 * (c + 1) as f32).sin() * 5.0 + ((i * 7919 + c) % 101) as f32 * 0.01
}

fn recording(dir: &Path) -> PathBuf {
    let field = |name: &str, offset| Field {
        name: name.into(),
        dtype: DType::F32,
        offset,
        unit: Some("V".into()),
        scale: 1.0,
        value_offset: 0.0,
        bits: None,
    };
    let desc = Descriptor {
        sequence: None,
        files: Vec::new(),
        packet_types: Vec::new(),
        packets: None,
        schema: SCHEMA,
        name: None,
        endian: Endian::Little,
        header_size: 0,
        record_size: 8,
        time: TimeSpec::Rate { rate_hz: 1000.0, t0_ns: 1_700_000_000_000_000_000 },
        fields: vec![field("a", 0), field("b", 4)],
    };
    let path = dir.join("rec.tlm");
    let mut w = std::io::BufWriter::new(fs::File::create(&path).unwrap());
    reference::write_header(&mut w, &desc).unwrap();
    for i in 0..N {
        w.write_all(&value(0, i).to_le_bytes()).unwrap();
        w.write_all(&value(1, i).to_le_bytes()).unwrap();
    }
    w.flush().unwrap();
    path
}

fn recipe(name: &str, expr: &str, unit: Option<&str>) -> Recipe {
    Recipe {
        name: name.into(),
        expr: expr.into(),
        unit: unit.map(Into::into),
        category: Some("Derived".into()),
        decimals: Some(2),
        automatic: true,
    }
}

fn column(o: &tlmbench_store::Opened, c: usize) -> Vec<f64> {
    o.recording().read(c, 0, o.recording().info().samples).to_f64(1.0, 0.0)
}

#[test]
fn derived_channels_match_the_engine_and_reopen_from_the_cache() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    fs::create_dir(&data).unwrap();
    let src = recording(&data);
    let store = Store::new(Some(&tmp.path().join("cache")));
    let base = store.open(&src, None).unwrap();

    let recipes = [
        recipe("Sum", "a + b", Some("V")),
        recipe("Smooth", "sma(a, 50)", None),
        recipe("Mean a", "mean(a)", None),
        recipe("Low pass", "lowpass(a, 20)", None),
        recipe("Centred", "(a - mean(a)) / std(a)", None),
        recipe("Smooth squared", "Smooth ^ 2", None), // uses another recipe
        recipe("Needs c", "a + c", None),
    ];
    let d = store.open_derived(&base, &recipes).unwrap();
    let o = d.opened.as_ref().unwrap();
    assert_eq!(d.names, ["Sum", "Smooth", "Mean a", "Low pass", "Centred", "Smooth squared"]);
    assert_eq!(d.skipped.len(), 1);
    assert_eq!((d.skipped[0].0.as_str(), d.skipped[0].1.msg.as_str()), ("Needs c", "There's no channel called c"));

    let info = o.recording().info();
    assert_eq!((info.samples, info.format.as_str()), (N, "Derived"));
    assert_eq!(info.t_first, base.recording().info().t_first);
    assert_eq!(info.t_last, base.recording().info().t_last);
    assert_eq!(info.channels[0].unit.as_deref(), Some("V"));
    assert_eq!(info.channels[0].category.as_deref(), Some("Derived"));

    // every channel is what the engine gives in one go over the whole recording
    let env = RecEnv::new(base.recording());
    for (k, r) in recipes[..6].iter().enumerate() {
        let src = if r.name == "Smooth squared" { "sma(a, 50) ^ 2" } else { &r.expr };
        let whole = eval_chunked(&parse(src).unwrap(), &env, usize::MAX).unwrap();
        let got = column(o, k);
        let want: Vec<f64> = match &whole {
            tlmbench_expr::Val::Ser(s) => s.v.clone(),
            v => vec![v.number().unwrap(); N],
        };
        assert_eq!(got.len(), want.len(), "{}", r.name);
        for i in (0..N).step_by(997).chain([0, N - 1, 700_000, 700_049, 1 << 20]) {
            let (g, w) = (got[i], want[i]);
            assert!(
                (g.is_nan() && w.is_nan()) || (g - w).abs() <= 1e-9 * w.abs().max(1.0),
                "{} at {i}: {g} vs {w}",
                r.name
            );
        }
    }
    // a one-number recipe is a flat line; the gap shows as a gap in what's computed from a
    assert!(column(o, 2).windows(2).all(|w| w[0] == w[1]));
    assert!(column(o, 1)[700_010].is_nan());

    // the derived recording has a pyramid and draws like any channel
    o.build(&mut |_, _| {}).unwrap();
    let vp = Viewport { t0: info.t_first.unwrap(), t1: info.t_last.unwrap(), width_px: 400, raw_max: 0 };
    let v = o.view(1, vp, 3.0, Precision::Exact);
    assert!(!v.trace.y.is_empty());

    // reopening reads the cache: nothing is rebuilt
    let entry = store.root().join("derived");
    let dirs: Vec<_> = fs::read_dir(&entry).unwrap().flatten().collect();
    assert_eq!(dirs.len(), 1);
    let manifest = dirs[0].path().join("manifest.json");
    let before = fs::metadata(&manifest).unwrap().modified().unwrap();
    drop(d);
    let again = store.open_derived(&base, &recipes).unwrap();
    assert_eq!(fs::metadata(&manifest).unwrap().modified().unwrap(), before);
    assert_eq!(again.names.len(), 6);

    // changing one recipe is a new entry, the old one stays until evicted
    let mut changed = recipes.to_vec();
    changed[0].expr = "a - b".into();
    let third = store.open_derived(&base, &changed).unwrap();
    assert_eq!(fs::read_dir(&entry).unwrap().count(), 2);
    assert_ne!(column(third.opened.as_ref().unwrap(), 0)[10], column(again.opened.as_ref().unwrap(), 0)[10]);

    // nothing was written beside the source
    assert_eq!(fs::read_dir(&data).unwrap().count(), 1);
}

#[test]
fn a_recording_with_a_time_column_keeps_its_times() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("p.csv");
    let mut text = String::from("time,v,i\n");
    let mut t = 0.0f64;
    for k in 0..5000 {
        t += 0.01 + (k % 7) as f64 * 0.001;
        text.push_str(&format!("{t:.4},{},{}\n", k % 13, 2 + k % 3));
    }
    fs::write(&src, text).unwrap();
    let store = Store::new(Some(&tmp.path().join("cache")));
    let base = store.open(&src, None).unwrap();
    let d = store
        .open_derived(
            &base,
            &[
                recipe("P", "v * i", Some("W")),
                recipe("Rate of v", "rate(v)", None),
                recipe("Slow", "sma(v, 0.5s)", None),
            ],
        )
        .unwrap();
    let o = d.opened.unwrap();
    let (b, r) = (base.recording(), o.recording());
    assert_eq!(r.read_times(0, 5000), b.read_times(0, 5000));
    let p = r.read(0, 0, 5000).to_f64(1.0, 0.0);
    let v = b.read(0, 0, 5000).to_f64(1.0, 0.0);
    let i = b.read(1, 0, 5000).to_f64(1.0, 0.0);
    assert!((0..5000).all(|k| p[k] == v[k] * i[k]));
    // rate() uses the real intervals: dv / dt between neighbouring samples
    let rate = r.read(1, 0, 5000).to_f64(1.0, 0.0);
    let times = b.read_times(0, 5000);
    let k = 100;
    let want = (v[k] - v[k - 1]) / ((times[k] - times[k - 1]) as f64 / 1e9);
    assert!((rate[k] - want).abs() < 1e-6 * want.abs().max(1.0), "{} vs {want}", rate[k]);
}
