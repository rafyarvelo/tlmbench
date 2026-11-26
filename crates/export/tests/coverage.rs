//! The data writers through every column type, time format, layout and refusal (coverage, R6c.3, Nov 26): each format
//! writes what the recording holds, in the type it holds it, with the time said the way it was asked for.

use std::fs;
use std::path::{Path, PathBuf};

use tlmbench_core::Recording;
use tlmbench_core::chunks::Held;
use tlmbench_decode::OpenOptions;
use tlmbench_export::{Error, Format, Layout, Options, TimeFormat, export, export_with};

const T0: i64 = 1_793_491_200_000_000_000; // 2026-11-01T00:00:00Z

fn open(path: &Path, cache: &Path, descriptor: Option<PathBuf>) -> Box<dyn Recording> {
    tlmbench_decode::open(
        path,
        &OpenOptions { descriptor, part: None, profile: None, cache_dir: Some(cache.to_owned()), adjust: Vec::new() },
    )
    .unwrap()
}

/// Ten channels, one of each stored type, five records at 1 kHz from `t0`; the floats have a missing value in the
/// middle record. Names: `i8` … `f64`; `i16` has a unit, `f32` a scale.
fn every_type(dir: &Path, t0: i64) -> (PathBuf, PathBuf) {
    let types = ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64"];
    let sizes = [1, 2, 4, 8, 1, 2, 4, 8, 4, 8];
    let mut toml = format!(
        "schema = 1\nendian = \"little\"\nrecord_size = 42\n[time]\nsource = \"rate\"\nrate_hz = 1000.0\nt0_ns = {t0}\n"
    );
    let mut off = 0;
    for (t, s) in types.iter().zip(sizes) {
        toml.push_str(&format!("[[fields]]\nname = \"{t}\"\ntype = \"{t}\"\noffset = {off}\n"));
        if *t == "i16" {
            toml.push_str("unit = \"mm\"\n");
        }
        if *t == "f32" {
            toml.push_str("scale = 2.0\n");
        }
        off += s;
    }
    let desc = dir.join("types.toml");
    fs::write(&desc, toml).unwrap();
    let mut bytes = Vec::new();
    for i in 0..5i64 {
        bytes.extend_from_slice(&((i - 2) as i8).to_le_bytes());
        bytes.extend_from_slice(&((-300 + i) as i16).to_le_bytes());
        bytes.extend_from_slice(&((-70_000 + i) as i32).to_le_bytes());
        bytes.extend_from_slice(&(-5_000_000_000 + i).to_le_bytes());
        bytes.extend_from_slice(&((250 + i) as u8).to_le_bytes());
        bytes.extend_from_slice(&((65_530 + i) as u16).to_le_bytes());
        bytes.extend_from_slice(&((4_000_000_000u64 + i as u64) as u32).to_le_bytes());
        bytes.extend_from_slice(&(10_000_000_000_000_000_000u64 + i as u64).to_le_bytes());
        let (f32v, f64v) = if i == 2 { (f32::NAN, f64::NAN) } else { (i as f32 * 0.5, i as f64 * 0.25) };
        bytes.extend_from_slice(&f32v.to_le_bytes());
        bytes.extend_from_slice(&f64v.to_le_bytes());
    }
    let data = dir.join("types.bin");
    fs::write(&data, bytes).unwrap();
    (data, desc)
}

fn typed(dir: &Path, t0: i64) -> Box<dyn Recording> {
    let (data, desc) = every_type(dir, t0);
    open(&data, &dir.join("cache"), Some(desc))
}

fn all(rec: &dyn Recording) -> Vec<usize> {
    (0..rec.info().channels.len()).collect()
}

fn values(r: &dyn Recording, c: usize) -> Vec<f64> {
    (0..r.info().samples).map(|i| r.value_at(c, i, &Held::default())).collect()
}

fn same(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x == y || x.is_nan() && y.is_nan())
}

