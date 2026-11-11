//! WASM alignment estimators: the template lines two files up through the store's Plugin method.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use tlmbench_core::timemap::TimeMap;
use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec};
use tlmbench_decode::reference;
use tlmbench_plugin::WasmEstimator;
use tlmbench_store::Store;
use tlmbench_store::align::{EstimatorPlugin, Method, estimate, set_estimators};

fn build(name: &str, wasm: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins").join(name);
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

fn descriptor() -> Descriptor {
    Descriptor {
        sequence: None,
        files: Vec::new(),
        packet_types: Vec::new(),
        packets: None,
        schema: SCHEMA,
        name: None,
        endian: Endian::Little,
        header_size: 0,
        record_size: 4,
        time: TimeSpec::Rate { rate_hz: 1000.0, t0_ns: 0 },
        fields: vec![Field {
            name: "x".into(),
            dtype: DType::F32,
            offset: 0,
            unit: None,
            scale: 1.0,
            value_offset: 0.0,
            bits: None,
        }],
    }
}

/// 10 s at 1 kHz with one triangular peak at `peak_s` on the file's own clock.
fn write(path: &Path, peak_s: f64) {
    let mut f = fs::File::create(path).unwrap();
    reference::write_header(&mut f, &descriptor()).unwrap();
    let buf: Vec<u8> =
        (0..10_000).flat_map(|i| ((5.0 - (i as f64 / 1000.0 - peak_s).abs()).max(0.0) as f32).to_le_bytes()).collect();
    f.write_all(&buf).unwrap();
}

static ONE: Mutex<()> = Mutex::new(());

#[test]
fn the_template_estimator_lines_two_files_up_through_the_store() {
    let _g = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let p = WasmEstimator::load(&build("template-estimator", "my_estimator.wasm")).unwrap();
    assert_eq!(p.name(), "peak");
    set_estimators(vec![Arc::new(p)]);
    let dir = tempfile::tempdir().unwrap();
    let (pa, pb) = (dir.path().join("a.tlm"), dir.path().join("b.tlm"));
    write(&pa, 5.0);
    write(&pb, 2.5);
    let store = Store::new(Some(&dir.path().join("cache")));
    let (a, b) = (store.open(&pa, None).unwrap(), store.open(&pb, None).unwrap());
    let method = Method::Plugin { name: "peak".into(), params: r#"{"channel":"x"}"#.into() };
    let got = estimate(dir.path(), &a, &TimeMap::identity(), &b, &method).unwrap();
    assert_eq!(got.note.as_deref(), Some("Peaks of x matched"));
    // b's peak, 2.5 s on its clock, is a's 5.0 s: b's whole clock moves 2.5 s later
    assert_eq!(got.map.apply(2_500_000_000), 5_000_000_000);
    assert_eq!(got.map.apply(0), 2_500_000_000);

    // the primary's own map is respected: its times arrive on the shared timeline
    let moved = estimate(dir.path(), &a, &TimeMap::offset(1_000_000_000), &b, &method).unwrap();
    assert_eq!(moved.map.apply(2_500_000_000), 6_000_000_000);

    // a parameter the plugin can't use is its error, named
    let bad = Method::Plugin { name: "peak".into(), params: "{}".into() };
    let e = estimate(dir.path(), &a, &TimeMap::identity(), &b, &bad).unwrap_err().to_string();
    assert!(e.contains("peak: give the channel"), "{e}");
    // a channel one file lacks
    let nope = Method::Plugin { name: "peak".into(), params: r#"{"channel":"y"}"#.into() };
    let e = estimate(dir.path(), &a, &TimeMap::identity(), &b, &nope).unwrap_err().to_string();
    assert!(e.contains("Both files need a channel named y"), "{e}");
    // and a plugin that isn't installed
    set_estimators(vec![]);
    let e = estimate(dir.path(), &a, &TimeMap::identity(), &b, &method).unwrap_err().to_string();
    assert!(e.contains("No alignment plugin named peak"), "{e}");
}
