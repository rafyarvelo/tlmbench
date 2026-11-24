//! The template payload plugin through the whole path: a descriptor names it for one packet type and for one field of
//! another; the file opens as two recordings with the plugin's values (ledger D-ingest-09).

mod common;

use std::path::Path;
use std::sync::Arc;

use tlmbench_core::chunks::Held;
use tlmbench_decode::descriptor::Descriptor;
use tlmbench_decode::packets::encode_packet;
use tlmbench_decode::payload::set_payload_plugins;
use tlmbench_decode::{OpenOptions, open_all};
use tlmbench_plugin::WasmPayload;

const ROVER: &str = r#"
schema = 1
name = "rover"

[packets]
sync = "EB90"
header_size = 6
length = { offset = 2, type = "u16" }
id = { offset = 4, type = "u8" }
checksum = "crc16-ccitt"
compression = "zstd"

[[packet_types]]
id = 1
name = "imu"
record_size = 12
time = { source = "field", field = "t", unit = "us" }
fields = [
  { name = "t", type = "u64", offset = 6 },
  { name = "pos", type = "f64", offset = 14, plugin = "demo-payload", size = 3, params = { scale = 0.5 }, unit = "m" },
  { name = "count", type = "u8", offset = 17 },
]

[[packet_types]]
id = 2
name = "status"
plugin = "demo-payload"
params = { temp_scale = 0.5 }
"#;

/// The payload plugins are the process's: the tests that set them take turns.
static REGISTRY: std::sync::Mutex<()> = std::sync::Mutex::new(());

const PACKETS: usize = 20;
const PER: usize = 4;

/// The imu packet `k`'s records: time 1 ms apart, `pos` a 24-bit big-endian count from -40, `count` the record number.
fn imu(k: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for j in 0..PER {
        let i = k * PER + j;
        out.extend_from_slice(&(i as u64 * 1000).to_le_bytes());
        let v = i as i32 - 40;
        out.extend_from_slice(&[(v >> 16) as u8, (v >> 8) as u8, v as u8]);
        out.push(i as u8);
    }
    out
}

/// The status packet `k`: a time in µs, a status word (mode `k % 16`, a fault on odd packets) and a temperature.
fn status(k: usize) -> Vec<u8> {
    let mut out = (k as u64 * 4000 + 500).to_le_bytes().to_vec();
    let word = (k as u16 & 0x0f) | if k % 2 == 1 { 0x8000 } else { 0 };
    out.extend_from_slice(&word.to_le_bytes());
    out.extend_from_slice(&(100 + k as i16).to_le_bytes());
    out
}

fn write_rover(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let d = Descriptor::parse(ROVER).unwrap();
    let p = d.packets.clone().unwrap();
    let mut data = Vec::new();
    for k in 0..PACKETS {
        data.extend(encode_packet(&p, false, &[0, 0, 0, 0, 1, 0], &imu(k)).unwrap());
        data.extend(encode_packet(&p, false, &[0, 0, 0, 0, 2, 0], &status(k)).unwrap());
    }
    let (file, toml) = (dir.join("rover.bin"), dir.join("rover.toml"));
    std::fs::write(&file, data).unwrap();
    std::fs::write(&toml, ROVER).unwrap();
    (file, toml)
}

