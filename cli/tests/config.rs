//! Shared configuration on the command line: `--config`, `validate` and `config export`.

use std::path::Path;
use std::process::{Command, Output};

fn run(cache: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tlmbench")).args(args).arg("--cache").arg(cache).output().unwrap()
}

fn validate(dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tlmbench")).arg("validate").arg(dir).output().unwrap()
}

fn recipe(name: &str, expr: &str) -> String {
    format!("schema = 1\n\n[[recipe]]\nname = \"{name}\"\nexpr = \"{expr}\"\nautomatic = true\n")
}

fn log(dir: &Path) -> std::path::PathBuf {
    let mut text = String::from("time,battery_v,current_a\n");
    for i in 0..50 {
        text.push_str(&format!("{:.1},{},{}\n", i as f64 / 10.0, 12 + i % 3, 1 + i % 4));
    }
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("log.csv"), text).unwrap();
    dir.join("log.csv")
}

#[test]
fn config_folders_replace_the_nearest_and_later_ones_win() {
    let (data, cache) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let file = log(&data.path().join("runs"));
    // the nearest `.tlmbench/` says Power is a sum; two --config folders say product, then difference
    let near = data.path().join(".tlmbench/recipes");
    std::fs::create_dir_all(&near).unwrap();
    std::fs::write(near.join("p.toml"), recipe("Power", "battery_v + current_a")).unwrap();
    let (a, b) = (data.path().join("a"), data.path().join("b"));
    for (d, expr) in [(&a, "battery_v * current_a"), (&b, "battery_v - current_a")] {
        std::fs::create_dir_all(d.join("recipes")).unwrap();
        std::fs::write(d.join("recipes/p.toml"), recipe("Power", expr)).unwrap();
    }
    let f = file.to_str().unwrap();
    let first = |extra: &[&str]| {
        let mut args = vec!["query", "values", f, "--at", "0", "--derived", "-c", "Power"];
        args.extend_from_slice(extra);
        let out = run(cache.path(), &args);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let nearest = first(&[]);
    let one = first(&["--config", a.to_str().unwrap()]);
    let two = first(&["--config", a.to_str().unwrap(), "--config", b.to_str().unwrap()]);
    assert!(nearest.trim_end().ends_with(",13"), "{nearest}"); // 12 + 1
    assert!(one.trim_end().ends_with(",12"), "{one}"); // 12 * 1
    assert!(two.trim_end().ends_with(",11"), "{two}"); // 12 - 1
}

#[test]
fn validate_lints_the_folder_and_exits_3_on_problems() {
    let data = tempfile::tempdir().unwrap();
    let dir = data.path().join(".tlmbench/recipes");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("ok.toml"), recipe("Power", "battery_v * 2")).unwrap();
    let ok = validate(data.path());
    assert!(ok.status.success(), "{}", String::from_utf8_lossy(&ok.stderr));
    std::fs::write(dir.join("bad.toml"), recipe("Bad", "battery_v *")).unwrap();
    let bad = validate(data.path());
    assert_eq!(bad.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("bad.toml"));
}

#[test]
fn config_export_writes_the_cache_and_rewrites_nothing_when_unchanged() {
    let cache = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(cache.path().join("recipes")).unwrap();
    std::fs::write(cache.path().join("recipes/power.toml"), recipe("Power", "battery_v * current_a")).unwrap();
    let out = tempfile::tempdir().unwrap();
    let dest = out.path().join(".tlmbench");
    let first = run(cache.path(), &["config", "export", "--out", dest.to_str().unwrap()]);
    assert!(first.status.success(), "{}", String::from_utf8_lossy(&first.stderr));
    assert!(dest.join("recipes/power.toml").exists());
    let again = run(cache.path(), &["config", "export", "--out", dest.to_str().unwrap()]);
    assert!(again.stdout.is_empty(), "an unchanged export writes nothing");
    assert!(validate(&dest).status.success());
}
