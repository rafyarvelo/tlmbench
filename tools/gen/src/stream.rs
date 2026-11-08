//! The live sender for Gate 3, and the check of what was recorded.
//!
//! `--send tcp://host:port` listens there and streams to the first client; `--send udp://host:port` sends datagrams
//! to it. Each record is a u64 sequence number (from 0) and one f32 per channel, the same signal as the files
//! (without gaps), paced to `--rate` records a second for `--seconds`. `--descriptor-out` writes the descriptor a
//! receiver needs. `--append <file.csv>` writes the same signal as a growing CSV log instead (a time in
//! microseconds and four decimals a value, written in uneven chunks that cut lines). `--verify <file.tlm>` checks a
//! recording of either holds every record, in order, bit for bit.

use std::fs::File;
use std::io::{self, BufReader, Read, Write};
use std::net::{TcpListener, UdpSocket};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec};
use tlmbench_decode::reference;
use tlmbench_live::Endpoint;

use crate::{Signal, push_fixed4};

pub fn descriptor(channels: usize, rate: u64) -> Descriptor {
    let field = |name: String, dtype, offset, unit: Option<&str>| Field {
        name,
        dtype,
        offset,
        unit: unit.map(Into::into),
        scale: 1.0,
        value_offset: 0.0,
        bits: None,
    };
    let mut fields = vec![field("seq".into(), DType::U64, 0, None)];
    fields.extend((0..channels).map(|k| field(format!("signal_{k}"), DType::F32, 8 + 4 * k as u64, Some("V"))));
    Descriptor {
        sequence: Some("seq".into()),
        files: Vec::new(),
        packet_types: Vec::new(),
        packets: None,
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
        Endpoint::File(_) | Endpoint::Serial { .. } => {
            return Err(io::Error::other("--send takes tcp:// or udp://; --append writes a file"));
        }
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
        Endpoint::File(_) | Endpoint::Serial { .. } => unreachable!("refused above"),
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

/// Row `i` of an appended CSV: the time in seconds (microsecond digits), then four decimals a channel.
fn csv_row(sig: &mut Signal, i: u64, rate: u64, vals: &mut [f32], out: &mut Vec<u8>) {
    let us = i * 1_000_000 / rate;
    out.extend_from_slice(format!("{}.{:06}", us / 1_000_000, us % 1_000_000).as_bytes());
    sig.sample(i, vals);
    for v in vals.iter() {
        out.push(b',');
        push_fixed4(out, *v);
    }
    out.push(b'\n');
}

/// Appends `rate × seconds` rows to `path` (created with its header) as a logger would: paced, in uneven chunks.
/// With `rotate_at`, the file is truncated once after that many seconds and continues from row 0 again.
pub fn append(
    path: &Path,
    channels: usize,
    rate: u64,
    seconds: f64,
    seed: u64,
    rotate_at: Option<f64>,
) -> io::Result<()> {
    let header = || {
        let mut h = String::from("time_s");
        (0..channels).for_each(|k| h += &format!(",signal_{k} [V]"));
        h + "\n"
    };
    let open = |truncate: bool| {
        std::fs::OpenOptions::new().create(true).append(!truncate).write(true).truncate(truncate).open(path)
    };
    let mut f = open(true)?;
    f.write_all(header().as_bytes())?;
    let total = (rate as f64 * seconds).round() as u64;
    let (mut sig, mut vals, mut buf) = (Signal::new(seed, channels, rate), vec![0f32; channels], Vec::new());
    let (started, mut i, mut row, mut rotated, mut cut) = (Instant::now(), 0u64, 0u64, false, 0usize);
    let mut before = 0u64;
    let mut lens = [997usize, 61, 4093, 1, 257, 16_411, 65_521].iter().cycle();
    while i < total {
        let el = started.elapsed().as_secs_f64();
        if rotate_at.is_some_and(|r| el >= r) && !rotated {
            // finish what the old file was being written, then start it over
            f.write_all(&buf[cut..])?;
            (rotated, cut, before) = (true, 0, row);
            buf.clear();
            f = open(true)?;
            f.write_all(header().as_bytes())?;
            (sig, row) = (Signal::new(seed, channels, rate), 0);
        }
        let due = ((el * rate as f64) as u64).min(total);
        while i < due {
            csv_row(&mut sig, row, rate, &mut vals, &mut buf);
            (i, row) = (i + 1, row + 1);
        }
        // write what is due in uneven pieces, so lines are often cut: a reader must wait for their ends
        while cut < buf.len() {
            let n = (*lens.next().unwrap()).min(buf.len() - cut);
            f.write_all(&buf[cut..cut + n])?;
            cut += n;
        }
        (cut, _) = (0, buf.clear());
        thread::sleep(Duration::from_micros(500));
    }
    f.write_all(&buf[cut..])?;
    let secs = started.elapsed().as_secs_f64();
    let parts = if rotated { vec![before, row] } else { vec![row] };
    println!(
        "{}",
        serde_json::json!({ "rows": total, "parts": parts, "seconds": secs, "rows_per_s": total as f64 / secs })
    );
    Ok(())
}

/// Checks `path` holds records 0, 1, 2… of the sender's signal (`expect` of them, when given); prints JSON, and
/// fails when anything is missing, out of order or different.
pub fn verify(path: &Path, seed: u64, rate: u64, expect: Option<u64>) -> io::Result<bool> {
    let mut f = BufReader::with_capacity(8 << 20, File::open(path)?);
    let mut head = vec![0u8; 64 << 10];
    let n = f.get_mut().read(&mut head)?;
    let (start, d) = reference::read_header(&head[..n]).map_err(|e| io::Error::other(e.to_string()))?;
    let channels = d.fields.len() - 1;
    if let TimeSpec::Field { .. } = d.time {
        return verify_tailed(path, start, &d, seed, rate, expect);
    }
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

/// [`verify`] for a tailed --append file: each record is the row's time (ns) and its values as the CSV text reads.
fn verify_tailed(
    path: &Path,
    start: usize,
    d: &Descriptor,
    seed: u64,
    rate: u64,
    expect: Option<u64>,
) -> io::Result<bool> {
    let channels = d.fields.len() - 1;
    let (mut sig, mut vals, mut row) = (Signal::new(seed, channels, rate), vec![0f32; channels], Vec::new());
    let mut f = BufReader::with_capacity(8 << 20, File::open(path)?);
    io::copy(&mut (&mut f).take(start as u64), &mut io::sink())?;
    let mut rec = vec![0u8; d.record_size as usize];
    let (mut records, mut first_bad): (u64, Option<u64>) = (0, None);
    loop {
        match f.read_exact(&mut rec) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        }
        row.clear();
        csv_row(&mut sig, records, rate, &mut vals, &mut row);
        let text = std::str::from_utf8(&row[..row.len() - 1]).unwrap();
        let mut cols = text.split(',');
        let t = cols.next().unwrap().replace('.', "").parse::<i64>().unwrap() * 1000;
        let mut want = t.to_le_bytes().to_vec();
        cols.for_each(|c| want.extend_from_slice(&c.parse::<f64>().unwrap().to_le_bytes()));
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