#[test]
fn a_descriptor_hands_a_packet_type_and_a_field_to_the_template_payload_plugin() {
    let _turn = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    let wasm = common::build("template-payload", "my_payload.wasm");
    set_payload_plugins(vec![Arc::new(WasmPayload::load(&wasm).unwrap())]);
    let tmp = tempfile::tempdir().unwrap();
    let (file, toml) = write_rover(tmp.path());
    let opts = OpenOptions { descriptor: Some(toml), cache_dir: Some(tmp.path().join("cache")), ..Default::default() };
    let all = open_all(&file, &opts).unwrap();
    let h = Held::default();
    let names = |k: usize| all[k].info().channels.iter().map(|c| c.name.clone()).collect::<Vec<_>>();

    // imu: its own field, then the plugin's
    assert_eq!(all[0].info().part.as_deref(), Some("imu"));
    assert_eq!(names(0), ["count", "pos"]);
    assert_eq!(all[0].info().samples, PACKETS * PER);
    for i in [0, 7, 40, 79] {
        assert_eq!(all[0].value_at(0, i, &h), i as f64, "count {i}");
        assert_eq!(all[0].value_at(1, i, &h), (i as f64 - 40.0) * 0.5, "pos {i}");
    }
    assert_eq!(all[0].time_at(41), 41_000_000);
    assert_eq!(all[0].info().channels[1].unit.as_deref(), Some("m"));

    // status: every channel from the plugin, with its times
    assert_eq!(all[1].info().part.as_deref(), Some("status"));
    assert_eq!(names(1), ["mode", "fault", "temp"]);
    assert_eq!(all[1].info().samples, PACKETS);
    assert_eq!(all[1].time_at(3), 12_500_000);
    assert_eq!((all[1].value_at(0, 13, &h), all[1].value_at(1, 13, &h)), (13.0, 1.0));
    assert_eq!(all[1].value_at(1, 4, &h), 0.0);
    assert_eq!(all[1].value_at(2, 6, &h), 53.0);
    assert!(all[1].info().notes.iter().any(|n| n.contains("demo-payload")), "{:?}", all[1].info().notes);

    // opened again, both come from the cache entries the first open built
    let again = open_all(&file, &opts).unwrap();
    assert_eq!((again[0].value_at(1, 79, &h), again[1].value_at(2, 6, &h)), (19.5, 53.0));
    // without the plugin a file it decodes doesn't open, cached or not: its values are the plugin's (and its version's)
    set_payload_plugins(Vec::new());
    let Err(err) = open_all(&file, &opts) else { panic!("opened without the plugin") };
    let err = err.to_string();
    assert!(err.contains("needs the plugin \"demo-payload\""), "{err}");
}

#[test]
fn a_type_a_plugin_decodes_needs_a_length_and_no_layout_of_its_own() {
    let no_length =
        ROVER.replace("length = { offset = 2, type = \"u16\" }\n", "").replace("compression = \"zstd\"\n", "");
    let err = Descriptor::parse(&no_length).err().unwrap().to_string();
    assert!(err.contains("needs packets.length"), "{err}");
    let layout = ROVER.replace("params = { temp_scale = 0.5 }", "params = { temp_scale = 0.5 }\nrecord_size = 4");
    let err = Descriptor::parse(&layout).err().unwrap().to_string();
    assert!(err.contains("the plugin gives its channels"), "{err}");
    let no_size = ROVER.replace("size = 3, ", "");
    let err = Descriptor::parse(&no_size).err().unwrap().to_string();
    assert!(err.contains("needs a size"), "{err}");
    let scaled = ROVER.replace("size = 3, ", "size = 3, scale = 2.0, ");
    let err = Descriptor::parse(&scaled).err().unwrap().to_string();
    assert!(err.contains("no bits, scale or value_offset"), "{err}");
}

#[test]
fn a_fixed_width_file_hands_a_field_to_the_plugin() {
    let _turn = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    let wasm = common::build("template-payload", "my_payload.wasm");
    let plugin = Arc::new(WasmPayload::load(&wasm).unwrap());
    let tmp = tempfile::tempdir().unwrap();
    let desc = r#"
schema = 1
record_size = 11
time = { source = "rate", rate_hz = 100.0 }
fields = [
  { name = "volts", type = "u8", offset = 0 },
  { name = "pos", type = "f32", offset = 1, plugin = "demo-payload", size = 3 },
  { name = "big", type = "i64", offset = 4, plugin = "demo-payload", size = 7, params = { scale = 2 } },
]
"#;
    let mut data = Vec::new();
    for i in 0..1000i64 {
        data.push((i % 200) as u8);
        let v = -i;
        data.extend_from_slice(&[(v >> 16) as u8, (v >> 8) as u8, v as u8]);
        let w = i * 1_000_000;
        data.extend_from_slice(&w.to_be_bytes()[1..]);
    }
    let (file, toml) = (tmp.path().join("bench.bin"), tmp.path().join("bench.toml"));
    std::fs::write(&file, data).unwrap();
    std::fs::write(&toml, desc).unwrap();
    set_payload_plugins(vec![plugin]);
    let opts = OpenOptions { descriptor: Some(toml), cache_dir: Some(tmp.path().join("cache")), ..Default::default() };
    let rec = open_all(&file, &opts).unwrap().pop().unwrap();
    let h = Held::default();
    let names: Vec<_> = rec.info().channels.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["volts", "pos", "big"]);
    assert_eq!(rec.info().samples, 1000);
    assert_eq!(rec.value_at(0, 333, &h), 133.0);
    assert_eq!(rec.value_at(1, 333, &h), -333.0);
    assert_eq!(rec.value_at(2, 999, &h), 999.0 * 2e6);
    // the plugin's columns share the file's times (100 Hz from zero)
    assert_eq!(rec.time_at(250), 2_500_000_000);
}