/// Raw, every format keeps every stored type; shown, the scaled one becomes a double.
#[test]
fn every_stored_type_survives_every_columnar_format_raw_and_the_scaled_one_is_a_double_shown() {
    let tmp = tempfile::tempdir().unwrap();
    let rec = typed(tmp.path(), T0);
    let cache = tmp.path().join("cache2");
    let types = ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64"];
    for format in [Format::Parquet, Format::Csv] {
        for raw in [true, false] {
            let out = tmp.path().join(format!("t-{raw}.{}", format.extension()));
            let mut o = Options::new(format, all(rec.as_ref()), 0, usize::MAX);
            o.raw = raw;
            let w = export(rec.as_ref(), &o, &out).unwrap();
            assert_eq!(w.rows, 5);
            let back = open(&out, &cache, None);
            for (c, ty) in types.iter().enumerate() {
                let want = if raw { values_raw(rec.as_ref(), c) } else { values(rec.as_ref(), c) };
                assert!(same(&values(back.as_ref(), c), &want), "{format:?} raw={raw} {ty}");
            }
            if format == Format::Parquet {
                for (c, ty) in types.iter().enumerate() {
                    let want = if !raw && *ty == "f32" { "f64" } else { ty };
                    assert_eq!(back.info().channels[c].dtype, want, "{format:?} raw={raw}");
                }
            }
        }
    }
    // Arrow: the same through its own reader, one array type per stored type
    let arrow = tmp.path().join("t.arrow");
    let mut o = Options::new(Format::Arrow, all(rec.as_ref()), 0, usize::MAX);
    o.raw = true;
    export(rec.as_ref(), &o, &arrow).unwrap();
    let r = arrow_ipc::reader::FileReader::try_new(fs::File::open(&arrow).unwrap(), None).unwrap();
    let schema = r.schema();
    let names: Vec<String> = schema.fields().iter().map(|f| f.data_type().to_string()).collect();
    assert_eq!(
        names,
        [
            "Timestamp(ns, \"UTC\")",
            "Int8",
            "Int16",
            "Int32",
            "Int64",
            "UInt8",
            "UInt16",
            "UInt32",
            "UInt64",
            "Float32",
            "Float64"
        ]
    );
    assert_eq!(schema.field(0).name(), "time");
    assert_eq!(schema.field(2).metadata().get("unit").map(String::as_str), Some("mm"));
    let batch = r.map(|b| b.unwrap()).next().unwrap();
    let u64s = batch.column(8).as_any().downcast_ref::<arrow_array::UInt64Array>().unwrap();
    assert_eq!(u64s.value(4), 10_000_000_000_000_000_004);
    let times = batch.column(0).as_any().downcast_ref::<arrow_array::TimestampNanosecondArray>().unwrap();
    assert_eq!(times.value(1), T0 + 1_000_000);
}

fn values_raw(r: &dyn Recording, c: usize) -> Vec<f64> {
    r.read(c, 0, r.info().samples).to_f64(1.0, 0.0)
}

