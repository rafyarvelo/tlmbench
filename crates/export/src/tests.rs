use std::fs;
use std::path::Path;

use tlmbench_core::Recording;
use tlmbench_core::chunks::Held;
use tlmbench_decode::OpenOptions;

use super::*;

fn open(path: &Path, cache: &Path) -> Box<dyn Recording> {
    tlmbench_decode::open(
        path,
        &OpenOptions {
            descriptor: None,
            part: None,
            profile: None,
            cache_dir: Some(cache.to_owned()),
            adjust: Vec::new(),
        },
    )
    .unwrap()
}

/// Every channel's shown values and the times of `r`, for comparing round trips.
fn contents(r: &dyn Recording) -> (Vec<Timestamp>, Vec<Vec<f64>>) {
    let n = r.info().samples;
    let h = Held::default();
    let values = (0..r.info().channels.len()).map(|c| (0..n).map(|i| r.value_at(c, i, &h)).collect()).collect();
    (r.read_times(0, n), values)
}

fn same(a: &[Vec<f64>], b: &[Vec<f64>]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| x.len() == y.len() && x.iter().zip(y).all(|(p, q)| p == q || p.is_nan() && q.is_nan()))
}

/// A big-endian fixed-width file with a scaled i16 and an f32 (NaN in places), 150k records at 1 kHz from a date.
fn scaled_file(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let desc = dir.join("bench.toml");
    fs::write(
        &desc,
        "schema = 1\nendian = \"big\"\nrecord_size = 6\n[time]\nsource = \"rate\"\nrate_hz = 1000.0\n\
         t0_ns = 1793491200000000000\n[[fields]]\nname = \"volts\"\ntype = \"i16\"\noffset = 0\nunit = \"V\"\n\
         scale = 0.001\n[[fields]]\nname = \"temp\"\ntype = \"f32\"\noffset = 2\n",
    )
    .unwrap();
    let mut bytes = Vec::new();
    for i in 0..150_000u32 {
        bytes.extend_from_slice(&((i % 2000) as i16 - 1000).to_be_bytes());
        let t = if i % 997 == 0 { f32::NAN } else { i as f32 * 0.1 };
        bytes.extend_from_slice(&t.to_be_bytes());
    }
    let data = dir.join("bench.bin");
    fs::write(&data, bytes).unwrap();
    (data, desc)
}

#[test]
fn csv_and_parquet_read_back_exactly() {
    let tmp = tempfile::tempdir().unwrap();
    let (data, desc) = scaled_file(tmp.path());
    let cache = tmp.path().join("cache");
    let src = tlmbench_decode::open(
        &data,
        &OpenOptions {
            descriptor: Some(desc),
            part: None,
            profile: None,
            cache_dir: Some(cache.clone()),
            adjust: Vec::new(),
        },
    )
    .unwrap();
    let (times, values) = contents(src.as_ref());
    for format in [Format::Csv, Format::Parquet] {
        let out = tmp.path().join(format!("out.{}", format.extension()));
        let w = export(src.as_ref(), &Options::new(format, vec![0, 1], 0, usize::MAX), &out).unwrap();
        assert_eq!(w.rows, 150_000);
        let back = open(&out, &cache);
        let names: Vec<_> = back.info().channels.iter().map(|c| (c.name.clone(), c.unit.clone())).collect();
        assert_eq!(names, [("volts".to_owned(), Some("V".to_owned())), ("temp".to_owned(), None)], "{format:?}");
        let (t2, v2) = contents(back.as_ref());
        assert_eq!(t2, times, "{format:?}");
        // CSV reads every number as f64: the f32 column's shortest text is the f32, so compare as f32
        let v2: Vec<Vec<f64>> = match format {
            Format::Csv => vec![v2[0].clone(), v2[1].iter().map(|&v| v as f32 as f64).collect()],
            _ => v2,
        };
        assert!(same(&v2, &values), "{format:?}");
        let manifest = fs::read_to_string(&w.manifest).unwrap();
        assert_eq!(manifest.lines().nth(1), Some(&*format!("volts,V,f64,,out.{}", format.extension())));
    }
    // no temporary files left
    assert!(fs::read_dir(tmp.path()).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().starts_with('.')));
}

