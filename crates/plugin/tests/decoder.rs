//! WASM decoder plugins: the template decodes a file through the whole path (plugin, cache, recording), and a
//! plugin that misbehaves is stopped by the sandbox.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tlmbench_decode::plugin::{DecoderPlugin, Tier, set_plugins};
use tlmbench_decode::{OpenOptions, open};
use tlmbench_plugin::{Limits, WasmDecoder};

mod common;
use common::build;

fn demo_file(dir: &Path, rows: u32) -> PathBuf {
    let mut b = b"DEMO".to_vec();
    b.extend(rows.to_le_bytes());
    for i in 0..rows {
        b.extend((i as u64 * 1000).to_le_bytes()); // µs
        b.extend((i as f32 * 0.25).to_le_bytes());
        b.extend(((i % 7) as i16 - 3).to_le_bytes());
    }
    let p = dir.join("run.demo");
    std::fs::write(&p, b).unwrap();
    p
}

// the plugin list is process-wide
static ONE: Mutex<()> = Mutex::new(());

#[test]
fn the_template_decodes_a_file_through_the_whole_path() {
    let _g = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let wasm = build("template-decoder", "my_decoder.wasm");
    let p = WasmDecoder::load(&wasm).unwrap();
    assert_eq!((p.info().name.as_str(), p.info().tier), ("demo", Tier::Wasm));
    assert_eq!(p.info().files, vec!["*.demo".to_string()]);
    let plugin: Arc<dyn DecoderPlugin> = Arc::new(p);
    set_plugins(vec![plugin]);
    let dir = tempfile::tempdir().unwrap();
    let file = demo_file(dir.path(), 150_000); // more than one chunk
    let opts = OpenOptions { cache_dir: Some(dir.path().join("cache")), ..Default::default() };
    let rec = open(&file, &opts).unwrap();
    let info = rec.info();
    assert_eq!(info.format, "demo (plugin)");
    assert_eq!(info.samples, 150_000);
    assert_eq!(info.channels.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["volts", "status"]);
    assert_eq!(info.channels[0].unit.as_deref(), Some("V"));
    assert_eq!(info.t_last, Some(149_999 * 1_000_000));
    assert_eq!(rec.value_at(0, 100_001, &Default::default()), 100_001.0 * 0.25);
    assert_eq!(rec.value_at(1, 10, &Default::default()), 0.0);
    // a file cut short opens with the records it has
    let cut = dir.path().join("cut.demo");
    let bytes = std::fs::read(&file).unwrap();
    std::fs::write(&cut, &bytes[..8 + 14 * 10 + 5]).unwrap();
    assert_eq!(open(&cut, &opts).unwrap().info().samples, 10);
    set_plugins(vec![]);
}

fn hostile(limits: Limits) -> Arc<dyn DecoderPlugin> {
    Arc::new(WasmDecoder::load_with(&build("test-hostile", "hostile.wasm"), limits).unwrap())
}

fn open_as(mode: &str, limits: Limits) -> Result<usize, String> {
    let _g = ONE.lock().unwrap_or_else(|e| e.into_inner());
    set_plugins(vec![hostile(limits)]);
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("x.dat");
    std::fs::write(&file, format!("{mode} and some more bytes")).unwrap();
    let opts = OpenOptions { cache_dir: Some(dir.path().join("cache")), ..Default::default() };
    let r = open(&file, &opts).map(|r| r.info().samples).map_err(|e| e.to_string());
    set_plugins(vec![]);
    r
}

#[test]
fn a_well_behaved_run_of_the_test_plugin_works() {
    assert_eq!(open_as("OKAY", Limits::DECODER), Ok(1));
}

#[test]
fn a_plugin_that_never_returns_is_stopped() {
    // building the plugin (a cold CI machine takes many seconds) is not what is timed
    build("test-hostile", "hostile.wasm");
    let started = std::time::Instant::now();
    let e = open_as("LOOP", Limits { cpu: std::time::Duration::from_millis(300), ..Limits::DECODER }).unwrap_err();
    assert!(e.contains("ran past its time limit"), "{e}");
    // the limit is 0.3 s; a loop that wasn't stopped would never finish
    assert!(started.elapsed().as_secs() < 20, "stopped promptly: {:?}", started.elapsed());
}

#[test]
fn a_plugin_that_wants_too_much_memory_is_stopped() {
    let e = open_as("MEMO", Limits { memory: 64 << 20, ..Limits::DECODER }).unwrap_err();
    assert!(e.contains("more memory") || e.contains("failed"), "{e}");
}

#[test]
fn a_plugin_cannot_read_files_or_the_clock() {
    let e = open_as("FILE", Limits::DECODER).unwrap_err();
    assert!(e.contains("no file access"), "{e}");
    // the clock stands still: the same answer every time, and not today's date
    assert_eq!(open_as("TIME", Limits::DECODER), Ok(1));
}

#[test]
fn a_chunk_that_does_not_match_its_schema_is_an_error_naming_the_plugin() {
    let e = open_as("BADC", Limits::DECODER).unwrap_err();
    assert!(e.contains("hostile") && e.contains("8 bytes for 2 rows"), "{e}");
}