/// The time column's name and text for each way of asking, dates and relative times, in CSV and in the columnar files.
#[test]
fn time_is_written_as_asked_in_csv_parquet_and_arrow() {
    let tmp = tempfile::tempdir().unwrap();
    let dated = typed(tmp.path(), T0);
    let rel_dir = tmp.path().join("rel");
    fs::create_dir(&rel_dir).unwrap();
    let relative = typed(&rel_dir, 1_500_000_000);
    let first_row = |rec: &dyn Recording, t: TimeFormat, name: &str| {
        let mut o = Options::new(Format::Csv, vec![0], 0, 2);
        o.time = t;
        let out = tmp.path().join(name);
        export(rec, &o, &out).unwrap();
        fs::read_to_string(out).unwrap().lines().take(3).map(String::from).collect::<Vec<_>>()
    };
    assert_eq!(
        first_row(dated.as_ref(), TimeFormat::Auto, "a.csv"),
        ["time,i8", "2026-11-01T00:00:00Z,-2", "2026-11-01T00:00:00.001Z,-1"]
    );
    assert_eq!(first_row(dated.as_ref(), TimeFormat::Iso, "b.csv")[1], "2026-11-01T00:00:00Z,-2");
    assert_eq!(
        first_row(dated.as_ref(), TimeFormat::Seconds, "c.csv"),
        ["time,i8", "1793491200,-2", "1793491200.001,-1"]
    );
    assert_eq!(
        first_row(dated.as_ref(), TimeFormat::Ns, "d.csv"),
        ["time_ns,i8", "1793491200000000000,-2", "1793491200001000000,-1"]
    );
    assert_eq!(first_row(relative.as_ref(), TimeFormat::Auto, "e.csv"), ["time,i8", "1.5,-2", "1.501,-1"]);
    assert_eq!(first_row(relative.as_ref(), TimeFormat::Iso, "f.csv")[1], "1970-01-01T00:00:01.500Z,-2");

    // the columnar files: a timestamp for dates or ISO, i64 nanoseconds for relative, f64 seconds when asked
    let field = |rec: &dyn Recording, t: TimeFormat, format: Format, name: &str| {
        let mut o = Options::new(format, vec![0], 0, usize::MAX);
        o.time = t;
        let out = tmp.path().join(name);
        export(rec, &o, &out).unwrap();
        let r = arrow_ipc::reader::FileReader::try_new(fs::File::open(out).unwrap(), None).unwrap();
        let f = r.schema().field(0).clone();
        let b = r.map(|b| b.unwrap()).next().unwrap();
        (f.name().clone(), f.data_type().to_string(), b.column(0).len())
    };
    let ts = |n: &str| (n.to_owned(), "Timestamp(ns, \"UTC\")".to_owned(), 5);
    assert_eq!(field(dated.as_ref(), TimeFormat::Auto, Format::Arrow, "a.arrow"), ts("time"));
    assert_eq!(field(dated.as_ref(), TimeFormat::Iso, Format::Arrow, "b.arrow"), ts("time"));
    assert_eq!(field(relative.as_ref(), TimeFormat::Iso, Format::Arrow, "c.arrow"), ts("time"));
    assert_eq!(field(dated.as_ref(), TimeFormat::Ns, Format::Arrow, "d.arrow"), ("time_ns".into(), "Int64".into(), 5));
    assert_eq!(
        field(relative.as_ref(), TimeFormat::Auto, Format::Arrow, "e.arrow"),
        ("time_ns".into(), "Int64".into(), 5)
    );
    assert_eq!(
        field(dated.as_ref(), TimeFormat::Seconds, Format::Arrow, "f.arrow"),
        ("time_s".into(), "Float64".into(), 5)
    );
    // Parquet takes the same through the writer: read back as seconds
    let mut o = Options::new(Format::Parquet, vec![0], 0, usize::MAX);
    o.time = TimeFormat::Seconds;
    let out = tmp.path().join("s.parquet");
    export(dated.as_ref(), &o, &out).unwrap();
    let back = open(&out, &tmp.path().join("cache3"), None);
    assert_eq!(back.info().samples, 5);
}

/// A long file without a time column, with a delimiter that needs quoting, a units row and the manifest's quoting.
#[test]
fn csv_options_name_quote_and_lay_out_what_they_are_given() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("n.csv");
    // text in the first column: the rows are numbered, there is no time
    fs::write(&src, "label,speed [m/s],a;b \"q\"\nx,1,10\ny,2,\nz,3,30\n").unwrap();
    let rec = open(&src, &tmp.path().join("cache"), None);
    assert!(matches!(rec.info().time, tlmbench_core::TimeSource::Index));
    // channel 0 is the text column; the two numbers are 1 and 2
    let ch: Vec<usize> = vec![1, 2];
    let run = |name: &str, f: &dyn Fn(&mut Options)| {
        let mut o = Options::new(Format::Csv, ch.clone(), 0, usize::MAX);
        f(&mut o);
        let out = tmp.path().join(name);
        let w = export(rec.as_ref(), &o, &out).unwrap();
        (fs::read_to_string(out).unwrap(), fs::read_to_string(w.manifest).unwrap())
    };
    let (text, manifest) = run("a.csv", &|o| o.units_row = true);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[..3], ["speed,\"a;b \"\"q\"\"\"", "m/s,", "1,10"]);
    assert!(manifest.starts_with("channel,unit,type,rate_hz,file\n"));
    assert!(
        manifest.contains("\"a;b \"\"q\"\"\",,f64,,a.csv") || manifest.contains("\"a;b \"\"q\"\"\",,"),
        "{manifest}"
    );
    let (text, _) = run("b.csv", &|o| o.delimiter = b';');
    assert_eq!(text.lines().next(), Some("speed [m/s];\"a;b \"\"q\"\"\""));
    assert_eq!(text.lines().nth(2), Some("2;"), "a missing value is an empty field");
    let (text, manifest) = run("c.csv", &|o| {
        o.layout = Layout::Long;
        o.rate_hz = Some(50.0);
    });
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[..5], ["channel,value", "speed,1", "\"a;b \"\"q\"\"\",10", "speed,2", "\"a;b \"\"q\"\"\","]);
    assert!(manifest.lines().nth(1).unwrap().contains(",50,c.csv"), "{manifest}");
}

