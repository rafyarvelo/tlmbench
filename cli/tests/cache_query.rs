//! `tlmbench cache` and `tlmbench query` end to end.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(cache: &Path, args: &[&str]) -> Output {
    let out = Command::new(env!("CARGO_BIN_EXE_tlmbench")).args(args).arg("--cache").arg(cache).output().unwrap();
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    out
}

fn text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A two-channel .tlm with a time field in µs: 10 kHz with a 2 s dropout at sample 3M.
fn recording(dir: &Path, n: usize) -> PathBuf {
    use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec, TimeUnit};
    let field = |name: &str, dtype, offset| Field {
        name: name.into(),
        dtype,
        offset,
        unit: None,
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
        record_size: 16,
        time: TimeSpec::Field { field: "t".into(), unit: TimeUnit::Us, t0_ns: 1_767_225_600_000_000_000 },
        fields: vec![field("t", DType::I64, 0), field("volts", DType::F32, 8), field("amps", DType::F32, 12)],
    };
    let path = dir.join("bench.tlm");
    let mut w = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
    tlmbench_decode::reference::write_header(&mut w, &desc).unwrap();
    for i in 0..n as i64 {
        let t = i * 100 + if i >= 3_000_000 { 2_000_000 } else { 0 };
        w.write_all(&t.to_le_bytes()).unwrap();
        w.write_all(&((i as f32 * 1e-4).sin() + if i == 77_777 { 9.0 } else { 0.0 }).to_le_bytes()).unwrap();
        w.write_all(&((i % 1000) as f32).to_le_bytes()).unwrap();
    }
    w.flush().unwrap();
    path
}

#[test]
fn build_list_query_clear() {
    let data = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let path = recording(data.path(), 4_500_000);
    let file = path.to_str().unwrap();
    let c = cache.path();

    let info = |c| serde_json::from_str::<serde_json::Value>(&text(&run(c, &["info", "--json", file]))).unwrap();
    assert_eq!(info(c)["gaps"], serde_json::Value::Null, "not indexed before a build");
    let view = ["query", "view", file, "-c", "volts", "--width", "1500", "--from", "250", "--until", "460"];
    let raw = text(&run(c, &view));

    run(c, &["cache", "build", "--quiet", file]);
    let g = info(c);
    assert_eq!(g["gaps"]["median_interval_ns"], 100_000);
    assert_eq!(g["gaps"]["count"], 1);

    let built = text(&run(c, &view));
    assert_eq!(built, raw, "the pyramid answers exactly like the raw samples");
    assert!(built.lines().count() > 1500 && built.starts_with("channel,time_ns,value\nvolts,"));
    assert!(built.lines().any(|l| l.ends_with(',')), "the dropout is a gap");
    // Connected gaps: no empty value, and the join is marked dashed
    let joined = text(&run(c, &[&view[..], &["--connect-gaps"]].concat()));
    assert!(joined.starts_with("channel,time_ns,value,dashed\nvolts,"));
    assert!(!joined.lines().any(|l| l.contains(",,")), "no gaps left");
    assert_eq!(joined.lines().filter(|l| l.ends_with(",1")).count(), 1, "one connected stretch");
    // Draw every sample: up to --raw-max in the span come back raw (2M here, and a neighbour each side), not reduced
    let every = text(&run(c, &[&view[..], &["--raw-max", "3000000"]].concat()));
    assert!(every.lines().count() == 2_000_003, "every sample: {}", every.lines().count());
    assert_eq!(text(&run(c, &[&view[..], &["--raw-max", "1999999"]].concat())), built, "past the budget it reduces");

    let values = text(&run(c, &["query", "values", file, "--at", "7.77771"]));
    let v = ((77_777f32 * 1e-4).sin() + 9.0) as f64;
    assert_eq!(values.lines().nth(1), Some(format!("volts,77777,1767225607777700000,{v}").as_str()));
    assert_eq!(values.lines().count(), 3);

    let stats = text(&run(c, &["query", "stats", file, "-c", "amps"]));
    let row: Vec<&str> = stats.lines().nth(1).unwrap().split(',').collect();
    assert_eq!(&row[..3], ["amps", "4500000", "0"]);
    assert_eq!(row[4], "999");
    assert_eq!(row[6], "499.5");

    let list = text(&run(c, &["cache", "list", "--json"]));
    let kinds: Vec<serde_json::Value> = list.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(kinds.len(), 1);
    assert_eq!(kinds[0]["kind"], "pyramid");
    assert_eq!(kinds[0]["building"], false);
    assert!(text(&run(c, &["cache", "clear"])).starts_with("Removed 1 cache entries"));
    assert_eq!(text(&run(c, &["cache", "list", "--json"])), "");
}

/// Two processes building one file: one builds, the other follows it, and both finish with the same entry.
#[test]
fn concurrent_builds_share_one_entry() {
    let data = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let path = recording(data.path(), 8_000_000);
    let spawn = || {
        Command::new(env!("CARGO_BIN_EXE_tlmbench"))
            .args(["cache", "build", "--quiet"])
            .arg(&path)
            .arg("--cache")
            .arg(cache.path())
            .output()
    };
    let (a, b) = std::thread::scope(|s| {
        let a = s.spawn(spawn);
        let b = s.spawn(spawn);
        (a.join().unwrap().unwrap(), b.join().unwrap().unwrap())
    });
    assert!(
        a.status.success() && b.status.success(),
        "{}{}",
        String::from_utf8_lossy(&a.stderr),
        String::from_utf8_lossy(&b.stderr)
    );
    let list = text(&run(cache.path(), &["cache", "list", "--json"]));
    assert_eq!(list.lines().count(), 1);
    assert!(list.contains("\"building\":false"));
}

#[test]
fn rows_filtered_and_sorted() {
    let data = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let path = recording(data.path(), 200_000);
    let file = path.to_str().unwrap();
    let c = cache.path();
    let rows = |extra: &[&str]| {
        let mut args = vec!["query", "rows", file, "-c", "volts", "-c", "amps"];
        args.extend_from_slice(extra);
        text(&run(c, &args))
    };

    let spike = rows(&["--filter", "volts=> 5"]);
    assert_eq!(spike.lines().count(), 2);
    assert!(spike.starts_with("row,time_ns,volts,amps\n77777,1767225607777700000,"), "{spike}");

    // the last two rows with amps = 999, newest first
    let newest = rows(&["--filter", "amps=999", "--sort", "time:desc", "--limit", "2"]);
    let firsts: Vec<&str> = newest.lines().skip(1).map(|l| l.split(',').next().unwrap()).collect();
    assert_eq!(firsts, ["199999", "198999"]);

    // two sort keys, and offset after sorting: amps descending, then time descending
    let two = rows(&["--sort", "amps:desc", "--sort", "time:desc", "--limit", "3", "--offset", "1"]);
    let firsts: Vec<&str> = two.lines().skip(1).map(|l| l.split(',').next().unwrap()).collect();
    assert_eq!(firsts, ["198999", "197999", "196999"]);

    // or, and the time column (absolute ns: the first 5 ms hold rows 0..50)
    let either = rows(&["--filter", "amps=0 or 1", "--filter", "time=< 1767225600005000000"]);
    let firsts: Vec<&str> = either.lines().skip(1).map(|l| l.split(',').next().unwrap()).collect();
    assert_eq!(firsts, ["0", "1"]);

    let bad = Command::new(env!("CARGO_BIN_EXE_tlmbench"))
        .args(["query", "rows", file, "--filter", "amps=> x", "--cache"])
        .arg(c)
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("can't read \"> x\""));
}
