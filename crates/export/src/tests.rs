use std::fs;
use std::path::Path;

use tlmbench_core::Recording;
use tlmbench_core::chunks::Held;
use tlmbench_decode::OpenOptions;

use super::*;

fn open(path: &Path, cache: &Path) -> Box<dyn Recording> {
    tlmbench_decode::open(path, &OpenOptions { descriptor: None, cache_dir: Some(cache.to_owned()) }).unwrap()
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
    let src =
        tlmbench_decode::open(&data, &OpenOptions { descriptor: Some(desc), cache_dir: Some(cache.clone()) }).unwrap();
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
    let src =
        tlmbench_decode::open(&data, &OpenOptions { descriptor: Some(desc), cache_dir: Some(cache.clone()) }).unwrap();
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