#[test]
fn raw_keeps_stored_types_and_a_span() {
    let tmp = tempfile::tempdir().unwrap();
    let (data, desc) = scaled_file(tmp.path());
    let cache = tmp.path().join("cache");
    let src = tlmbench_decode::open(
        &data,
        &OpenOptions {
            descriptor: Some(desc),
            part: None,
            profile: None,
            cache_dir: Some(cache.clone()),
            adjust: Vec::new(),
        },
    )
    .unwrap();
    let out = tmp.path().join("raw.parquet");
    let mut o = Options::new(Format::Parquet, vec![0], 1000, 3000);
    o.raw = true;
    export(src.as_ref(), &o, &out).unwrap();
    let back = open(&out, &cache);
    assert_eq!(back.info().channels[0].dtype, "i16");
    assert_eq!(back.info().samples, 2000);
    assert_eq!(back.info().t_first, Some(src.time_at(1000)));
    assert_eq!(back.value_at(0, 0, &Held::default()), 0.0); // (1000 % 2000) - 1000
}

#[test]
fn long_layout_and_arrow() {
    let tmp = tempfile::tempdir().unwrap();
    let csv = tmp.path().join("in.csv");
    fs::write(&csv, "t,a,b [m]\n0,1,2\n0.5,3,\n").unwrap();
    let cache = tmp.path().join("cache");
    let src = open(&csv, &cache);
    let out = tmp.path().join("long.csv");
    let mut o = Options::new(Format::Csv, vec![0, 1], 0, usize::MAX);
    o.layout = Layout::Long;
    export(src.as_ref(), &o, &out).unwrap();
    assert_eq!(fs::read_to_string(&out).unwrap(), "time,channel,value\n0,a,1\n0,b,2\n0.5,a,3\n0.5,b,\n");

    let arrow = tmp.path().join("w.arrow");
    export(src.as_ref(), &Options::new(Format::Arrow, vec![0, 1], 0, usize::MAX), &arrow).unwrap();
    let r = arrow_ipc::reader::FileReader::try_new(fs::File::open(&arrow).unwrap(), None).unwrap();
    let schema = r.schema();
    assert_eq!(schema.field(0).name(), "time_ns");
    assert_eq!(schema.field(2).metadata().get("unit").map(String::as_str), Some("m"));
    let batches: Vec<_> = r.map(|b| b.unwrap()).collect();
    assert_eq!(batches.iter().map(|b| b.num_rows()).sum::<usize>(), 2);
}

#[test]
fn resampling_holds_the_last_value() {
    let tmp = tempfile::tempdir().unwrap();
    let csv = tmp.path().join("in.csv");
    fs::write(&csv, "t,v\n0,1\n0.25,2\n1,3\n").unwrap();
    let cache = tmp.path().join("cache");
    let src = open(&csv, &cache);
    let out = tmp.path().join("grid.csv");
    let mut o = Options::new(Format::Csv, vec![0], 0, usize::MAX);
    o.resample_hz = Some(2.0);
    export(src.as_ref(), &o, &out).unwrap();
    let text = fs::read_to_string(&out).unwrap();
    assert!(text.starts_with("# resampled to 2 Hz"), "{text}");
    assert!(text.ends_with("time,v\n0,1\n0.5,2\n1,3\n"), "{text}");
    // and the app reads its own export back
    assert_eq!(open(&out, &cache).info().samples, 3);
}

