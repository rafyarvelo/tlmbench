//! The payload plugin paths with a plugin written in Rust: the field wrapper hands every call to the right column, and a
//! plugin's failures (no session, a refused schema, an error, too few values, packets with no rows) come back as errors
//! that name the plugin, not as a wrong recording (coverage, R6c.3). The WASM template through the same paths is
//! `crates/plugin/tests/payload.rs`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tlmbench_core::Viewport;
use tlmbench_core::chunks::Held;
use tlmbench_core::mask::Mask;
use tlmbench_core::pyramid::{Pyramid, build_levels};
use tlmbench_decode::descriptor::{DType, Descriptor};
use tlmbench_decode::packets::encode_packet;
use tlmbench_decode::payload::{PayloadPlugin, PayloadSession, set_payload_plugins};
use tlmbench_decode::plugin::{Chunk, PluginChannel, PluginInfo, PluginSchema, Tier};
use tlmbench_decode::{OpenOptions, open_all};

/// What a test makes the plugin do (the descriptor's params say, as JSON; `no_session` is the plugin's own).
#[derive(Debug)]
struct Mock {
    info: PluginInfo,
    no_session: bool,
}

struct Session {
    sent: usize,
    first_call: bool,
}

impl PayloadSession for Session {
    fn packet_schema(&mut self, _type_name: &str, params: &str) -> Result<PluginSchema, String> {
        if params.contains("schema_err") {
            return Err("no such type".into());
        }
        Ok(PluginSchema {
            channels: vec![PluginChannel { name: "n".into(), unit: None, dtype: DType::U8 }],
            timed: true,
        })
    }
    fn decode_packets(
        &mut self,
        _type_name: &str,
        params: &str,
        packets: &[(Vec<u8>, Vec<u8>)],
    ) -> Result<Chunk, String> {
        if params.contains("\"err\"") {
            return Err("boom".into());
        }
        // `empty_first`: the first batch gives no rows, which isn't the end of the file
        if params.contains("empty_first") && std::mem::take(&mut self.first_call) {
            return Ok(Chunk::default());
        }
        let time = (0..packets.len()).map(|k| (self.sent + k) as i64 * 1_000_000).collect();
        self.sent += packets.len();
        Ok(Chunk { rows: packets.len(), time, columns: vec![packets.iter().map(|(_, p)| p[0]).collect()] })
    }
    fn decode_field(
        &mut self,
        _field: &str,
        params: &str,
        _dtype: DType,
        size: u32,
        count: u32,
        data: &[u8],
    ) -> Result<Vec<u8>, String> {
        assert_eq!(data.len(), (size * count) as usize);
        if params.contains("\"err\"") {
            return Err("boom".into());
        }
        let v: Vec<f32> = data.iter().map(|&b| b as f32).collect();
        let mut out: Vec<u8> = bytemuck::cast_slice(&v).to_vec();
        if params.contains("short") {
            out.truncate(out.len() - 4);
        }
        Ok(out)
    }
}

impl PayloadPlugin for Mock {
    fn info(&self) -> &PluginInfo {
        &self.info
    }
    fn session(&self) -> Result<Box<dyn PayloadSession>, String> {
        if self.no_session {
            return Err("out of memory".into());
        }
        Ok(Box::new(Session { sent: 0, first_call: true }))
    }
}

fn install(no_session: bool) {
    let info = PluginInfo {
        name: "mock".into(),
        version: "1.0.0".into(),
        tier: Tier::Wasm,
        description: None,
        files: vec![],
        path: "mock.wasm".into(),
    };
    set_payload_plugins(vec![Arc::new(Mock { info, no_session })]);
}

/// The payload plugins are the process's: the tests that set them take turns.
static REGISTRY: std::sync::Mutex<()> = std::sync::Mutex::new(());

const N: usize = 3000;

/// `v` (a big-endian u16, so the file's own column is chunked) counts 0..=250 over and over, and `w` (a byte through the
/// plugin) is `250 - v`: the two columns differ, so a call that reaches the wrong one is seen.
fn fields_file(dir: &Path, params: &str) -> (PathBuf, OpenOptions) {
    let desc = format!(
        r#"
schema = 1
record_size = 3
endian = "big"
time = {{ source = "rate", rate_hz = 1000.0 }}
fields = [
  {{ name = "v", type = "u16", offset = 0 }},
  {{ name = "w", type = "f32", offset = 2, plugin = "mock", size = 1, params = {params} }},
]
"#
    );
    let data: Vec<u8> = (0..N).flat_map(|i| [0, (i % 251) as u8, 250 - (i % 251) as u8]).collect();
    let (file, toml) = (dir.join("f.bin"), dir.join("f.toml"));
    std::fs::write(&file, data).unwrap();
    std::fs::write(&toml, desc).unwrap();
    (file, OpenOptions { descriptor: Some(toml), cache_dir: Some(dir.join("cache")), ..Default::default() })
}

