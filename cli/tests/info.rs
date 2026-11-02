//! `tlmbench info` end to end: the binary on the prototype's fixture files, a reference file, and errors.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../prototype/tests")
}

fn info(cache: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tlmbench")).arg("info").arg("--cache").arg(cache).args(args).output().unwrap()
}

fn json(out: &Output) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(&out.stdout).lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

#[test]
fn csv_fixtures_as_json() {
    let cache = tempfile::tempdir().unwrap();
    let f = fixtures();
    let files = ["fold/stocks/AAPL.csv", "csv/nasty_headers.csv", "csv/notime.csv"].map(|p| f.join(p));
    let mut args = vec!["--json"];
    args.extend(files.iter().map(|p| p.to_str().unwrap()));
    let out = info(cache.path(), &args);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v = json(&out);
    assert_eq!(v[0]["samples"], 521);
    assert_eq!(v[0]["time"]["name"], "Date");
    assert_eq!(v[0]["duration_ns"], 728i64 * 86_400 * 1_000_000_000);
    assert_eq!(v[1]["channels"][0]["unit"], "m/s");
    assert_eq!(v[1]["skipped"], serde_json::json!(["Note, quoted", "Flight Mode"]));
    assert_eq!(v[2]["time"]["kind"], "index");
}

#[test]
fn reference_file_text_output() {
    use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rec.tlm");
    let desc = Descriptor {
        sequence: None,
        schema: SCHEMA,
        name: None,
        endian: Endian::Little,
        header_size: 0,
        record_size: 4,
        time: TimeSpec::Rate { rate_hz: 1000.0, t0_ns: 1_767_225_600_000_000_000 },
        fields: vec![Field {
            name: "volts".into(),
            dtype: DType::F32,
            offset: 0,
            unit: Some("V".into()),
            scale: 1.0,
            value_offset: 0.0,
        }],
    };
    let mut data = Vec::new();
    tlmbench_decode::reference::write_header(&mut data, &desc).unwrap();
    for i in 0..5000 {
        data.extend((i as f32).to_le_bytes());
    }
    std::fs::write(&path, data).unwrap();
    let out = info(dir.path(), &[path.to_str().unwrap()]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(text.contains("Format    tlmbench reference"), "{text}");
    assert!(text.contains("1000 Hz, 2026-01-01T00:00:00Z → 2026-01-01T00:00:04.999Z (4.999 s)"), "{text}");
    assert!(text.contains("Samples   5,000"), "{text}");
    assert!(text.contains("volts  f32  V"), "{text}");
}

#[test]
fn exit_codes() {
    let dir = tempfile::tempdir().unwrap();
    let out = info(dir.path(), &[fixtures().join("csv/readme.md").to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("not a format tlmbench reads"));

    let bad = dir.path().join("bad.toml");
    std::fs::write(&bad, "schema = 1\nrecord_size = 4\n[time]\nsource = \"index\"\n[[fields]]\nname = \"v\"\ntype = \"f32\"\noffset = 2\n").unwrap();
    let out = info(dir.path(), &["--descriptor", bad.to_str().unwrap(), bad.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(3), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("runs past the 4-byte record"));
}