#[test]
fn hdf5_and_mat_read_back_exactly() {
    let tmp = tempfile::tempdir().unwrap();
    let (data, desc) = scaled_file(tmp.path());
    let cache = tmp.path().join("cache");
    let src = tlmbench_decode::open(
        &data,
        &OpenOptions {
            descriptor: Some(desc),
            part: None,
            profile: None,
            cache_dir: Some(cache.clone()),
            adjust: Vec::new(),
        },
    )
    .unwrap();
    let (times, values) = contents(src.as_ref());

    let h5 = tmp.path().join("out.h5");
    export(src.as_ref(), &Options::new(Format::Hdf5, vec![0, 1], 0, usize::MAX), &h5).unwrap();
    let f = hdf5::File::open(&h5).unwrap();
    assert_eq!(f.dataset("time_ns").unwrap().read_raw::<i64>().unwrap(), times);
    assert_eq!(f.dataset("volts").unwrap().read_raw::<f64>().unwrap(), values[0]);
    let unit: hdf5::types::VarLenUnicode = f.dataset("volts").unwrap().attr("unit").unwrap().read_scalar().unwrap();
    assert_eq!(unit.as_str(), "V");
    let temp: Vec<f64> = f.dataset("temp").unwrap().read_raw::<f32>().unwrap().iter().map(|&v| v as f64).collect();
    assert!(same(&[temp], &values[1..]));

    let mat = tmp.path().join("out.mat");
    let mut o = Options::new(Format::Mat, vec![0, 1], 0, usize::MAX);
    o.raw = true;
    export(src.as_ref(), &o, &mat).unwrap();
    let head = fs::read(&mat).unwrap();
    assert!(head.starts_with(b"MATLAB 7.3 MAT-file"));
    assert_eq!(&head[124..128], b"\x00\x02IM");
    let f = hdf5::File::open(&mat).unwrap();
    let volts = f.dataset("volts").unwrap();
    assert_eq!(volts.shape(), [1, 150_000]);
    let class: hdf5::types::FixedAscii<5> = volts.attr("MATLAB_class").unwrap().read_scalar().unwrap();
    assert_eq!(class.as_str(), "int16");
    let raw = volts.read_raw::<i16>().unwrap();
    assert_eq!((raw[0], raw[1999], raw[2000]), (-1000, 999, -1000));
    assert_eq!(f.dataset("time_ns").unwrap().read_raw::<i64>().unwrap(), times);
    assert!(fs::read_dir(tmp.path()).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().starts_with('.')));
}

#[test]
fn html_export_writes_data_files_and_trims_to_the_budget() {
    use crate::html::{self, Mode};
    let tmp = tempfile::tempdir().unwrap();
    let (data, desc) = scaled_file(tmp.path());
    let src = tlmbench_decode::open(
        &data,
        &OpenOptions { descriptor: Some(desc), part: None, profile: None, cache_dir: Some(tmp.path().join("cache")), adjust: Vec::new() },
    )
    .unwrap();
    let (times, values) = contents(src.as_ref());
    let file = |raw: Vec<usize>| html::File { name: "run".into(), rec: src.as_ref(), channels: vec![0, 1], raw };

    let out = tmp.path().join("site");
    let r = html::export(&out, &[file(vec![])], serde_json::json!({ "layout": { "name": "L" } }), &html::Options::default()).unwrap();
    assert_eq!(r.mode, Mode::Full);
    assert!(!r.trimmed);
    let m: serde_json::Value = serde_json::from_slice(&fs::read(out.join("export.json")).unwrap()).unwrap();
    assert_eq!(m["format"], "tlmbench-export");
    assert_eq!(m["layout"]["name"], "L");
    assert_eq!(m["files"][0]["samples"], 150_000);
    // the raw chunks hold the shown values exactly
    let raw: Vec<f64> = fs::read(out.join("data/f0/c0.0.bin")).unwrap().chunks(8).map(|b| f64::from_le_bytes(b.try_into().unwrap())).collect();
    assert_eq!(raw[..], values[0][..raw.len()]);
    // times are in files unless a rate gives them
    let kind = m["files"][0]["time"]["kind"].as_str().unwrap().to_owned();
    if kind == "column" {
        let t: Vec<i64> = fs::read(out.join("data/f0/t.0.bin")).unwrap().chunks(8).map(|b| i64::from_le_bytes(b.try_into().unwrap())).collect();
        assert_eq!(t[..], times[..t.len()]);
    }
    assert!(m["checksums"]["data/f0/c0.pyr.bin"].is_string());

    // Light keeps raw samples only for the marked channels, and a tiny budget drops the finest levels
    let light = tmp.path().join("light");
    let r = html::export(&light, &[file(vec![1])], serde_json::json!({}), &html::Options { mode: Mode::Light, ..Default::default() }).unwrap();
    assert_eq!(r.mode, Mode::Light);
    assert!(!light.join("data/f0/c0.0.bin").exists() && light.join("data/f0/c1.0.bin").exists());
    let tiny = tmp.path().join("tiny");
    let r = html::export(&tiny, &[file(vec![])], serde_json::json!({}), &html::Options { budget: 40_000, ..Default::default() }).unwrap();
    assert!(r.trimmed && r.mode == Mode::Light);
    assert!(r.files[0].channels.iter().all(|c| c.first_level > 4));
}
