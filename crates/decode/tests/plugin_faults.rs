//! A decoder plugin that misbehaves: each fault is an error that names the plugin, never a recording with wrong data
//! (coverage, R6c.3). Also how a plugin claims a file (by name pattern or by sniffing) and a reference file's header.

use std::path::Path;
use std::sync::Arc;

use tlmbench_decode::descriptor::{DType, Descriptor};
use tlmbench_decode::plugin::*;
use tlmbench_decode::reference;
use tlmbench_decode::{OpenOptions, open};

#[derive(Debug)]
struct Faulty {
    info: PluginInfo,
}

struct Session {
    schema: PluginSchema,
    fault: String,
    sent: bool,
}

impl DecoderSession for Session {
    fn schema(&self) -> &PluginSchema {
        &self.schema
    }
    fn next(&mut self, _max_rows: u32) -> Result<Option<Chunk>, String> {
        if std::mem::replace(&mut self.sent, true) {
            return Ok(None);
        }
        let ok = |n: usize| vec![0u8; n * 4];
        match self.fault.as_str() {
            "next_error" => Err("the file ended in the middle of a record".into()),
            "few_columns" => Ok(Some(Chunk { rows: 2, time: vec![0, 1], columns: vec![ok(2)] })),
            "short_column" => Ok(Some(Chunk { rows: 2, time: vec![0, 1], columns: vec![ok(2), ok(1)] })),
            "few_times" => Ok(Some(Chunk { rows: 2, time: vec![0], columns: vec![ok(2), ok(2)] })),
            "no_rows" => Ok(Some(Chunk::default())),
            _ => unreachable!(),
        }
    }
}

impl DecoderPlugin for Faulty {
    fn info(&self) -> &PluginInfo {
        &self.info
    }
    fn sniff(&self, _head: &[u8], _name: &str) -> bool {
        false
    }
    fn open(&self, path: &Path) -> Result<Box<dyn DecoderSession>, String> {
        // the fault is the file's extension: `x.dup`, `x.next_error`, ...
        let fault = path.extension().unwrap().to_string_lossy().into_owned();
        if fault == "open_error" {
            return Err("can't read this version".into());
        }
        let name = |n: &str| PluginChannel { name: n.into(), unit: None, dtype: DType::F32 };
        let channels = match fault.as_str() {
            "dup" => vec![name("a"), name("a")],
            "empty_name" => vec![name("")],
            _ => vec![name("a"), name("b")],
        };
        Ok(Box::new(Session { schema: PluginSchema { channels, timed: true }, fault, sent: false }))
    }
}

fn faulty() -> Arc<dyn DecoderPlugin> {
    Arc::new(Faulty {
        info: PluginInfo {
            name: "faulty".into(),
            version: "1.0.0".into(),
            tier: Tier::Wasm,
            description: None,
            // it claims files by name, not by sniffing
            files: vec![
                "*.dup".into(),
                "*.empty_name".into(),
                "*.open_error".into(),
                "*.next_error".into(),
                "*.few_*".into(),
                "*.short_column".into(),
                "*.no_rows".into(),
            ],
            path: "faulty.wasm".into(),
        },
    })
}

#[test]
fn a_plugins_faults_are_errors_that_name_it() {
    let dir = tempfile::tempdir().unwrap();
    let opts = OpenOptions { cache_dir: Some(dir.path().join("cache")), ..Default::default() };
    set_plugins(vec![faulty()]);
    for (ext, want) in [
        ("dup", "channel names must be unique and not empty (\"a\")"),
        ("empty_name", "channel names must be unique and not empty (\"\")"),
        ("open_error", "faulty: can't read this version"),
        ("next_error", "faulty: the file ended in the middle of a record"),
        ("few_columns", "a chunk has 1 columns, the schema 2"),
        ("short_column", "column \"b\" has 4 bytes for 2 rows of f32"),
        ("few_times", "a chunk has 1 times for 2 rows"),
    ] {
        let file = dir.path().join(format!("x.{ext}"));
        std::fs::write(&file, b"data").unwrap();
        let err = open(&file, &opts).err().unwrap_or_else(|| panic!("{ext} opened")).to_string();
        assert!(err.contains("faulty") && err.contains(want), "{ext}: {err}");
    }
    // a first chunk with no rows ends the file: an empty recording, not an error
    let file = dir.path().join("x.no_rows");
    std::fs::write(&file, b"data").unwrap();
    assert_eq!(open(&file, &opts).unwrap().info().samples, 0);
    set_plugins(vec![]);
}

#[test]
fn a_reference_files_header_is_checked() {
    let desc =
        Descriptor::parse("schema = 1\nrecord_size = 4\nfields = [{ name = \"a\", type = \"f32\", offset = 0 }]\n")
            .unwrap();
    let mut h = Vec::new();
    let len = reference::write_header(&mut h, &desc).unwrap();
    assert_eq!((len % 64, h.len()), (0, len));
    assert_eq!(reference::read_header(&h).unwrap().0, len);
    let err = |b: &[u8]| reference::read_header(b).err().unwrap().to_string();
    assert!(err(&h[..10]).contains("not a tlmbench reference file"));
    let mut wrong = h.clone();
    wrong[0] = b'X';
    assert!(err(&wrong).contains("not a tlmbench reference file"));
    let mut version = h.clone();
    version[8] = 9;
    assert!(err(&version).contains("version 9 is not supported (this version reads 1)"));
    let mut short = h.clone();
    short.truncate(len - 64);
    assert!(err(&short).contains("doesn't fit"), "{}", err(&short));
    let mut utf = h.clone();
    utf[24] = 0xff;
    assert!(err(&utf).contains("descriptor is not UTF-8"));
}
