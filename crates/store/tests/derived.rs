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

#[test]
fn spectrum_of_a_channel_over_the_time_view() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("p.csv");
    // 200 Hz, a 12.5 Hz tone of amplitude 3 on an offset of 40; irregular times keep the same mean rate
    let mut text = String::from("time,v\n");
    for i in 0..8000 {
        let t = i as f64 / 200.0;
        text.push_str(&format!("{t:.3},{}\n", 40.0 + 3.0 * (2.0 * std::f64::consts::PI * 12.5 * t).sin()));
    }
    std::fs::write(&src, text).unwrap();
    let store = Store::new(Some(&tmp.path().join("cache")));
    let o = store.open(&src, None).unwrap();
    let info = o.recording().info();
    let (t0, t1) = (info.t_first.unwrap(), info.t_last.unwrap());
    let s = o.spectrum(0, t0, t1, 16384).unwrap();
    assert_eq!(s.n, 4096);
    assert!((s.peak_hz - 12.5).abs() < 0.1, "{}", s.peak_hz);
    assert!(s.peak_amp > 2.5 && s.peak_amp < 3.2, "{}", s.peak_amp);
    // a short view has fewer points; under 16 samples there is nothing to show
    let one_second = (t0 + 5_000_000_000, t0 + 6_000_000_000);
    assert_eq!(o.spectrum(0, one_second.0, one_second.1, 16384).unwrap().n, 128);
    assert!(o.spectrum(0, t0, t0 + 50_000_000, 16384).is_none());
    assert_eq!(o.spectrum(0, t0, t1, 1024).unwrap().n, 1024);
}

#[test]
fn a_recording_joins_its_derived_channels_without_rebuilding_a_pyramid() {
    use std::sync::Arc;
    use tlmbench_core::table::{Col, Filter, SortKey};
    use tlmbench_store::Status;
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("data");
    fs::create_dir(&data).unwrap();
    let src = recording(&data);
    let store = Store::new(Some(&tmp.path().join("cache")));
    let base = Arc::new(store.open(&src, None).unwrap());
    let d =
        store.open_derived(&base, &[recipe("Sum", "a + b", Some("V")), recipe("Slow", "sma(a, 100)", None)]).unwrap();
    let joined = store.join(base.clone(), Arc::new(d.opened.unwrap())).unwrap();
    assert!(joined.is_joined());
    let names: Vec<&str> = joined.recording().info().channels.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["a", "b", "Sum", "Slow"]);
    assert_eq!(joined.status(), Status::Missing);

    // building the joined recording builds each part's pyramid (nothing of its own) and puts them side by side
    joined.build(&mut |_, _| {}).unwrap();
    assert_eq!(joined.status(), Status::Built);
    let pyramids =
        fs::read_dir(store.root().join("pyr").join("v2")).unwrap().flatten().filter(|e| e.path().is_dir()).count();
    assert_eq!(pyramids, 2, "the recording's and the derived entry's");

    let info = joined.recording().info();
    let vp = Viewport { t0: info.t_first.unwrap(), t1: info.t_last.unwrap(), width_px: 300, raw_max: 0 };
    let views = joined.views(&[0, 2, 3], vp, &[3.0; 3], Precision::Exact);
    assert_eq!(views.iter().map(|v| v.channel).collect::<Vec<_>>(), [0, 2, 3]);
    assert!(views.iter().all(|v| !v.trace.y.is_empty()));

    // stats of a derived channel come from its own pyramid and agree with the part opened alone
    let both = joined.stats(&[1, 2], vp.t0, vp.t1 + 1).unwrap();
    let alone = base.stats(&[1], vp.t0, vp.t1 + 1).unwrap();
    assert_eq!(both[0].count, alone[0].count);
    assert!(both[1].count > 0 && (both[1].mean - (both[0].mean + 0.0)).abs() < 100.0);
    let (_, _, v) = joined.values_at(&[0, 1, 2], vp.t0 + 5_000_000_000).unwrap();
    assert!((v[2] - (v[0] + v[1])).abs() < 1e-3, "{v:?}");

    // a table filters on a derived column and shows a raw one
    let rows = joined.select_rows(
        (vp.t0, vp.t1 + 1),
        vec![(Col::Channel(2), Filter::parse("> 9").unwrap().unwrap())],
        vec![SortKey { col: Col::Channel(2), descending: true }],
        &|_| true,
    );
    let (first, rows) = rows.unwrap();
    assert!(!rows.ids.is_empty());
    let top = first + rows.ids[0] as usize;
    assert!(joined.recording().value_at(2, top, &Default::default()) > 9.0);
    let at = |c: usize| joined.recording().value_at(c, top, &Default::default());
    let (a, b, sum) = (at(0), at(1), at(2));
    assert!((sum - (a + b)).abs() < 1e-3, "{a} + {b} = {sum}");
}

