//! Plugins on the command line: `tlmbench plugins` lists them, and `info` and `convert` read a file only a plugin
//! decodes. Native plugins stay off until `--native-plugin` names them.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Builds a plugin crate under `plugins/` (needs the wasm32-wasip2 target, which rust-toolchain.toml installs).
fn build(name: &str, wasm: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../plugins").join(name);
    let out = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["build", "--release", "--target", "wasm32-wasip2", "--quiet"])
        .current_dir(&dir)
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTFLAGS")
        .output()
        .expect("cargo runs");
    assert!(out.status.success(), "building {name}: {}", String::from_utf8_lossy(&out.stderr));
    dir.join("target/wasm32-wasip2/release").join(wasm)
}

fn tlmbench(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tlmbench")).args(args).output().unwrap()
}

#[test]
fn a_wasm_decoder_in_the_plugins_folder_opens_its_files_and_is_listed() {
    let dir = tempfile::tempdir().unwrap();
    let plugins = dir.path().join("plugins");
    std::fs::create_dir_all(&plugins).unwrap();
    std::fs::copy(build("template-decoder", "my_decoder.wasm"), plugins.join("my_decoder.wasm")).unwrap();
    let gone = format!("libgone.{}", std::env::consts::DLL_EXTENSION);
    std::fs::write(plugins.join(&gone), b"never loaded").unwrap();
    let (p, cache) = (plugins.to_str().unwrap(), dir.path().join("cache"));
    let cache = cache.to_str().unwrap();

    // a DEMO file: 3 records of (µs, f32, i16)
    let mut bytes = b"DEMO".to_vec();
    bytes.extend(3u32.to_le_bytes());
    for i in 0..3u64 {
        bytes.extend((i * 500).to_le_bytes());
        bytes.extend((i as f32 + 0.5).to_le_bytes());
        bytes.extend((i as i16).to_le_bytes());
    }
    let file = dir.path().join("run.demo");
    std::fs::write(&file, bytes).unwrap();

    // without the plugin the file is unknown
    let none = tlmbench(&[
        "info",
        "--plugins",
        dir.path().join("nothing").to_str().unwrap(),
        "--cache",
        cache,
        file.to_str().unwrap(),
    ]);
    assert!(!none.status.success());

    let out = tlmbench(&["info", "--json", "--plugins", p, "--cache", cache, file.to_str().unwrap()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["format"], "demo (plugin)");
    assert_eq!(v["samples"], 3);
    assert_eq!(v["channels"][0]["name"], "volts");
    assert_eq!(v["duration_ns"], 1_000_000);

    let list = tlmbench(&["plugins", "--json", "--plugins", p]);
    assert!(list.status.success(), "{}", String::from_utf8_lossy(&list.stderr));
    let l: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    let by = |f: &str| l.as_array().unwrap().iter().find(|x| x["file"] == f).unwrap().clone();
    assert_eq!(
        (by("my_decoder.wasm")["tier"].as_str(), by("my_decoder.wasm")["version"].as_str()),
        (Some("wasm"), Some("0.1.0"))
    );
    assert_eq!(by("my_decoder.wasm")["status"]["state"], "loaded");
    assert_eq!(by(&gone)["status"]["state"], "off");
    // the author's harness
    let c = tlmbench(&[
        "plugins",
        "--check",
        plugins.join("my_decoder.wasm").to_str().unwrap(),
        file.to_str().unwrap(),
        "--cache",
        cache,
    ]);
    let c = String::from_utf8_lossy(&c.stdout).to_string();
    assert!(
        c.contains("demo 0.1.0 (decoder)")
            && c.contains("by name yes, by sniff yes")
            && c.contains("decoded 3 samples"),
        "{c}"
    );
    assert!(c.contains("channel volts") && c.contains("channel status"), "{c}");
    let bad = tlmbench(&["plugins", "--check", file.to_str().unwrap()]);
    assert_eq!(bad.status.code(), Some(2), "a file that isn't a plugin");
    let text = String::from_utf8_lossy(&tlmbench(&["plugins", "--plugins", p]).stdout).to_string();
    assert!(text.contains("my_decoder.wasm") && text.contains("loaded") && text.contains("--native-plugin"), "{text}");
}
