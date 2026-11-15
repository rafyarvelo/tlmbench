//! Browser-only mode's Parquet reader: a WASM module the page loads only when a Parquet file is opened, so the main core
//! stays small. It reads by the decoder's own rules (`parquet_core.rs`, included as source: which column is the time,
//! which are channels, units, nulls) and answers in the form the page's core loads (`crates/web`'s `dump`): a JSON
//! description, then the times (i64 LE, when the file has a time column) and each channel's values (f64 LE).
//!
//! The ABI is the main core's (`alloc`, `dealloc`, `call`, `reply_len`), so `js/wasmcore.ts`'s loader serves both. One
//! command, `parquet_dump`: the file's bytes in, the dump out. Codecs are pure Rust (snappy, LZ4, gzip, brotli, and
//! Zstandard through ruzstd); the file's bytes are all in memory, so a file larger than the page can hold fails with
//! the browser's own out-of-memory error.

#![allow(clippy::chunks_exact_to_as_chunks)]

#[path = "../../decode/src/backwards.rs"]
#[allow(dead_code)]
mod backwards;
#[path = "../../decode/src/csv.rs"]
#[allow(dead_code)]
mod csv;
#[path = "../../decode/src/descriptor.rs"]
#[allow(dead_code)]
mod descriptor;
#[path = "../../decode/src/error.rs"]
#[allow(dead_code)]
mod error;
#[path = "../../decode/src/parquet_core.rs"]
#[allow(dead_code)]
mod parquet_core;
#[path = "../../decode/src/profile.rs"]
#[allow(dead_code)]
mod profile;

#[allow(unused_imports)]
pub(crate) use error::Result;

use std::cell::RefCell;
use std::fmt::Display;

use bytes::Bytes;
use parquet::arrow::ProjectionMask;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use serde_json::json;

use crate::backwards::{Backwards, clamp_backwards};
use crate::descriptor::DType;
use crate::parquet_core::{Decoded, Plan, name_and_unit, plan, times, values};

