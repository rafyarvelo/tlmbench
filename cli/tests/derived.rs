//! Derived channels on the command line: `derive`, and `--derived` / `--expr` in `info`, `query` and `convert`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(cache: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tlmbench")).args(args).arg("--cache").arg(cache).output().unwrap()
}

fn ok(cache: &Path, args: &[&str]) -> String {
    let out = run(cache, args);
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// `flights/log.csv` (battery_v, current_a at 10 Hz) with team recipes in `flights/.tlmbench/recipes`.
fn folder(dir: &Path) -> PathBuf {
    let flights = dir.join("flights");
    std::fs::create_dir_all(flights.join(".tlmbench/recipes")).unwrap();
    let mut text = String::from("time,battery_v,current_a\n");
    for i in 0..200 {
        text.push_str(&format!("{:.1},{},{}\n", i as f64 / 10.0, 12 + i % 3, 1 + i % 4));
    }
    std::fs::write(flights.join("log.csv"), text).unwrap();
    let recipe = |name: &str, expr: &str, auto: bool| {
        format!("schema = 1\n\n[[recipe]]\nname = \"{name}\"\nexpr = \"{expr}\"\nunit = \"W\"\nautomatic = {auto}\n")
    };
    std::fs::write(flights.join(".tlmbench/recipes/power.toml"), recipe("Power", "battery_v * current_a", true))
        .unwrap();
    std::fs::write(flights.join(".tlmbench/recipes/manual.toml"), recipe("Manual", "battery_v - 1", false)).unwrap();
    std::fs::write(flights.join(".tlmbench/recipes/needs.toml"), recipe("Needs gps", "gps_alt + 1", true)).unwrap();
    flights.join("log.csv")
}

#[test]
fn derive_prints_a_series_or_a_number() {
    let (data, cache) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let file = folder(data.path());
    let f = file.to_str().unwrap();
    let out = ok(cache.path(), &["derive", f, "--expr", "battery_v * current_a"]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "time_ns,value");
    assert_eq!(lines.len(), 201);
    assert!(lines[1].ends_with(",12")); // 12 * 1
    assert!(lines[2].ends_with(",26")); // 13 * 2
    // one number
    let mean = ok(cache.path(), &["derive", f, "--expr", "max(battery_v)"]);
    assert_eq!(mean.trim(), "14");
    // a span
    let span = ok(cache.path(), &["derive", f, "--expr", "delta(battery_v)", "--from", "5", "--until", "6"]);
    assert_eq!(span.lines().count(), 11);
    // a mistake names the character and exits 3
    let bad = run(cache.path(), &["derive", f, "--expr", "battery_v * (2 +"]);
    assert_eq!(bad.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("The expression ends too soon (at 16)"));
    let missing = run(cache.path(), &["derive", f, "--expr", "nope + 1"]);
    assert_eq!(missing.status.code(), Some(3));
}

#[test]
fn info_lists_derived_channels() {
    let (data, cache) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let file = folder(data.path());
    let f = file.to_str().unwrap();
    let plain = ok(cache.path(), &["info", f]);
    assert!(!plain.contains("Derived") && !plain.contains('ƒ'));
    let text = ok(cache.path(), &["info", f, "--derived"]);
    assert!(text.contains("Derived   1") && text.contains("ƒ Power"), "{text}");
    assert!(!text.contains("Manual"), "not automatic: {text}");
    let json: serde_json::Value = serde_json::from_str(
        ok(cache.path(), &["info", f, "--derived", "--json", "--expr", "Half=battery_v/2"]).trim(),
    )
    .unwrap();
    let names: Vec<&str> = json["derived"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Power", "Half"]);
    // a recipe that needs a channel the file lacks is skipped with a note, not an error
    let out = run(cache.path(), &["info", f, "--derived"]);
    assert!(String::from_utf8_lossy(&out.stderr).contains("recipe \"Needs gps\" doesn't apply"));
    // an explicit recipe file instead of the nearest .tlmbench
    let only = data.path().join("only.toml");
    std::fs::write(&only, "schema = 1\n\n[[recipe]]\nname = \"Twice\"\nexpr = \"current_a * 2\"\n").unwrap();
    let json: serde_json::Value = serde_json::from_str(
        ok(cache.path(), &["info", f, "--derived", "--recipes", only.to_str().unwrap(), "--json"]).trim(),
    )
    .unwrap();
    assert_eq!(json["derived"][0]["name"], "Twice");
}

#[test]
fn query_reads_derived_channels_beside_raw_ones() {
    let (data, cache) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let file = folder(data.path());
    let f = file.to_str().unwrap();
    let stats = ok(cache.path(), &["query", "stats", f, "--derived", "-c", "Power", "-c", "battery_v"]);
    let rows: Vec<&str> = stats.lines().collect();
    assert_eq!(rows.len(), 3, "{stats}");
    // order follows the recording's channels, then the derived ones
    assert!(rows[1].starts_with("battery_v,200,12,") && rows[2].starts_with("Power,200,"), "{stats}");
    let values =
        ok(cache.path(), &["query", "values", f, "--derived", "--at", "1.0", "-c", "Power", "-c", "current_a"]);
    assert!(values.contains("Power,10,") && values.contains("current_a,10,"), "{values}");
    // a table is of one recording's channels
    let mixed = run(cache.path(), &["query", "rows", f, "--derived", "-c", "Power", "-c", "battery_v"]);
    assert_eq!(mixed.status.code(), Some(3));
    let rows =
        ok(cache.path(), &["query", "rows", f, "--derived", "-c", "Power", "--filter", "Power=> 40", "--limit", "3"]);
    assert_eq!(rows.lines().next().unwrap(), "row,time_ns,Power");
    assert!(rows.lines().skip(1).all(|l| l.rsplit(',').next().unwrap().parse::<f64>().unwrap() > 40.0));
}

#[test]
fn convert_writes_derived_columns() {
    let (data, cache, out) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let file = folder(data.path());
    let f = file.to_str().unwrap();
    let o = out.path().to_str().unwrap();
    ok(cache.path(), &["convert", f, "--to", "csv", "--out", o]);
    let plain = std::fs::read_to_string(out.path().join("log.csv")).unwrap();
    assert_eq!(plain.lines().next().unwrap(), "time,battery_v,current_a");
    ok(cache.path(), &["convert", f, "--to", "csv", "--out", o, "--derived", "--expr", "Half=battery_v/2"]);
    let with = std::fs::read_to_string(out.path().join("log.csv")).unwrap();
    assert_eq!(with.lines().next().unwrap(), "time,battery_v,current_a,Power [W],Half");
    let first: Vec<&str> = with.lines().nth(1).unwrap().split(',').collect();
    assert_eq!(&first[1..], ["12", "1", "12", "6"]);
    let manifest = std::fs::read_to_string(out.path().join("log.channels.csv")).unwrap();
    assert!(manifest.contains("Power,W,f64"), "{manifest}");
    // only a derived channel, by name
    ok(cache.path(), &["convert", f, "--to", "csv", "--out", o, "--derived", "-c", "Power"]);
    assert_eq!(std::fs::read_to_string(out.path().join("log.csv")).unwrap().lines().next().unwrap(), "time,Power [W]");
    // Parquet takes them too
    ok(cache.path(), &["convert", f, "--to", "parquet", "--out", o, "--expr", "Half=battery_v/2", "-c", "Half"]);
    assert!(out.path().join("log.parquet").exists());
    // nothing was written beside the data
    let mut beside: Vec<String> = std::fs::read_dir(file.parent().unwrap())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    beside.sort();
    assert_eq!(beside, [".tlmbench", "log.csv"]);
}