/// HDF5 and MAT: every type as itself, time as asked, repeated names made unique in the way each format needs, and
/// the resample note kept in HDF5.
#[test]
fn hdf5_and_mat_keep_types_name_columns_apart_and_say_how_time_is_written() {
    let tmp = tempfile::tempdir().unwrap();
    let a = typed(tmp.path(), T0);
    let dir2 = tmp.path().join("two");
    fs::create_dir(&dir2).unwrap();
    let b = typed(&dir2, T0);
    let extra: [&dyn Recording; 1] = [b.as_ref()];
    // the same ten names twice, the second ten from another recording of the same samples
    let channels: Vec<usize> = (0..20).collect();
    let mut o = Options::new(Format::Hdf5, channels.clone(), 0, usize::MAX);
    o.raw = true;
    o.time = TimeFormat::Seconds;
    let h5 = tmp.path().join("t.h5");
    export_with(a.as_ref(), &extra, &o, &h5).unwrap();
    let f = hdf5::File::open(&h5).unwrap();
    let time = f.dataset("time_s").unwrap();
    assert_eq!(time.read_raw::<f64>().unwrap()[1], T0 as f64 / 1e9 + 0.001);
    let unit: hdf5::types::VarLenUnicode = time.attr("unit").unwrap().read_scalar().unwrap();
    assert_eq!(unit.as_str(), "s");
    assert_eq!(f.dataset("i8").unwrap().read_raw::<i8>().unwrap(), [-2, -1, 0, 1, 2]);
    assert_eq!(f.dataset("i8 (2)").unwrap().read_raw::<i8>().unwrap(), [-2, -1, 0, 1, 2]);
    assert_eq!(f.dataset("u8").unwrap().read_raw::<u8>().unwrap(), [250, 251, 252, 253, 254]);
    assert_eq!(f.dataset("u64").unwrap().read_raw::<u64>().unwrap()[4], 10_000_000_000_000_000_004);
    assert_eq!(f.dataset("i64").unwrap().read_raw::<i64>().unwrap()[0], -5_000_000_000);
    assert_eq!(f.dataset("u32").unwrap().read_raw::<u32>().unwrap()[1], 4_000_000_001);
    assert_eq!(f.dataset("u16").unwrap().read_raw::<u16>().unwrap()[4], 65_534);
    assert_eq!(f.dataset("i32").unwrap().read_raw::<i32>().unwrap()[0], -70_000);
    assert_eq!(f.dataset("f32").unwrap().read_raw::<f32>().unwrap()[1], 0.5);
    assert!(f.dataset("f64").unwrap().read_raw::<f64>().unwrap()[2].is_nan());
    let mm: hdf5::types::VarLenUnicode = f.dataset("i16").unwrap().attr("unit").unwrap().read_scalar().unwrap();
    assert_eq!(mm.as_str(), "mm");

    // MAT: valid names, made unique; the time is `time`; the class says the type
    let mut o = Options::new(Format::Mat, channels, 0, usize::MAX);
    o.raw = true;
    o.time = TimeFormat::Seconds;
    let mat = tmp.path().join("t.mat");
    export_with(a.as_ref(), &extra, &o, &mat).unwrap();
    let f = hdf5::File::open(&mat).unwrap();
    assert_eq!(f.dataset("time").unwrap().shape(), [1, 5]);
    assert!(f.dataset("i8_2").is_ok() && f.dataset("f64_2").is_ok());
    let class: hdf5::types::FixedAscii<6> =
        f.dataset("f32").unwrap().attr("MATLAB_class").unwrap().read_scalar().unwrap();
    assert_eq!(class.as_str(), "single");
    let class: hdf5::types::FixedAscii<6> =
        f.dataset("u64").unwrap().attr("MATLAB_class").unwrap().read_scalar().unwrap();
    assert_eq!(class.as_str(), "uint64");

    // a note for a resampled file is kept in HDF5; ISO asks for the nanosecond column; a name with a slash is one name
    let src = tmp.path().join("slash.csv");
    fs::write(&src, "t,a/b,a/b\n0,1,2\n0.5,3,4\n1,5,6\n").unwrap();
    let rec = open(&src, &tmp.path().join("cache9"), None);
    let mut o = Options::new(Format::Hdf5, vec![0, 1], 0, usize::MAX);
    o.resample_hz = Some(4.0);
    o.time = TimeFormat::Iso;
    let out = tmp.path().join("slash.h5");
    export(rec.as_ref(), &o, &out).unwrap();
    let f = hdf5::File::open(&out).unwrap();
    let note: hdf5::types::VarLenUnicode = f.attr("tlmbench").unwrap().read_scalar().unwrap();
    assert!(note.as_str().starts_with("resampled to 4 Hz"), "{}", note.as_str());
    assert_eq!(f.dataset("time_ns").unwrap().read_raw::<i64>().unwrap().len(), 5);
    let names = f.member_names().unwrap();
    assert!(
        names.contains(&"a_b".to_owned()) && names.contains(&"a_b (2)".to_owned()) && names.len() == 3,
        "{names:?}"
    );
    // a recording with no time has no time dataset
    let plain = tmp.path().join("n.csv");
    fs::write(&plain, "label,v\nx,1\ny,2\n").unwrap();
    let rec = open(&plain, &tmp.path().join("cache10"), None);
    for (format, name) in
        [(Format::Hdf5, "n.h5"), (Format::Mat, "n.mat"), (Format::Parquet, "n.parquet"), (Format::Arrow, "n.arrow")]
    {
        export(rec.as_ref(), &Options::new(format, vec![1], 0, usize::MAX), &tmp.path().join(name)).unwrap();
    }
    let f = hdf5::File::open(tmp.path().join("n.h5")).unwrap();
    assert_eq!(f.member_names().unwrap(), ["v"]);
    let r = arrow_ipc::reader::FileReader::try_new(fs::File::open(tmp.path().join("n.arrow")).unwrap(), None).unwrap();
    assert_eq!(r.schema().fields().len(), 1);
}

