//! Builds the plugin crates under `plugins/` for the tests (they need the wasm32-wasip2 target, which
//! rust-toolchain.toml installs). Each crate has a target folder of its own, so this never touches the workspace's.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

static BUILDING: Mutex<()> = Mutex::new(());

fn run(name: &str, extra: &[&str]) -> PathBuf {
    let _g = BUILDING.lock().unwrap_or_else(|e| e.into_inner());
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins").join(name);
    let out = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["build", "--release", "--quiet"])
        .args(extra)
        .current_dir(&dir)
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTFLAGS")
        .output()
        .expect("cargo runs");
    assert!(out.status.success(), "building {name}: {}", String::from_utf8_lossy(&out.stderr));
    dir
}

/// A WASM plugin: `plugins/<name>/target/wasm32-wasip2/release/<wasm>`.
pub fn build(name: &str, wasm: &str) -> PathBuf {
    run(name, &["--target", "wasm32-wasip2"]).join("target/wasm32-wasip2/release").join(wasm)
}

/// A native plugin for this machine: `plugins/<name>/target/release/<lib>`.
pub fn build_native(name: &str, lib: &str) -> PathBuf {
    let file = format!("{}{lib}{}", std::env::consts::DLL_PREFIX, std::env::consts::DLL_SUFFIX);
    run(name, &[]).join("target/release").join(file)
}