#[test]
fn the_recipe_library_keeps_one_file_per_recipe() {
    use tlmbench_store::Library;
    let tmp = tempfile::tempdir().unwrap();
    let lib = Library::new(tmp.path());
    assert!(lib.list().is_empty());
    lib.save(&recipe("Power", "battery_v * current_a", Some("W")), None).unwrap();
    lib.save(&recipe("Accel magnitude", "sqrt(ax^2 + ay^2)", Some("g")), None).unwrap();
    lib.save(&recipe("Accel  magnitude!", "other", None), None).unwrap(); // the same file name stem, another recipe
    let names: Vec<String> = lib.list().into_iter().map(|r| r.name).collect();
    assert_eq!(names, ["Accel  magnitude!", "Accel magnitude", "Power"]);
    let files: Vec<String> = fs::read_dir(tmp.path().join("recipes"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(files.len(), 3, "{files:?}");
    assert!(files.contains(&"power.toml".to_string()));
    // the file is the shared format
    let text = fs::read_to_string(tmp.path().join("recipes/power.toml")).unwrap();
    assert!(
        text.contains("schema = 1") && text.contains("[[recipe]]") && text.contains("expr = \"battery_v * current_a\""),
        "{text}"
    );

    // editing in place, renaming (the old file goes), deleting
    let mut p = lib.get("Power").unwrap();
    p.expr = "battery_v * current_a / 1000".into();
    lib.save(&p, Some("Power")).unwrap();
    assert_eq!(lib.get("Power").unwrap().expr, "battery_v * current_a / 1000");
    p.name = "Power (kW)".into();
    lib.save(&p, Some("Power")).unwrap();
    assert!(lib.get("Power").is_none() && lib.get("Power (kW)").is_some());
    assert_eq!(lib.list().len(), 3);
    lib.delete("Power (kW)").unwrap();
    assert_eq!(lib.list().len(), 2);
    assert!(lib.save(&recipe("  ", "x", None), None).is_err());

    // export and import round trip through the shared text
    let text = lib.export(&["Accel magnitude".into()]);
    let other = Library::new(&tmp.path().join("other"));
    assert_eq!(other.import(&text).unwrap(), ["Accel magnitude"]);
    assert_eq!(other.get("Accel magnitude").unwrap(), lib.get("Accel magnitude").unwrap());
    assert!(other.import("schema = 9").is_err());
    // a damaged file is skipped by list and reported
    fs::write(tmp.path().join("recipes/broken.toml"), "not toml [").unwrap();
    assert_eq!(lib.list().len(), 2);
    assert_eq!(lib.problems().len(), 1);
}

#[test]
fn the_dialog_checks_an_expression_and_previews_it() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("p.csv");
    let mut text = String::from("time,v,i\n");
    for k in 0..1000 {
        text.push_str(&format!("{k},{},{}\n", k % 10, if k == 5 { String::new() } else { "2".to_string() }));
    }
    fs::write(&src, text).unwrap();
    let store = Store::new(Some(&tmp.path().join("cache")));
    let o = store.open(&src, None).unwrap();
    let rec = o.recording();
    let p = tlmbench_store::check(rec, "v * i").unwrap();
    assert_eq!((p.samples, p.values, p.min, p.max), (1000, 999, Some(0.0), Some(18.0)));
    assert!((p.mean.unwrap() - 9.0).abs() < 0.1 && p.number.is_none());
    // one number
    let p = tlmbench_store::check(rec, "max(v)").unwrap();
    assert_eq!((p.number, p.min), (Some(9.0), Some(9.0)));
    // errors name the character
    let e = tlmbench_store::check(rec, "v * (i").unwrap_err();
    assert_eq!((e.msg.as_str(), e.at), ("Expected \")\"", 6));
    assert_eq!(tlmbench_store::check(rec, "nope + 1").unwrap_err().msg, "There's no channel called nope");
    assert!(tlmbench_store::check(rec, "lowpass(v, 5000)").unwrap_err().msg.contains("below half the sample rate"));
    // a rolling function over a window works on the preview as it will on the file
    assert_eq!(tlmbench_store::check(rec, "sma(v, 10)").unwrap().values, 991);
}
