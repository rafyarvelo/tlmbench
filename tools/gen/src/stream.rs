//! The live sender for Gate 3, and the check of what was recorded.
//!
//! `--send tcp://host:port` listens there and streams to the first client; `--send udp://host:port` sends datagrams
//! to it. Each record is a u64 sequence number (from 0) and one f32 per channel, the same signal as the files
//! (without gaps), paced to `--rate` records a second for `--seconds`. `--descriptor-out` writes the descriptor a
//! receiver needs. `--verify <file.tlm>` checks a recording holds every record, in order, bit for bit.

use std::fs::File;
use std::io::{self, BufReader, Read, Write};
use std::net::{TcpListener, UdpSocket};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec};
use tlmbench_decode::reference;
use tlmbench_live::Endpoint;

use crate::Signal;

pub fn descriptor(channels: usize, rate: u64) -> Descriptor {
    let field = |name: String, dtype, offset, unit: Option<&str>| Field {
        name,
        dtype,
        offset,
        unit: unit.map(Into::into),
        scale: 1.0,
        value_offset: 0.0,
    };
    let mut fields = vec![field("seq".into(), DType::U64, 0, None)];
    fields.extend((0..channels).map(|k| field(format!("signal_{k}"), DType::F32, 8 + 4 * k as u64, Some("V"))));
    Descriptor {
        sequence: Some("seq".into()),
        schema: SCHEMA,
        name: Some("tlmbench-gen live sender".into()),
        endian: Endian::Little,
        header_size: 0,
        record_size: 8 + 4 * channels as u64,
        time: TimeSpec::Rate { rate_hz: rate as f64, t0_ns: 0 },
        fields,
    }
}

/// Record `i` into `out` (8 + 4 × channels bytes).
fn record(sig: &mut Signal, i: u64, vals: &mut [f32], out: &mut Vec<u8>) {
    sig.sample(i, vals);
    out.extend_from_slice(&i.to_le_bytes());
    for v in vals.iter() {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

/// Streams `rate × seconds` records to `to`; prints a JSON report (how many, and whether it kept pace).
pub fn send(to: &Endpoint, channels: usize, rate: u64, seconds: f64, seed: u64) -> io::Result<()> {
    let total = (rate as f64 * seconds).round() as u64;
    let rs = 8 + 4 * channels;
    let mut sig = Signal::new(seed, channels, rate);
    let mut vals = vec![0f32; channels];
    // whole records per write: about 1 ms worth, at most 64 KB (8 KB datagrams for UDP)
    let max = match to {
        Endpoint::Tcp(_) => 65_536,
        Endpoint::Udp(_) => 8192,
    } / rs;
    let per_write = ((rate / 1000) as usize).clamp(1, max.max(1));
    enum Out {
        Tcp(std::net::TcpStream),
        Udp(UdpSocket, String),
    }
    let mut out = match to {
        Endpoint::Tcp(a) => {
            let l = TcpListener::bind(a)?;
            eprintln!("sender: waiting for a receiver on tcp://{}", l.local_addr()?);
            let (s, _) = l.accept()?;
            s.set_nodelay(true)?;
            Out::Tcp(s)
        }
        Endpoint::Udp(a) => Out::Udp(UdpSocket::bind("0.0.0.0:0")?, a.clone()),
    };
    let mut buf = Vec::with_capacity(per_write * rs);
    let (started, mut sent, mut behind_max) = (Instant::now(), 0u64, 0f64);
    while sent < total {
        let due = ((started.elapsed().as_secs_f64() * rate as f64) as u64).min(total);
        if due <= sent {
            thread::sleep(Duration::from_micros(200));
            continue;
        }
        behind_max = behind_max.max((due - sent) as f64 / rate as f64);
        while sent < due {
            let n = (due - sent).min(per_write as u64);
            buf.clear();
            for i in sent..sent + n {
                record(&mut sig, i, &mut vals, &mut buf);
            }
            match &mut out {
                Out::Tcp(s) => s.write_all(&buf)?,
                Out::Udp(s, a) => {
                    s.send_to(&buf, a.as_str())?;
                }
            }
            sent += n;
        }
    }
    let secs = started.elapsed().as_secs_f64();
    println!(
        "{}",
        serde_json::json!({
            "sent": sent,
            "seconds": secs,
            "records_per_s": sent as f64 / secs,
            "samples_per_s": (sent * channels as u64) as f64 / secs,
            // the most the sender fell behind its schedule: over 0.1 s, the run didn't hold the rate
            "behind_max_s": behind_max,
            "kept_pace": behind_max < 0.1,
        })
    );
    Ok(())
}

/// Checks `path` holds records 0, 1, 2… of the sender's signal (`expect` of them, when given); prints JSON, and
/// fails when anything is missing, out of order or different.
pub fn verify(path: &Path, seed: u64, expect: Option<u64>) -> io::Result<bool> {
    let mut f = BufReader::with_capacity(8 << 20, File::open(path)?);
    let mut head = vec![0u8; 64 << 10];
    let n = f.get_mut().read(&mut head)?;
    let (start, d) = reference::read_header(&head[..n]).map_err(|e| io::Error::other(e.to_string()))?;
    let channels = d.fields.len() - 1;
    let TimeSpec::Rate { rate_hz, .. } = d.time else { return Err(io::Error::other("not a sender recording")) };
    let mut sig = Signal::new(seed, channels, rate_hz as u64);
    let (rs, mut vals) = (8 + 4 * channels, vec![0f32; channels]);
    let mut f = BufReader::with_capacity(8 << 20, File::open(path)?);
    io::copy(&mut (&mut f).take(start as u64), &mut io::sink())?;
    let (mut rec, mut want) = (vec![0u8; rs], Vec::with_capacity(rs));
    let (mut records, mut first_bad): (u64, Option<u64>) = (0, None);
    loop {
        match f.read_exact(&mut rec) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        }
        want.clear();
        record(&mut sig, records, &mut vals, &mut want);
        if first_bad.is_none() && rec != want {
            first_bad = Some(records);
        }
        records += 1;
    }
    let ok = first_bad.is_none() && expect.is_none_or(|e| e == records);
    println!(
        "{}",
        serde_json::json!({ "records": records, "expected": expect, "first_mismatch": first_bad, "ok": ok })
    );
    Ok(ok)
}
