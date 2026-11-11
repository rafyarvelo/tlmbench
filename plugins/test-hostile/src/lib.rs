//! A decoder that misbehaves on purpose, to test the sandbox. The file's first bytes pick how:
//! `LOOP` spins forever, `MEMO` asks for 2 GB, `FILE` reads /etc/passwd with std, `TIME` reports the clock,
//! `BADC` returns a column of the wrong size, anything else decodes one good row.
wit_bindgen::generate!({ path: "wit", world: "decoder" });

use tlmbench::plugin::source;
use tlmbench::plugin::types::Dtype;

struct Hostile;

fn mode() -> Vec<u8> {
    source::read(0, 4)
}

impl Guest for Hostile {
    fn describe() -> About {
        About { name: "hostile".into(), version: "0.0.1".into(), description: None, files: vec![] }
    }
    fn sniff(head: Vec<u8>, _name: String) -> bool {
        head.len() >= 4 && head.iter().take(4).all(|b| b.is_ascii_uppercase())
    }
    fn start() -> Result<Schema, String> {
        Ok(Schema {
            channels: vec![Channel { name: "x".into(), unit: None, dtype: Dtype::F64 }],
            timed: false,
        })
    }
    fn next(_max_rows: u32) -> Result<Option<Chunk>, String> {
        thread_local!(static DONE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) });
        if DONE.with(|d| d.replace(true)) {
            return Ok(None);
        }
        let one = |v: f64| Chunk { rows: 1, time: vec![], columns: vec![v.to_le_bytes().to_vec()] };
        match mode().as_slice() {
            b"LOOP" => {
                let mut x = 0u64;
                loop {
                    x = std::hint::black_box(x.wrapping_add(1));
                }
            }
            b"MEMO" => {
                let big = vec![1u8; 2 << 30];
                Ok(Some(one(big[std::hint::black_box(7)] as f64)))
            }
            b"FILE" => match std::fs::read("/etc/passwd") {
                Ok(b) => Ok(Some(one(b.len() as f64))),
                Err(e) => Err(format!("no file access: {e}")),
            },
            b"TIME" => {
                let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                Ok(Some(one(t.as_secs() as f64)))
            }
            b"BADC" => Ok(Some(Chunk { rows: 2, time: vec![], columns: vec![vec![0u8; 8]] })),
            _ => Ok(Some(one(42.0))),
        }
    }
}

export!(Hostile);
