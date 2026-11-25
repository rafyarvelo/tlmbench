//! A decoder plugin that gives a column of every value type, with and without times: each is cached in its own type and
//! reads, draws and sums the same (coverage, R6c.3: the cache's typed columns).

use std::path::Path;
use std::sync::Arc;

use tlmbench_core::chunks::Held;
use tlmbench_core::{TraceKind, Viewport};
use tlmbench_decode::descriptor::DType;
use tlmbench_decode::plugin::*;
use tlmbench_decode::{OpenOptions, open};

const TYPES: [DType; 10] = [
    DType::I8,
    DType::I16,
    DType::I32,
    DType::I64,
    DType::U8,
    DType::U16,
    DType::U32,
    DType::U64,
    DType::F32,
    DType::F64,
];
const ROWS: usize = 700;

/// Row `i`'s value in every column: -50..50 repeating, so it fits i8 (the unsigned ones add 50).
fn value(t: DType, i: usize) -> f64 {
    let v = (i % 100) as f64 - 50.0;
    if matches!(t, DType::U8 | DType::U16 | DType::U32 | DType::U64) { v + 50.0 } else { v }
}

fn bytes_of(t: DType, i: usize) -> Vec<u8> {
    let v = value(t, i);
    match t {
        DType::I8 => (v as i8).to_le_bytes().to_vec(),
        DType::I16 => (v as i16).to_le_bytes().to_vec(),
        DType::I32 => (v as i32).to_le_bytes().to_vec(),
        DType::I64 => (v as i64).to_le_bytes().to_vec(),
        DType::U8 => (v as u8).to_le_bytes().to_vec(),
        DType::U16 => (v as u16).to_le_bytes().to_vec(),
        DType::U32 => (v as u32).to_le_bytes().to_vec(),
        DType::U64 => (v as u64).to_le_bytes().to_vec(),
        DType::F32 => (v as f32).to_le_bytes().to_vec(),
        DType::F64 => v.to_le_bytes().to_vec(),
    }
}

#[derive(Debug)]
struct AllTypes {
    info: PluginInfo,
    timed: bool,
}

struct Session {
    schema: PluginSchema,
    done: bool,
}

impl DecoderSession for Session {
    fn schema(&self) -> &PluginSchema {
        &self.schema
    }
    fn next(&mut self, _max_rows: u32) -> Result<Option<Chunk>, String> {
        if std::mem::replace(&mut self.done, true) {
            return Ok(None);
        }
        let time = if self.schema.timed { (0..ROWS as i64).map(|i| i * 1_000_000).collect() } else { vec![] };
        let columns = TYPES.iter().map(|&t| (0..ROWS).flat_map(|i| bytes_of(t, i)).collect()).collect();
        Ok(Some(Chunk { rows: ROWS, time, columns }))
    }
}

impl DecoderPlugin for AllTypes {
    fn info(&self) -> &PluginInfo {
        &self.info
    }
    fn sniff(&self, head: &[u8], _name: &str) -> bool {
        head.starts_with(b"ALLT")
    }
    fn open(&self, _path: &Path) -> Result<Box<dyn DecoderSession>, String> {
        let channels =
            TYPES.iter().map(|t| PluginChannel { name: t.name().to_string(), unit: None, dtype: *t }).collect();
        Ok(Box::new(Session { schema: PluginSchema { channels, timed: self.timed }, done: false }))
    }
}

fn check(timed: bool) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("run.allt");
    std::fs::write(&file, b"ALLT").unwrap();
    let info = PluginInfo {
        name: "allt".into(),
        version: "1.0.0".into(),
        tier: Tier::Wasm,
        description: None,
        files: vec![],
        path: "allt.wasm".into(),
    };
    set_plugins(vec![Arc::new(AllTypes { info, timed })]);
    let opts = OpenOptions { cache_dir: Some(dir.path().join("cache")), ..Default::default() };
    // built, then from the cache
    for pass in ["built", "cached"] {
        let rec = open(&file, &opts).unwrap();
        assert_eq!(rec.info().samples, ROWS, "{pass}");
        let held = Held::default();
        let vp = Viewport { t0: 0, t1: ROWS as i64 * 1_000_000_000, width_px: 20, raw_max: 0 };
        for (c, t) in TYPES.iter().enumerate() {
            assert_eq!(rec.info().channels[c].dtype, t.name(), "{pass}");
            let want: Vec<f64> = (0..ROWS).map(|i| value(*t, i)).collect();
            assert_eq!(rec.read(c, 0, ROWS).to_f64(1.0, 0.0), want, "{pass}: {} column", t.name());
            assert_eq!(rec.value_at(c, 333, &held), want[333], "{pass}: {}", t.name());
            let sums = rec.summarize(c, 0, ROWS);
            let (lo, hi) = want.iter().fold((f64::MAX, f64::MIN), |(a, b), &v| (a.min(v), b.max(v)));
            assert_eq!(
                (sums.iter().map(|s| s.count).sum::<u64>(), sums.iter().map(|s| s.min).fold(f64::MAX, f64::min)),
                (ROWS as u64, lo),
                "{pass}: {}",
                t.name()
            );
            assert_eq!(sums.iter().map(|s| s.max).fold(f64::MIN, f64::max), hi);
            let tr = rec.trace(c, vp);
            assert_eq!(tr.kind, TraceKind::M4, "{pass}: {}", t.name());
            assert!(
                tr.y.iter().filter(|y| y.is_finite()).all(|&y| (lo as f32..=hi as f32).contains(&y)),
                "{pass}: {}",
                t.name()
            );
        }
    }
    set_plugins(vec![]);
}

// the registry is process-wide: one test at a time
static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn a_timed_plugin_column_of_every_type_reads_and_sums() {
    let _g = ONE.lock().unwrap_or_else(|e| e.into_inner());
    check(true);
}

#[test]
fn an_untimed_plugin_numbers_its_rows_a_second_apart() {
    let _g = ONE.lock().unwrap_or_else(|e| e.into_inner());
    check(false);
}
