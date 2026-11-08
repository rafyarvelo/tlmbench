//! Import profiles end to end: `inspect`, and `--profile` on `info` and `query`.

use std::path::Path;
use std::process::{Command, Output};

fn run(cache: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tlmbench")).args(args).arg("--cache").arg(cache).output().unwrap()
}

fn csv(dir: &Path) -> std::path::PathBuf {
    let p = dir.join("flight.csv");
    std::fs::write(&p, "time_ms,Speed [m/s],gps,note\n0,1,10,a\n100,2,20,b\n200,3,30,c\n400,5,50,d\n").unwrap();
    p
}

#[test]
fn inspect_shows_columns_time_and_rate() {
    let (dir, cache) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let file = csv(dir.path());
    let out = run(cache.path(), &["inspect", file.to_str().unwrap()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["time"], 0);
    assert_eq!(v["time_unit"], "ms");
    assert_eq!(v["rows"], 4);
    assert_eq!(v["columns"][1]["unit"], "m/s");
    assert_eq!(v["columns"][3]["kind"], "text");
    assert_eq!(v["preview"][0][1], "1");
    assert_eq!(v["rate_hz"], 10.0);
    assert!(v["profile"]["exact"].is_null());
}

#[test]
fn a_profile_changes_what_opens() {
    let (dir, cache) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let file = csv(dir.path());
    let profile = dir.path().join("p.json");
    std::fs::write(
        &profile,
        r#"{"rate":{"type":"assume","hz":50},
            "columns":[{"source":"Speed","name":"v"},{"source":"gps","kind":"skip"}]}"#,
    )
    .unwrap();
    let out = run(cache.path(), &["info", "--json", "--profile", profile.to_str().unwrap(), file.to_str().unwrap()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["time"]["kind"], "rate");
    assert_eq!(v["channels"][1]["name"], "v");
    assert_eq!(v["skipped"], serde_json::json!(["gps", "note"]));
    assert_eq!(v["duration_ns"], 60_000_000);
    // without it, the file opens as before
    let plain = run(cache.path(), &["info", "--json", file.to_str().unwrap()]);
    let v: serde_json::Value = serde_json::from_slice(&plain.stdout).unwrap();
    assert_eq!(v["time"]["kind"], "column");
}

#[test]
fn a_bad_profile_is_a_configuration_error() {
    let (dir, cache) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let file = csv(dir.path());
    let profile = dir.path().join("bad.json");
    std::fs::write(&profile, r#"{"rate":{"type":"nope"}}"#).unwrap();
    let out = run(cache.path(), &["info", "--profile", profile.to_str().unwrap(), file.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(3));
}