#[test]
fn what_cannot_be_exported_is_refused_with_why_and_leaves_nothing_behind() {
    let tmp = tempfile::tempdir().unwrap();
    let rec = typed(tmp.path(), T0);
    let err = |o: &Options, out: &Path| export(rec.as_ref(), o, out).unwrap_err();
    let msg = |e: Error| e.to_string();
    let out = tmp.path().join("x.csv");
    let mut o = Options::new(Format::Csv, vec![0], 0, usize::MAX);
    assert_eq!(msg(err(&Options::new(Format::Csv, vec![99], 0, usize::MAX), &out)), "no channel 99");
    o.layout = Layout::Long;
    o.format = Format::Hdf5;
    assert!(msg(err(&o, &out)).contains("long layout is for CSV, Parquet and Arrow"));
    o.format = Format::Mat;
    assert!(msg(err(&o, &out)).contains("long layout"));
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let mut o = Options::new(Format::Csv, vec![0], 0, usize::MAX);
        o.resample_hz = Some(bad);
        assert!(msg(err(&o, &out)).starts_with("bad resample rate"), "{bad}");
    }
    assert!(msg(err(&Options::new(Format::Csv, vec![0], 0, 1), Path::new(".."))).contains("not a file name"));
    // a folder that isn't there: the error names the path; no file is left in the folder beside
    let e = err(&Options::new(Format::Csv, vec![0], 0, usize::MAX), &tmp.path().join("nope/x.csv"));
    assert!(matches!(e, Error::Io { .. }) && msg(e).contains("nope"));
    let e = err(&Options::new(Format::Parquet, vec![0], 0, usize::MAX), &tmp.path().join("nope/x.parquet"));
    assert!(matches!(e, Error::Io { .. }));
    assert!(fs::read_dir(tmp.path()).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().starts_with('.')));
    // more channels from a recording of other length, and from none at all
    let dir2 = tmp.path().join("short");
    fs::create_dir(&dir2).unwrap();
    let short = dir2.join("s.csv");
    fs::write(&short, "t,x\n0,1\n1,2\n").unwrap();
    let other = open(&short, &dir2.join("cache"), None);
    let e = export_with(rec.as_ref(), &[other.as_ref()], &Options::new(Format::Csv, vec![0], 0, usize::MAX), &out)
        .unwrap_err();
    assert!(msg(e).contains("has 2 samples, not 5"));
    // no time to resample against
    let plain = tmp.path().join("n.csv");
    fs::write(&plain, "label,v\nx,1\ny,2\n").unwrap();
    let nt = open(&plain, &tmp.path().join("cache2"), None);
    let mut o = Options::new(Format::Csv, vec![0], 0, usize::MAX);
    o.resample_hz = Some(1.0);
    assert!(export(nt.as_ref(), &o, &out).unwrap_err().to_string().contains("without time can't be resampled"));
    // an empty span writes a header and no rows, resampled or not
    for resample in [None, Some(10.0)] {
        let mut o = Options::new(Format::Csv, vec![0], 3, 3);
        o.resample_hz = resample;
        let w = export(rec.as_ref(), &o, &out).unwrap();
        assert_eq!(w.rows, 0);
        assert!(fs::read_to_string(&out).unwrap().lines().last().unwrap().starts_with("time,i8"));
    }
}

