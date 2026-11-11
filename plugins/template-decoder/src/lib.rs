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
        let head = source::read(0, HEADER as u32);
        if head.len() < HEADER as usize || &head[..4] != MAGIC {
            return Err("not a DEMO file".into());
        }
        let declared = u32::from_le_bytes(head[4..8].try_into().unwrap()) as u64;
        // trust the file's length over its header: a file cut short still opens
        let rows = declared.min((source::len().saturating_sub(HEADER)) / RECORD);
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
            let (mut time, mut volts, mut status) = (Vec::new(), Vec::new(), Vec::new());
            for r in bytes.chunks_exact(RECORD as usize) {
                let us = u64::from_le_bytes(r[0..8].try_into().unwrap());
                time.push(us as i64 * 1000);
                volts.extend_from_slice(&r[8..12]);
                status.extend_from_slice(&r[12..14]);
            }
            st.next_row += n;
            Ok(Some(Chunk { rows: n as u32, time, columns: vec![volts, status] }))
        })
    }
}

export!(Demo);
