//! `tlmbench convert` end to end: a fixture CSV to each format and back through `info` and `query values`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../prototype/tests")
}

fn run(cache: &Path, cmd: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tlmbench")).arg(cmd).arg("--cache").arg(cache).args(args).output().unwrap()
}

fn ok(out: &Output) -> String {
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn to_each_format_and_back() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = tmp.path().join("cache");
    let src = fixtures().join("fold/stocks/AAPL.csv");
    let src = src.to_str().unwrap();
    let before = ok(&run(&cache, "query", &["values", "--at", "100000", src]));
    for to in ["csv", "parquet"] {
        let out = tmp.path().join(to);
        let line = ok(&run(&cache, "convert", &["--to", to, "--out", out.to_str().unwrap(), "--json", src]));
        let j: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(j["rows"], 521, "{to}");
        let file = out.join(format!("AAPL.{to}"));
        assert!(out.join("AAPL.channels.csv").exists());
        let again = ok(&run(&cache, "query", &["values", "--at", "100000", file.to_str().unwrap()]));
        assert_eq!(again, before, "{to}");
    }
    // nothing was written beside the input
    let siblings: Vec<_> = std::fs::read_dir(fixtures().join("fold/stocks")).unwrap().collect();
    assert!(siblings.iter().all(|e| !e.as_ref().unwrap().file_name().to_string_lossy().contains("channels")));
}

#[test]
fn channels_span_and_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = tmp.path().join("cache");
    let src = fixtures().join("fold/stocks/AAPL.csv");
    let out = tmp.path().join("o");
    let args = ["--to", "csv", "--out", out.to_str().unwrap(), "-c", "Op*,Close", "--until", "86400"];
    ok(&run(&cache, "convert", &[&args[..], &[src.to_str().unwrap()]].concat()));
    let text = std::fs::read_to_string(out.join("AAPL.csv")).unwrap();
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("time,Open,Close"));
    assert_eq!(lines.count(), 1);

    let bad =
        run(&cache, "convert", &["--to", "csv", "--out", out.to_str().unwrap(), "-c", "nope", src.to_str().unwrap()]);
    assert_eq!(bad.status.code(), Some(3));
    let rate = run(&cache, "convert", &["--to", "csv", "--out", "x", "--resample", "fast", src.to_str().unwrap()]);
    assert_eq!(rate.status.code(), Some(3));
}