/// A resampled file: every format says so, steps cover the span, a long layout counts a row per channel and step.
#[test]
fn resampling_across_formats_layouts_and_spans() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("r.csv");
    fs::write(&src, "t,v,w\n0,1,10\n0.25,2,20\n1,3,\n2,4,40\n").unwrap();
    let rec = open(&src, &tmp.path().join("cache"), None);
    let go = |format: Format, f: &dyn Fn(&mut Options), name: &str| {
        let mut o = Options::new(format, vec![0, 1], 0, usize::MAX);
        o.resample_hz = Some(2.0);
        f(&mut o);
        let out = tmp.path().join(name);
        let w = export(rec.as_ref(), &o, &out).unwrap();
        (w, out)
    };
    // 0, 0.5, 1, 1.5, 2 s: the last value at or before each; w is missing at 1 s and stays missing there
    let (w, out) = go(Format::Csv, &|_| {}, "a.csv");
    assert_eq!(w.rows, 5);
    let text = fs::read_to_string(&out).unwrap();
    assert!(text.ends_with("time,v,w\n0,1,10\n0.5,2,20\n1,3,\n1.5,3,\n2,4,40\n"), "{text}");
    // a long layout: a row per channel and step; a part of the span
    let (w, out) = go(Format::Csv, &|o| (o.layout, o.start, o.end) = (Layout::Long, 1, 3), "b.csv");
    assert_eq!(w.rows, 2 * 2, "two steps (0.25 s, 0.75 s) of two channels: {}", fs::read_to_string(&out).unwrap());
    let (w, _) = go(Format::Parquet, &|o| o.layout = Layout::Long, "c.parquet");
    assert_eq!(w.rows, 10);
    // the note is file metadata in Parquet and Arrow
    let (_, out) = go(Format::Arrow, &|_| {}, "d.arrow");
    let r = arrow_ipc::reader::FileReader::try_new(fs::File::open(out).unwrap(), None).unwrap();
    assert!(r.schema().metadata().get("tlmbench").unwrap().starts_with("resampled to 2 Hz"));
    let (_, out) = go(Format::Parquet, &|_| {}, "e.parquet");
    let back = tlmbench_decode::open(
        &out,
        &OpenOptions {
            descriptor: None,
            part: None,
            profile: None,
            cache_dir: Some(tmp.path().join("c2")),
            adjust: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(back.info().samples, 5);
}
