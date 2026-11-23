//! A decoder that misbehaves on purpose, to test the sandbox. The file's first bytes pick how:
//! `LOOP` spins forever, `MEMO` asks for 2 GB, `FILE` reads /etc/passwd with std, `TIME` reports the clock,
//! `BADC` returns a column of the wrong size, `SOCK <port>` connects to 127.0.0.1:<port>, `ENVS` looks for an
//! environment, arguments or a folder, `DEEP` recurses without end, `ROWS` returns more rows than asked for, anything
//! else decodes one good row.
wit_bindgen::generate!({ path: "wit", world: "decoder" });

use tlmbench::plugin::source;
use tlmbench::plugin::types::Dtype;

struct Hostile;

fn mode() -> Vec<u8> {
    source::read(0, 4)
}

/// The number after the mode (`SOCK 4321`).
fn arg() -> u16 {
    let b = source::read(5, 8);
    std::str::from_utf8(&b).unwrap_or("").split(|c: char| !c.is_ascii_digit()).next().unwrap_or("").parse().unwrap_or(0)
}

fn deep(n: u64) -> u64 {
    // no tail call to fold away: the stack grows until the engine stops it
    std::hint::black_box(deep(std::hint::black_box(n + 1))) + 1
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
    fn next(max_rows: u32) -> Result<Option<Chunk>, String> {
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
            b"SOCK" => match std::net::TcpStream::connect(("127.0.0.1", arg())) {
                Ok(_) => Ok(Some(one(1.0))),
                Err(e) => Err(format!("no network: {e}")),
            },
            b"ENVS" => {
                let env = std::env::vars_os().count();
                let args = std::env::args_os().count();
                let dirs = ["/", "."].iter().filter(|d| std::fs::read_dir(d).is_ok()).count();
                if env + args + dirs > 0 {
                    Err(format!("leaked: {env} variables, {args} arguments, {dirs} folders"))
                } else {
                    Ok(Some(one(0.0)))
                }
            }
            b"DEEP" => Ok(Some(one(deep(0) as f64))),
            b"ROWS" => Ok(Some(Chunk {
                rows: max_rows + 1,
                time: vec![],
                columns: vec![vec![0u8; (max_rows as usize + 1) * 8]],
            })),
            b"BADC" => Ok(Some(Chunk { rows: 2, time: vec![], columns: vec![vec![0u8; 8]] })),
            _ => Ok(Some(one(42.0))),
        }
    }
}

export!(Hostile);