fn open_err(file: &Path, opts: &OpenOptions) -> String {
    match open_all(file, opts) {
        Ok(_) => panic!("opened"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn the_field_wrapper_gives_a_plugins_column_what_its_own_column_gets() {
    let _turn = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    install(false);
    let tmp = tempfile::tempdir().unwrap();
    let (file, opts) = fields_file(tmp.path(), "{ scale = 1 }");
    let rec = open_all(&file, &opts).unwrap().pop().unwrap();
    assert_eq!(rec.info().channels.len(), 2);
    let h = Held::default();
    assert_eq!(rec.value_at(1, 300, &h), (250 - 300 % 251) as f64);

    // every view and statistic of the plugin's column is the file's column turned over (250 - v)
    let levels: Vec<_> = (0..2).map(|c| build_levels(rec.summarize(c, 0, N))).collect();
    let pyr: Vec<Pyramid> = levels.iter().map(|l| Pyramid::new(N, l.iter().map(|v| v.as_slice()).collect())).collect();
    let vp = Viewport { t0: rec.time_at(0), t1: rec.time_at(N - 1) + 1, width_px: 50, raw_max: 0 };
    let mask = Mask::from_runs(vec![100..200, 1500..1600]);
    // NaN marks a break in the line: compare the bits, since NaN != NaN
    let flip = |t: tlmbench_core::Trace| {
        t.y.iter().map(|y| if y.is_nan() { y.to_bits() } else { (250.0 - y).to_bits() }).collect::<Vec<_>>()
    };
    let ys = |t: tlmbench_core::Trace| t.y.iter().map(|y| y.to_bits()).collect::<Vec<_>>();
    // reducing to the lowest and highest values per pixel column swaps which of the two comes first, so compare the sets
    let sorted = |mut v: Vec<u32>| {
        v.sort_unstable();
        v
    };
    for (what, a, b) in [
        ("coarse", rec.view_coarse(0, vp, &pyr[0], &[], &h), rec.view_coarse(1, vp, &pyr[1], &[], &h)),
        ("masked", rec.view_masked(0, vp, &mask, &[], &h), rec.view_masked(1, vp, &mask, &[], &h)),
        ("exact", rec.view(0, vp, Some(&pyr[0]), &[], &h), rec.view(1, vp, Some(&pyr[1]), &[], &h)),
    ] {
        assert_eq!(a.t.len(), b.t.len(), "{what}");
        assert_eq!(sorted(flip(a)), sorted(ys(b)), "{what}: the plugin's column drawn is the file's turned over");
    }
    let (own_stats, masked) =
        (rec.stats_masked(0, &pyr[0], 0, N, &mask, &h), rec.stats_masked(1, &pyr[1], 0, N, &mask, &h));
    assert_eq!(masked.count as usize, N - 200, "the mask hides 200 samples");
    assert_eq!((masked.count, masked.mean), (own_stats.count, 250.0 - own_stats.mean));
    assert_eq!(
        (masked.min.map(|m| m.0), masked.max.map(|m| m.0)),
        (own_stats.max.map(|m| 250.0 - m.0), own_stats.min.map(|m| 250.0 - m.0))
    );
    let (a, b) = (rec.stats(0, &pyr[0], 10, 2000, &h), rec.stats(1, &pyr[1], 10, 2000, &h));
    assert_eq!((b.count, b.mean), (a.count, 250.0 - a.mean));

    // the rest: shared times, both columns' summaries and chunks, and the file's own bookkeeping
    assert_eq!(rec.read(0, 5, 9).to_f64(1.0, 0.0), [5.0, 6.0, 7.0, 8.0]);
    assert_eq!(rec.read(1, 5, 9).to_f64(1.0, 0.0), [245.0, 244.0, 243.0, 242.0]);
    assert_eq!(rec.read_times(0, 4), (0..4).map(|i| rec.time_at(i)).collect::<Vec<_>>());
    assert_eq!(rec.scale_offset(1), (1.0, 0.0));
    assert!(rec.missing_mask(1).is_empty());
    let all = rec.summarize_all(0, N);
    assert_eq!(all.len(), 2);
    assert_eq!((all[0][0].min, all[1][0].min), (0.0, 0.0));
    assert_eq!((all[0][0].first, all[1][0].first), (0.0, 250.0), "each column's own summaries");
    assert_eq!(rec.summarize(1, 0, N), all[1]);
    rec.prefetch(&[0..N]);
    // the file's own big-endian column is chunked and decodes from the file; the plugin's cached column isn't
    assert!((rec.chunked(0), rec.chunked(1)) == (true, false));
    let chunks = rec.decode_chunks(&[0], 0..1);
    assert_eq!(chunks[0][0].values::<u16>()[..3], [0, 1, 2]);
    assert_eq!(rec.decode_chunks(&[1, 0], 0..0).len(), 2, "one list per channel asked, in the order asked");
    rec.set_time_index(rec.time_index());
    assert_eq!(rec.lower_bound(rec.time_at(777)), 777);
    let mut gaps = rec.gap_index();
    rec.extend_gap_index(&mut gaps, 0);
    assert!(gaps.breaks(4.0).is_empty(), "evenly spaced: no gaps");
    assert!(rec.layout_id().windows(14).any(|w| w == b"+plugin-fields"));

    // opened again, it comes from the cache entry the first open built, even if the plugin would now fail
    install(true);
    assert_eq!(open_all(&file, &opts).unwrap().pop().unwrap().value_at(1, 5, &h), 245.0);
    set_payload_plugins(Vec::new());
}

#[test]
fn a_field_plugin_that_fails_or_gives_too_few_values_is_an_error_naming_it() {
    let _turn = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    install(false);
    let (file, opts) = fields_file(tmp.path(), r#"{ mode = "err" }"#);
    let err = open_err(&file, &opts);
    assert!(err.contains("mock (field \"w\"): boom"), "{err}");
    let (file, opts) = fields_file(tmp.path(), r#"{ mode = "short" }"#);
    let err = open_err(&file, &opts);
    assert!(err.contains("mock (field \"w\")") && err.contains("returned 11996 bytes for 3000 values of f32"), "{err}");
    // a plugin that can't start a session
    install(true);
    let (file, opts) = fields_file(tmp.path(), r#"{ x = 1 }"#);
    let err = open_err(&file, &opts);
    assert!(err.contains("mock: out of memory"), "{err}");
    set_payload_plugins(Vec::new());
}

const STATUS: &str = r#"
schema = 1
name = "pk"

[packets]
sync = "EB90"
header_size = 6
length = { offset = 2, type = "u16" }
id = { offset = 4, type = "u8" }
checksum = "crc16-ccitt"

[[packet_types]]
id = 1
name = "status"
plugin = "mock"
params = PARAMS
"#;

/// `count` packets of type 1 whose payload is one byte, `i % 200`; packet `damage` (if any) has a flipped payload bit.
fn packets_file(dir: &Path, params: &str, count: usize, damage: Option<usize>) -> (PathBuf, OpenOptions) {
    let text = STATUS.replace("PARAMS", params);
    let p = Descriptor::parse(&text).unwrap().packets.unwrap();
    let mut data = Vec::new();
    for i in 0..count {
        let mut pk = encode_packet(&p, false, &[0, 0, 0, 0, 1, 0], &[(i % 200) as u8, 7, 7]).unwrap();
        if damage == Some(i) {
            pk[6] ^= 0x01;
        }
        data.extend(pk);
    }
    let (file, toml) = (dir.join("p.bin"), dir.join("p.toml"));
    std::fs::write(&file, data).unwrap();
    std::fs::write(&toml, text).unwrap();
    (file, OpenOptions { descriptor: Some(toml), cache_dir: Some(dir.join("cache")), ..Default::default() })
}

#[test]
fn a_packet_type_plugin_skips_damaged_packets_and_goes_on_past_a_batch_with_no_rows() {
    let _turn = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    install(false);
    let tmp = tempfile::tempdir().unwrap();
    // 4100 packets: the first batch (4096) gives no rows, the second (4) does; one packet is damaged
    let (file, opts) = packets_file(tmp.path(), r#"{ mode = "empty_first" }"#, 4100, Some(5));
    let rec = open_all(&file, &opts).unwrap().pop().unwrap();
    let h = Held::default();
    // the first batch is 4096 intact packets (0..=4096 without the damaged 5) and gives no rows
    assert_eq!(rec.info().samples, 3);
    assert_eq!(rec.value_at(0, 0, &h), (4097 % 200) as f64, "the second batch starts at packet 4097");
    let notes = rec.info().notes.join(" ");
    assert!(notes.contains("1 damaged packets skipped"), "{notes}");
    assert!(notes.contains("Decoded by the plugin \"mock\" 1.0.0"), "{notes}");
    // from the cache the second time, with its damaged count
    let again = open_all(&file, &opts).unwrap().pop().unwrap();
    assert!(again.info().notes.join(" ").contains("1 damaged packets skipped"));
    set_payload_plugins(Vec::new());
}

#[test]
fn a_packet_type_plugin_that_fails_to_start_or_to_decode_is_an_error_naming_it() {
    let _turn = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    install(false);
    let (file, opts) = packets_file(tmp.path(), r#"{ mode = "schema_err" }"#, 10, None);
    assert!(open_err(&file, &opts).contains("mock: no such type"));
    let (file, opts) = packets_file(tmp.path(), r#"{ mode = "err" }"#, 10, None);
    assert!(open_err(&file, &opts).contains("boom"));
    install(true);
    let (file, opts) = packets_file(tmp.path(), r#"{ mode = "ok" }"#, 10, None);
    assert!(open_err(&file, &opts).contains("mock: out of memory"));
    set_payload_plugins(Vec::new());
}
