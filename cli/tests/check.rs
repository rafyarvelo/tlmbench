//! `tlmbench check`: sets from `.tlmbench/checks`, exit codes, reports, recipes by name, import-profile sets and the
//! cached results.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(cache: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tlmbench")).arg("check").args(args).arg("--cache").arg(cache).output().unwrap()
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap()
}

fn text(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

const SET: &str = r#"schema = 1
name = "Acceptance"
runs_on_any_file = true
info_groups = ["Info"]

[[check]]
name = "Battery above 12 V"
group = "Power"
expect = "battery_v > 12"
grace = 0.5

[[check]]
name = "Power under 50 W"
group = "Power"
expect = "Power < 50"
severity = "warn"

[[check]]
name = "Peak current"
group = "Power"
expect = "max(current_a) < 5"

[[check]]
name = "Altitude set"
group = "Info"
expect = "gps_alt > 0"
"#;

/// good.csv passes; bad.csv has a long low-battery stretch and a peak; nogps.csv is missing nothing but gps_alt, in
/// all three (the Info group is info only).
fn folder(dir: &Path) -> PathBuf {
    let f = dir.join("flights");
    std::fs::create_dir_all(f.join(".tlmbench/checks")).unwrap();
    std::fs::create_dir_all(f.join(".tlmbench/recipes")).unwrap();
    std::fs::write(f.join(".tlmbench/checks/acceptance.toml"), SET).unwrap();
    std::fs::write(
        f.join(".tlmbench/recipes/power.toml"),
        "schema = 1\n\n[[recipe]]\nname = \"Power\"\nexpr = \"battery_v * current_a\"\nautomatic = false\n",
    )
    .unwrap();
    for (name, low) in [("good", false), ("bad", true)] {
        let mut t = String::from("time,battery_v,current_a\n");
        for i in 0..200 {
            let v = if low && (50..100).contains(&i) { 11.0 } else { 13.0 };
            let a = if low && i == 150 { 6.0 } else { 2.0 };
            t.push_str(&format!("{:.1},{v},{a}\n", i as f64 / 10.0));
        }
        std::fs::write(f.join(format!("{name}.csv")), t).unwrap();
    }
    f
}

#[test]
fn a_folder_run_reports_and_exits_by_result() {
    let (data, cache, out) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let f = folder(data.path());
    let o = run(
        cache.path(),
        &[
            f.to_str().unwrap(),
            "--set",
            "Acceptance",
            "--report",
            "junit,csv,json,html",
            "--out",
            out.path().to_str().unwrap(),
        ],
    );
    let t = text(&o);
    // bad.csv: low battery 5 s (fail), peak 6 A (fail), power 13 * 6 = 78 W (warn); gps_alt is missing everywhere
    assert_eq!(code(&o), 2, "{t}");
    assert!(t.contains("bad.csv") && t.contains("Battery above 12 V · Fail · 1× · 5 s"), "{t}");
    assert!(t.contains("Peak current · Fail"), "{t}");
    assert!(t.contains("max 6 (limit 5)"), "{t}");
    assert!(t.contains("Power under 50 W · Warn"), "{t}");
    assert!(t.contains("good.csv"), "{t}");

    let csv = std::fs::read_to_string(out.path().join("results.csv")).unwrap();
    assert!(csv.starts_with("file,set,group,check,kind,severity,verdict,count,seconds"), "{csv}");
    let line = csv.lines().find(|l| l.contains("bad.csv") && l.contains("Battery above 12 V")).unwrap();
    assert!(line.contains(",fail,1,5.000,11,"), "{line}");
    assert!(csv.lines().any(|l| l.contains("good.csv") && l.contains("Battery above 12 V") && l.contains(",pass,0,")));
    assert!(csv.lines().any(|l| l.contains("Altitude set") && l.contains(",cant,")));

    let junit = std::fs::read_to_string(out.path().join("junit.xml")).unwrap();
    assert!(junit.contains("<testsuite name=") && junit.contains("classname=\"Acceptance.Power\""), "{junit}");
    assert!(junit.contains("<failure message=\"Fail · 1× · 5 s"), "{junit}");
    assert!(
        junit.contains("<error message=\"Can&apos;t run") || junit.contains("<error message=\"Can't run"),
        "{junit}"
    );

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.path().join("results.json")).unwrap()).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 2);
    let bad = json.as_array().unwrap().iter().find(|f| f["file"].as_str().unwrap().ends_with("bad.csv")).unwrap();
    assert_eq!(bad["sets"][0]["verdict"], "fail");
    assert_eq!(bad["sets"][0]["checks"][0]["result"]["runs"][0]["start"], "5000000000");
    let html = std::fs::read_to_string(out.path().join("report.html")).unwrap();
    assert!(html.contains("<h2>Acceptance</h2>") && html.contains("class=\"fail\""), "{html}");
}

