//! A decoder plugin template. It decodes a small made-up format so it runs as it is: replace `parse_header`,
//! `SCHEMA` and `read_rows` with your format.
//!
//! The made-up format ("DEMO"): the bytes `DEMO`, a u32 record count, then records of
//! `u64 time (µs)`, `f32 volts`, `i16 status`, all little-endian.
//!
//! What the host gives you: `source::len()` and `source::read(offset, len)`, nothing else (no files, network or
//! clock). What it asks: `describe`, `sniff`, `start` (the schema), then `next` until it returns `None`. Columns are
//! little-endian arrays in each channel's type; times are nanoseconds and must not decrease.

wit_bindgen::generate!({ path: "wit", world: "decoder" });

use std::cell::RefCell;

use tlmbench::plugin::source;
use tlmbench::plugin::types::Dtype;

const MAGIC: &[u8] = b"DEMO";
const HEADER: u64 = 8;
const RECORD: u64 = 14;

struct State {
    rows: u64,
    next_row: u64,
}

thread_local! {
    // A plugin keeps its own position: one instance reads one file.
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// How many whole records a DEMO file of `len` bytes starting with `head` holds.
fn rows_in(head: &[u8], len: u64) -> Result<u64, String> {
    if head.len() < HEADER as usize || &head[..4] != MAGIC {
        return Err("not a DEMO file".into());
    }
    let declared = u32::from_le_bytes(head[4..8].try_into().unwrap()) as u64;
    // trust the file's length over its header: a file cut short still opens
    Ok(declared.min(len.saturating_sub(HEADER) / RECORD))
}

/// Whole records' bytes as a chunk: the times in ns and one column per channel.
fn decode(bytes: &[u8]) -> Chunk {
    let (mut time, mut volts, mut status) = (Vec::new(), Vec::new(), Vec::new());
    for r in bytes.chunks_exact(RECORD as usize) {
        let us = u64::from_le_bytes(r[0..8].try_into().unwrap());
        time.push(us as i64 * 1000);
        volts.extend_from_slice(&r[8..12]);
        status.extend_from_slice(&r[12..14]);
    }
    Chunk { rows: time.len() as u32, time, columns: vec![volts, status] }
}

struct Demo;

impl Guest for Demo {
    fn describe() -> About {
        About {
            name: "demo".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            description: Some("The template's made-up DEMO format".into()),
            // files named like this are sent here even when `sniff` says no
            files: vec!["*.demo".into()],
        }
    }

    fn sniff(head: Vec<u8>, _name: String) -> bool {
        head.starts_with(MAGIC)
    }

    fn start() -> Result<Schema, String> {
        let rows = rows_in(&source::read(0, HEADER as u32), source::len())?;
        STATE.with(|s| *s.borrow_mut() = Some(State { rows, next_row: 0 }));
        Ok(Schema {
            channels: vec![
                Channel { name: "volts".into(), unit: Some("V".into()), dtype: Dtype::F32 },
                Channel { name: "status".into(), unit: None, dtype: Dtype::I16 },
            ],
            timed: true,
        })
    }

    fn next(max_rows: u32) -> Result<Option<Chunk>, String> {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            let st = s.as_mut().ok_or("start was not called")?;
            let n = (st.rows - st.next_row).min(max_rows as u64);
            if n == 0 {
                return Ok(None);
            }
            let bytes = source::read(HEADER + st.next_row * RECORD, (n * RECORD) as u32);
            if bytes.len() < (n * RECORD) as usize {
                return Err("the file ended inside a record".into());
            }
            st.next_row += n;
            Ok(Some(decode(&bytes)))
        })
    }
}

export!(Demo);

// The test harness: `cargo test` runs your format code on this machine (no WASM needed; `source` is a stub here, so
// test the functions that take bytes). Check the built component the way the app runs it with
// `tlmbench plugins --check my_decoder.wasm FILE`.
#[cfg(test)]
mod tests {
    use super::*;

    fn demo(rows: u32) -> Vec<u8> {
        let mut b = MAGIC.to_vec();
        b.extend_from_slice(&rows.to_le_bytes());
        for i in 0..rows {
            b.extend_from_slice(&(i as u64 * 10).to_le_bytes());
            b.extend_from_slice(&(i as f32 / 2.0).to_le_bytes());
            b.extend_from_slice(&(i as i16 - 1).to_le_bytes());
        }
        b
    }

    #[test]
    fn a_file_cut_short_opens_with_its_whole_records() {
        let b = demo(5);
        assert_eq!(rows_in(&b, b.len() as u64).unwrap(), 5);
        assert_eq!(rows_in(&b, b.len() as u64 - 3).unwrap(), 4);
        assert!(rows_in(b"NOPE\0\0\0\0", 8).is_err());
        assert!(Demo::sniff(b.clone(), "x.bin".into()));
    }

    #[test]
    fn records_decode_to_times_and_columns() {
        let b = demo(3);
        let c = decode(&b[HEADER as usize..]);
        assert_eq!(c.rows, 3);
        assert_eq!(c.time, [0, 10_000, 20_000]);
        assert_eq!(f32::from_le_bytes(c.columns[0][8..12].try_into().unwrap()), 1.0);
        assert_eq!(i16::from_le_bytes(c.columns[1][0..2].try_into().unwrap()), -1);
    }
}
