//! A decoder plugin's chunks become a cache entry that opens like any other recording.

use std::path::Path;
use std::sync::Arc;

use tlmbench_decode::descriptor::DType;
use tlmbench_decode::plugin::*;
use tlmbench_decode::{OpenOptions, open};

#[derive(Debug)]
struct Counting {
    info: PluginInfo,
    bad_time: bool,
}

struct Session {
    schema: PluginSchema,
    left: usize,
    next_row: i64,
    bad_time: bool,
}

impl DecoderSession for Session {
    fn schema(&self) -> &PluginSchema {
        &self.schema
    }
    fn next(&mut self, max_rows: u32) -> Result<Option<Chunk>, String> {
        let n = self.left.min(max_rows as usize).min(4);
        if n == 0 {
            return Ok(None);
        }
        let rows: Vec<i64> = (self.next_row..self.next_row + n as i64).collect();
        self.next_row += n as i64;
        self.left -= n;
        let time = rows.iter().map(|r| if self.bad_time && *r == 5 { 0 } else { r * 1_000_000 }).collect();
        let v: Vec<f32> = rows.iter().map(|r| *r as f32 * 0.5).collect();
        Ok(Some(Chunk { rows: n, time, columns: vec![bytemuck::cast_slice(&v).to_vec()] }))
    }
}

impl DecoderPlugin for Counting {
    fn info(&self) -> &PluginInfo {
        &self.info
    }
    fn sniff(&self, head: &[u8], _name: &str) -> bool {
        head.starts_with(b"CNT1")
    }
    fn open(&self, _path: &Path) -> Result<Box<dyn DecoderSession>, String> {
        Ok(Box::new(Session {
            schema: PluginSchema {
                channels: vec![PluginChannel { name: "v".into(), unit: Some("V".into()), dtype: DType::F32 }],
                timed: true,
            },
            left: 10,
            next_row: 0,
            bad_time: self.bad_time,
        }))
    }
}

fn plugin(bad_time: bool) -> Arc<dyn DecoderPlugin> {
    Arc::new(Counting {
        info: PluginInfo {
            name: "counting".into(),
            version: "1.0.0".into(),
            tier: Tier::Wasm,
            description: None,
            files: vec![],
            path: "counting.wasm".into(),
        },
        bad_time,
    })
}

// the registry is process-wide: one test at a time
static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn a_plugins_chunks_open_as_a_recording_and_are_cached() {
    let _g = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("run.cnt");
    std::fs::write(&file, b"CNT1 whatever").unwrap();
    set_plugins(vec![plugin(false)]);
    let opts = OpenOptions { cache_dir: Some(dir.path().join("cache")), ..Default::default() };
    let rec = open(&file, &opts).unwrap();
    let info = rec.info();
    assert_eq!(info.format, "counting (plugin)");
    assert_eq!(info.samples, 10);
    assert_eq!(info.channels[0].name, "v");
    assert_eq!(info.channels[0].unit.as_deref(), Some("V"));
    assert_eq!(info.channels[0].dtype, "f32");
    assert_eq!(info.t_last, Some(9_000_000));
    // reopened from the cache: the plugin isn't asked again
    set_plugins(vec![plugin(true)]);
    assert_eq!(open(&file, &opts).unwrap().info().samples, 10);
    set_plugins(vec![]);
}

#[test]
fn a_file_no_plugin_claims_stays_unknown_and_bad_chunks_are_errors() {
    let _g = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("run.cnt");
    std::fs::write(&file, b"CNT1 whatever").unwrap();
    let opts = OpenOptions { cache_dir: Some(dir.path().join("cache")), ..Default::default() };
    set_plugins(vec![]);
    assert!(open(&file, &opts).is_err());
    set_plugins(vec![plugin(true)]);
    let e = open(&file, &opts).unwrap_err().to_string();
    assert!(e.contains("time goes backwards at row 6"), "{e}");
    set_plugins(vec![]);
}