thread_local! {
    static REPLY: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Memory for the page to write arguments into.
#[unsafe(no_mangle)]
pub extern "C" fn alloc(n: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(n.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// # Safety
/// `p` and `n` are an earlier `alloc(n)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dealloc(p: *mut u8, n: usize) {
    drop(unsafe { Vec::from_raw_parts(p, 0, n.max(1)) });
}

/// The length of the last reply.
#[unsafe(no_mangle)]
pub extern "C" fn reply_len() -> usize {
    REPLY.with(|r| r.borrow().len())
}

/// Runs a command (`parquet_dump`, with `{ "path": … }` and the file's bytes). Returns a pointer to the reply: a kind byte
/// (1 the dump, 2 an error message) and its bytes, `reply_len()` in all.
///
/// # Safety
/// Each pointer is an `alloc`ed buffer of the given length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn call(
    cmd: *const u8,
    cmd_len: usize,
    args: *const u8,
    args_len: usize,
    bin: *const u8,
    bin_len: usize,
) -> *const u8 {
    let (cmd, args, bin) = unsafe {
        (
            std::slice::from_raw_parts(cmd, cmd_len),
            std::slice::from_raw_parts(args, args_len),
            std::slice::from_raw_parts(bin, bin_len),
        )
    };
    let out = match run(cmd, args, bin) {
        Ok(dump) => [vec![1u8], dump].concat(),
        Err(e) => [vec![2u8], e.into_bytes()].concat(),
    };
    REPLY.with(|r| {
        *r.borrow_mut() = out;
        r.borrow().as_ptr()
    })
}

fn run(cmd: &[u8], args: &[u8], bin: &[u8]) -> Result<Vec<u8>, String> {
    if cmd != b"parquet_dump" {
        return Err(format!("{}: not a command of the Parquet module", String::from_utf8_lossy(cmd)));
    }
    let a: serde_json::Value = serde_json::from_slice(args).map_err(|e| e.to_string())?;
    let path = a["path"].as_str().unwrap_or("file.parquet");
    parquet_dump(path, bin)
}

/// A Parquet file's channels in the page's dump form. `path` names the file in messages.
pub fn parquet_dump(path: &str, file: &[u8]) -> Result<Vec<u8>, String> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let bad = |e: &dyn Display| format!("{name}: Parquet: {e}");
    if !file.starts_with(b"PAR1") {
        return Err(format!("{name}: not a Parquet file"));
    }
    let builder = ParquetRecordBatchReaderBuilder::try_new(Bytes::copy_from_slice(file)).map_err(|e| bad(&e))?;
    let schema = builder.schema().clone();
    let meta = builder.metadata().clone();
    let groups = meta.row_groups();
    let leaves = meta.file_metadata().schema_descr();
    // columns that may hold nulls (by the row groups' statistics; none recorded counts as maybe): an integer one widens
    let has_nulls = |i: usize| {
        let Some(leaf) = (0..leaves.num_columns()).find(|&l| leaves.get_column_root_idx(l) == i) else {
            return true;
        };
        schema.field(i).is_nullable()
            && groups.iter().any(|g| g.column(leaf).statistics().and_then(|s| s.null_count_opt()).is_none_or(|n| n > 0))
    };
    let Plan { time, channels, skipped } = plan(&schema, has_nulls);

    let mut roots: Vec<usize> = channels.iter().map(|c| c.0).collect();
    roots.extend(time.map(|t| t.index));
    roots.sort_unstable();
    let col_of = |i: usize| roots.binary_search(&i).expect("projected");
    let mask = ProjectionMask::roots(leaves, roots.iter().copied());
    let reader = builder.with_projection(mask).with_batch_size(1 << 16).build().map_err(|e| bad(&e))?;

    let mut t: Vec<i64> = Vec::new();
    let mut cols: Vec<Vec<f64>> = vec![Vec::new(); channels.len()];
    for b in reader {
        let b = b.map_err(|e| bad(&e))?;
        // a batch at a time, so the bytes the reader makes are never held for the whole file
        let mut out = Decoded::new(channels.len());
        let keep =
            time.map(|tm| times(b.column(col_of(tm.index)), tm.unit_ns, &mut out)).transpose().map_err(|e| bad(&e))?;
        for (k, &(i, r)) in channels.iter().enumerate() {
            values(b.column(col_of(i)), r, keep.as_ref(), &mut out.columns[k]);
            let dtype = r.dtype();
            cols[k].extend(to_f64(&out.columns[k], dtype));
        }
        t.extend(out.t);
    }
    let n = cols.first().map_or(t.len(), Vec::len);

    // time that goes backwards is flagged, not reordered (the rows keep file order, each taking the latest time so far)
    let mut backwards = Backwards::default();
    clamp_backwards(&mut t, &mut None, 1, &mut backwards);
    let time_meta = match time {
        Some(tm) => json!({ "kind": "column", "name": schema.field(tm.index).name() }),
        None => json!({ "kind": "rate", "t0": "0", "hz": 1.0 }),
    };
    let meta_json = json!({
        "samples": n,
        "time": time_meta,
        "channels": channels.iter().map(|&(i, _)| {
            let (name, unit) = name_and_unit(schema.field(i));
            json!({ "name": name, "unit": unit, "category": null })
        }).collect::<Vec<_>>(),
        "format": "Parquet",
        "skipped": skipped,
        "notes": backwards.note().into_iter().collect::<Vec<_>>(),
        "index": time.is_none(),
    });
    let meta_bytes = serde_json::to_vec(&meta_json).map_err(|e| e.to_string())?;
    let mut dump = Vec::with_capacity(8 + meta_bytes.len() + 8 * n * (cols.len() + usize::from(time.is_some())));
    dump.extend_from_slice(&(meta_bytes.len() as u64).to_le_bytes());
    dump.extend_from_slice(&meta_bytes);
    if time.is_some() {
        t.iter().for_each(|x| dump.extend_from_slice(&x.to_le_bytes()));
    }
    for c in &cols {
        c.iter().for_each(|x| dump.extend_from_slice(&x.to_le_bytes()));
    }
    Ok(dump)
}

/// A column's little-endian bytes of `dtype` as `f64`.
fn to_f64(bytes: &[u8], dtype: DType) -> Vec<f64> {
    macro_rules! conv {
        ($t:ty) => {
            bytes
                .chunks_exact(std::mem::size_of::<$t>())
                .map(|b| <$t>::from_le_bytes(b.try_into().unwrap()) as f64)
                .collect()
        };
    }
    match dtype {
        DType::I8 => conv!(i8),
        DType::I16 => conv!(i16),
        DType::I32 => conv!(i32),
        DType::I64 => conv!(i64),
        DType::U8 => conv!(u8),
        DType::U16 => conv!(u16),
        DType::U32 => conv!(u32),
        DType::U64 => conv!(u64),
        DType::F32 => conv!(f32),
        DType::F64 => conv!(f64),
    }
}