/// A field a payload plugin decodes, through the store as the app reads it: the pyramid builds over it, and views
/// (exact and coarse), statistics and reads of its column all come from the plugin's values (coverage, Nov 24: only
/// single values through the wrapper were tested).
#[test]
fn a_plugin_field_draws_and_counts_through_the_store() {
    use tlmbench_core::Viewport;
    use tlmbench_store::{Precision, Store};
    let _turn = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    let wasm = common::build("template-payload", "my_payload.wasm");
    set_payload_plugins(vec![Arc::new(WasmPayload::load(&wasm).unwrap())]);
    let tmp = tempfile::tempdir().unwrap();
    let desc = r#"
schema = 1
record_size = 4
time = { source = "rate", rate_hz = 100.0 }
fields = [
  { name = "volts", type = "u8", offset = 0 },
  { name = "pos", type = "f32", offset = 1, plugin = "demo-payload", size = 3 },
]
"#;
    // pos is -i: the plugin reads it from three big-endian bytes
    let mut data = Vec::new();
    for i in 0..5000i64 {
        data.push((i % 200) as u8);
        let v = -i;
        data.extend_from_slice(&[(v >> 16) as u8, (v >> 8) as u8, v as u8]);
    }
    let (file, toml) = (tmp.path().join("bench.bin"), tmp.path().join("bench.toml"));
    std::fs::write(&file, data).unwrap();
    std::fs::write(&toml, desc).unwrap();
    let store = Store::new(Some(&tmp.path().join("cache")));
    let o = store.open(&file, Some(&toml)).unwrap();
    o.build(&mut |_, _| {}).unwrap();
    let rec = o.recording();
    assert_eq!(rec.info().channels.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["volts", "pos"]);

    let (scale, offset) = rec.scale_offset(1);
    assert_eq!(rec.read(1, 330, 333).to_f64(scale, offset), [-330.0, -331.0, -332.0]);
    let (t0, t1) = (rec.time_at(0), rec.time_at(4999) + 1);
    let st = o.stats(&[1], t0, t1).unwrap().pop().unwrap();
    assert_eq!(
        (st.count, st.min.map(|m| m.0), st.max.map(|m| m.0), st.mean),
        (5000, Some(-4999.0), Some(0.0), -2499.5)
    );
    // a narrower span, inside the file
    let part = o.stats(&[1], rec.time_at(1000), rec.time_at(2000)).unwrap().pop().unwrap();
    assert_eq!((part.count, part.min.map(|m| m.0), part.max.map(|m| m.0)), (1000, Some(-1999.0), Some(-1000.0)));

    let vp = Viewport { t0, t1, width_px: 200, raw_max: 0 };
    for precision in [Precision::Exact, Precision::Coarse] {
        let v = o.view(1, vp, 4.0, precision);
        let y: Vec<f32> = v.trace.y.iter().copied().filter(|y| y.is_finite()).collect();
        assert!(!y.is_empty(), "{precision:?}: something to draw");
        let (lo, hi) = y.iter().fold((f32::MAX, f32::MIN), |(a, b), &x| (a.min(x), b.max(x)));
        assert_eq!((lo, hi), (-4999.0, 0.0), "{precision:?}: the view spans the plugin's values");
    }
}
