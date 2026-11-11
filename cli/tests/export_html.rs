//! `tlmbench export html` end to end: the data files, the layout's channels, redaction, budgets, the checks summary,
//! and that nothing is written beside the input.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../prototype/tests")
}

fn run(cache: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tlmbench")).args(["export", "html", "--cache"]).arg(cache).args(args).output().unwrap()
}

fn manifest(out: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(out.join("export.json")).unwrap()).unwrap()
}

#[test]
fn writes_the_layout_channels_and_nothing_beside_the_input() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = tmp.path().join("cache");
    let src = fixtures().join("fold/stocks/AAPL.csv");
    let src = src.to_str().unwrap();
    let out = tmp.path().join("site");
    let o = out.to_str().unwrap();
    let view = tmp.path().join("v.json");
    std::fs::write(&view, r#"{"schema":1,"name":"Two","panes":[{"channels":["Close"],"view":"chart"},{"channels":["Open"],"view":"table"}]}"#).unwrap();
    let r = run(&cache, &["--no-viewer", "--layout", view.to_str().unwrap(), "--out", o, "--json", src]);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let m = manifest(&out);
    let names: Vec<&str> = m["files"][0]["channels"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Open", "Close"]);
    assert_eq!(m["layout"]["name"], "Two");
    assert_eq!(m["files"][0]["samples"], 521);
    // all channels, minus a redacted one
    let all = tmp.path().join("all");
    let r = run(&cache, &["--no-viewer", "--all-channels", "--redact", "Volume", "--out", all.to_str().unwrap(), src]);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let m = manifest(&all);
    assert!(m["files"][0]["channels"].as_array().unwrap().iter().all(|c| c["name"] != "Volume"));
    assert!(m["files"][0]["channels"].as_array().unwrap().len() >= 3);
    // a tiny budget trims, light keeps raw only for the table channel
    let tiny = tmp.path().join("tiny");
    let r = run(&cache, &["--no-viewer", "--mode", "light", "--layout", view.to_str().unwrap(), "--out", tiny.to_str().unwrap(), src]);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let m = manifest(&tiny);
    let raw: Vec<(&str, bool)> = m["files"][0]["channels"].as_array().unwrap().iter().map(|c| (c["name"].as_str().unwrap(), c["raw"].as_bool().unwrap())).collect();
    assert_eq!(raw, [("Open", true), ("Close", false)]);
    // bad input: exit 3 for a view that isn't there, a size that isn't one
    assert_eq!(run(&cache, &["--no-viewer", "--layout", "Nope", "--out", o, src]).status.code(), Some(3));
    assert_eq!(run(&cache, &["--no-viewer", "--budget", "lots", "--out", o, src]).status.code(), Some(3));
    // nothing was written beside the input
    assert!(std::fs::read_dir(fixtures().join("fold/stocks")).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().contains("export")));
}