#[test]
fn exit_codes_follow_the_worst_thing_that_happened() {
    let (data, cache) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let f = folder(data.path());
    let good = f.join("good.csv");
    let (g, set) = (good.to_str().unwrap(), "Acceptance");
    // good.csv alone: every check passes, and the one that can't run (gps_alt) is 2 unless allowed
    let o = run(cache.path(), &[g, "--set", set]);
    assert_eq!(code(&o), 2, "{}", text(&o));
    let o = run(cache.path(), &[g, "--set", set, "--allow-cant-run"]);
    assert_eq!(code(&o), 0, "{}", text(&o));
    // the failing file is 1
    let o = run(cache.path(), &[f.join("bad.csv").to_str().unwrap(), "--set", set, "--allow-cant-run"]);
    assert_eq!(code(&o), 1, "{}", text(&o));
    // a warning alone passes, unless --fail-on warn
    let warn_only = f.join("warn.csv");
    let mut t = String::from("time,battery_v,current_a\n");
    for i in 0..100 {
        t.push_str(&format!("{:.1},13,{}\n", i as f64 / 10.0, if i == 40 { 4.5 } else { 2.0 }));
    }
    std::fs::write(&warn_only, t).unwrap(); // 13 * 4.5 = 58.5 W: only the warning
    let w = warn_only.to_str().unwrap();
    assert_eq!(code(&run(cache.path(), &[w, "--set", set, "--allow-cant-run"])), 0);
    assert_eq!(code(&run(cache.path(), &[w, "--set", set, "--allow-cant-run", "--fail-on", "warn"])), 1);
    // configuration errors are 3: no set named, a set that isn't there, a mistake in an expression
    assert_eq!(code(&run(cache.path(), &[g])), 3);
    let o = run(cache.path(), &[g, "--set", "Nope"]);
    assert_eq!(code(&o), 3);
    assert!(text(&o).contains("no check set called \"Nope\" (found: Acceptance)"), "{}", text(&o));
    let broken = data.path().join("broken.toml");
    std::fs::write(&broken, "schema = 1\nname = \"Broken\"\n[[check]]\nname = \"x\"\nexpect = \"battery_v >\"\n")
        .unwrap();
    let o = run(cache.path(), &[g, "--sets-from", broken.to_str().unwrap(), "--set", "Broken"]);
    assert_eq!(code(&o), 3, "{}", text(&o));
    // an unreadable file is 2
    assert_eq!(code(&run(cache.path(), &[data.path().join("nothing.csv").to_str().unwrap(), "--set", set])), 2);
}

#[test]
fn results_are_cached_and_follow_the_set() {
    let (data, cache) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let f = folder(data.path());
    let bad = f.join("bad.csv");
    let args = [bad.to_str().unwrap(), "--set", "Acceptance", "--allow-cant-run"];
    let first = text(&run(cache.path(), &args));
    let n = std::fs::read_dir(cache.path().join("checks-results")).unwrap().count();
    assert_eq!(n, 1);
    assert_eq!(text(&run(cache.path(), &args)), first);
    assert_eq!(
        std::fs::read_dir(cache.path().join("checks-results")).unwrap().count(),
        1,
        "the second run reads the cache"
    );
    // changing the set (here, the grace that makes the low stretch count) is a new version
    std::fs::write(f.join(".tlmbench/checks/acceptance.toml"), SET.replace("grace = 0.5", "grace = 6")).unwrap();
    let again = text(&run(cache.path(), &args));
    assert!(!again.contains("Battery above 12 V"), "a 5 s stretch is inside a 6 s grace: {again}");
    assert_eq!(std::fs::read_dir(cache.path().join("checks-results")).unwrap().count(), 2);
}

#[test]
fn a_profile_names_its_sets() {
    let (data, cache) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let f = folder(data.path());
    let profile = data.path().join("profile.json");
    std::fs::write(&profile, r#"{"check_sets": ["Acceptance"]}"#).unwrap();
    let o = run(
        cache.path(),
        &[
            f.join("bad.csv").to_str().unwrap(),
            "--sets-from-profile",
            "--profile",
            profile.to_str().unwrap(),
            "--allow-cant-run",
        ],
    );
    assert_eq!(code(&o), 1, "{}", text(&o));
    assert!(text(&o).contains("Battery above 12 V · Fail"), "{}", text(&o));
}

#[test]
fn twenty_files_run_in_parallel_and_report_in_name_order() {
    let (data, cache, out) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let f = folder(data.path());
    for i in 0..18 {
        std::fs::copy(f.join(if i % 3 == 0 { "bad.csv" } else { "good.csv" }), f.join(format!("f{i:02}.csv"))).unwrap();
    }
    let o = run(
        cache.path(),
        &[
            f.to_str().unwrap(),
            "--set",
            "Acceptance",
            "--jobs",
            "4",
            "--allow-cant-run",
            "--report",
            "csv",
            "--out",
            out.path().to_str().unwrap(),
        ],
    );
    assert_eq!(code(&o), 1, "{}", text(&o));
    let csv = std::fs::read_to_string(out.path().join("results.csv")).unwrap();
    let files: Vec<&str> = csv.lines().skip(1).map(|l| l.split(',').next().unwrap()).collect();
    let mut firsts: Vec<&str> = files.to_vec();
    firsts.dedup();
    assert_eq!(firsts.len(), 20);
    assert!(firsts.windows(2).all(|w| w[0] < w[1]), "name order: {firsts:?}");
    let failing = csv.lines().filter(|l| l.contains("Battery above 12 V") && l.contains(",fail,fail,")).count();
    assert_eq!(failing, 7); // bad.csv and f00, f03, f06, f09, f12, f15
}
