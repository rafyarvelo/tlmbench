//! Synthetic recordings: sine + noise + spikes + gaps, the same for a given seed, as the reference fixed-width
//! format (.tlm) and as CSV. Recordings are rebuilt from the seed, never stored in the repo.
//!
//! Each channel is a sine (period 30 s × (channel + 1), amplitude 10) plus uniform noise of ±0.5, a ±25 spike
//! about every 200k samples, and a gap about every 5M samples: a run of NaN for 0.1 s in .tlm (fixed rate), and
//! 2 s with no rows in CSV (a time jump).

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, ValueEnum};
use tlmbench_core::datetime::parse_iso;
use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec};
use tlmbench_decode::reference;

#[derive(Parser, Debug)]
#[command(about = "Write deterministic synthetic recordings (sine + noise + spikes + gaps)")]
struct Args {
    /// Samples per channel: a number, or with a K, M or G suffix (100M)
    #[arg(long, value_parser = parse_count, default_value = "1M")]
    samples: u64,
    /// Size of the .tlm instead of --samples: samples per channel are this over 4 bytes × channels (10G, 20 channels:
    /// 125M samples each)
    #[arg(long, value_parser = parse_count, conflicts_with = "samples")]
    size: Option<u64>,
    #[arg(long, default_value_t = 1)]
    channels: usize,
    /// Sample rate in Hz
    #[arg(long, default_value_t = 10_000)]
    rate: u64,
    /// Formats to write
    #[arg(long, value_enum, value_delimiter = ',', default_value = "tlm,csv")]
    format: Vec<Format>,
    /// Output folder (default: $TLMBENCH_DATA, else ~/tlmbench-data)
    #[arg(long)]
    out: Option<PathBuf>,
    /// File name without extension (default: synth-<samples>-<channels>ch)
    #[arg(long)]
    name: Option<String>,
    #[arg(long, default_value_t = 1)]
    seed: u64,
    /// Time of the first sample (ISO 8601)
    #[arg(long, default_value = "2026-01-01T00:00:00Z")]
    start: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Format {
    Tlm,
    Csv,
}

fn parse_count(s: &str) -> Result<u64, String> {
    let s = s.trim();
    let (num, mult) = match s.chars().last().map(|c| c.to_ascii_uppercase()) {
        Some('K') => (&s[..s.len() - 1], 1_000),
        Some('M') => (&s[..s.len() - 1], 1_000_000),
        Some('G') => (&s[..s.len() - 1], 1_000_000_000),
        _ => (s, 1),
    };
    num.parse::<u64>().map(|n| n * mult).map_err(|e| format!("{s:?}: {e}"))
}

/// xorshift64*: small, fast and the same everywhere.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in [0, 1).
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// The signal of every channel, sample by sample.
struct Signal {
    rng: Rng,
    channels: usize,
    rate: f64,
    next_spike: u64,
    next_gap: u64,
}

const SPIKE_EVERY: f64 = 200_000.0;
const GAP_EVERY: f64 = 5_000_000.0;

impl Signal {
    fn new(seed: u64, channels: usize, rate: u64) -> Self {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let next_spike = (rng.unit() * SPIKE_EVERY * 2.0) as u64;
        let next_gap = (GAP_EVERY * (0.5 + rng.unit())) as u64;
        Self { rng, channels, rate: rate as f64, next_spike, next_gap }
    }

    /// Values of sample `i` into `out`; returns whether a gap starts here.
    fn sample(&mut self, i: u64, out: &mut [f32]) -> bool {
        let t = i as f64 / self.rate;
        let spike = i == self.next_spike;
        if spike {
            self.next_spike += 1 + (self.rng.unit() * SPIKE_EVERY * 2.0) as u64;
        }
        for (k, v) in out.iter_mut().enumerate().take(self.channels) {
            let period = 30.0 * (k + 1) as f64;
            let mut x = 10.0 * (std::f64::consts::TAU * t / period).sin() + self.rng.unit() - 0.5;
            if spike {
                x += if self.rng.next() & 1 == 0 { 25.0 } else { -25.0 };
            }
            *v = x as f32;
        }
        let gap = i == self.next_gap;
        if gap {
            self.next_gap += (GAP_EVERY * (0.5 + self.rng.unit())) as u64;
        }
        gap
    }
}

fn progress(label: &str, i: u64, n: u64, started: Instant) {
    if n >= 10 && i.is_multiple_of(n / 10) && i > 0 {
        eprint!("\r{label}: {}% ({:.0} s)", i * 100 / n, started.elapsed().as_secs_f64());
    }
}

fn write_tlm(path: &PathBuf, a: &Args, t0: i64) -> io::Result<()> {
    let desc = Descriptor {
        schema: SCHEMA,
        name: Some("tlmbench-gen synthetic".into()),
        endian: Endian::Little,
        header_size: 0,
        record_size: 4 * a.channels as u64,
        time: TimeSpec::Rate { rate_hz: a.rate as f64, t0_ns: t0 },
        fields: (0..a.channels)
            .map(|k| Field {
                name: format!("signal_{k}"),
                dtype: DType::F32,
                offset: 4 * k as u64,
                unit: Some("V".into()),
                scale: 1.0,
                value_offset: 0.0,
            })
            .collect(),
    };
    let mut w = BufWriter::with_capacity(8 << 20, File::create(path)?);
    reference::write_header(&mut w, &desc)?;
    let mut sig = Signal::new(a.seed, a.channels, a.rate);
    let mut vals = vec![0f32; a.channels];
    let gap_len = (a.rate / 10).max(1);
    let (mut gap_left, started) = (0u64, Instant::now());
    for i in 0..a.samples {
        if sig.sample(i, &mut vals) {
            gap_left = gap_len;
        }
        if gap_left > 0 {
            gap_left -= 1;
            vals.fill(f32::NAN);
        }
        for v in &vals {
            w.write_all(&v.to_le_bytes())?;
        }
        progress("tlm", i, a.samples, started);
    }
    w.flush()?;
    eprintln!();
    Ok(())
}

/// `-12.3456` with four decimals, without going through `format!` (100M rows).
fn push_fixed4(buf: &mut Vec<u8>, v: f32) {
    let scaled = (v as f64 * 1e4).round() as i64;
    if scaled < 0 {
        buf.push(b'-');
    }
    let (int, frac) = (scaled.unsigned_abs() / 10_000, scaled.unsigned_abs() % 10_000);
    push_uint(buf, int);
    buf.push(b'.');
    let f = frac as u32;
    buf.extend_from_slice(&[
        b'0' + (f / 1000) as u8,
        b'0' + (f / 100 % 10) as u8,
        b'0' + (f / 10 % 10) as u8,
        b'0' + (f % 10) as u8,
    ]);
}

fn push_uint(buf: &mut Vec<u8>, mut n: u64) {
    let mut tmp = [0u8; 20];
    let mut i = tmp.len();
    loop {
        i -= 1;
        tmp[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    buf.extend_from_slice(&tmp[i..]);
}

fn write_csv(path: &PathBuf, a: &Args, t0: i64) -> io::Result<()> {
    // fraction digits: as few as the sample period needs (10 kHz: 4)
    let period_num = 1_000_000_000u64;
    let exact = period_num.is_multiple_of(a.rate);
    let period_ns = period_num / a.rate;
    let digits = (0..=9u32)
        .find(|&d| exact && period_ns.is_multiple_of(10u64.pow(9 - d)) && t0 % 10i64.pow(9 - d) == 0)
        .unwrap_or(9);
    let mut w = BufWriter::with_capacity(8 << 20, File::create(path)?);
    let mut header = String::from("time_s");
    for k in 0..a.channels {
        header.push_str(&format!(",signal_{k} [V]"));
    }
    writeln!(w, "{header}")?;
    let mut sig = Signal::new(a.seed, a.channels, a.rate);
    let mut vals = vec![0f32; a.channels];
    let mut line = Vec::with_capacity(32 + 12 * a.channels);
    let gap_ns = 2_000_000_000i128;
    let (mut skip_until, started) = (i128::MIN, Instant::now());
    let mut written = 0u64;
    let mut i = 0u64;
    while written < a.samples {
        let t = t0 as i128 + i as i128 * period_num as i128 / a.rate as i128;
        if sig.sample(i, &mut vals) {
            skip_until = t + gap_ns;
        }
        i += 1;
        if t < skip_until {
            continue;
        }
        line.clear();
        let (secs, ns) = (t.div_euclid(1_000_000_000) as i64, t.rem_euclid(1_000_000_000) as u64);
        if secs < 0 {
            line.push(b'-');
        }
        push_uint(&mut line, secs.unsigned_abs());
        if digits > 0 {
            line.push(b'.');
            let frac = ns / 10u64.pow(9 - digits);
            let s = format!("{frac:0width$}", width = digits as usize);
            line.extend_from_slice(s.as_bytes());
        }
        for &v in &vals {
            line.push(b',');
            push_fixed4(&mut line, v);
        }
        line.push(b'\n');
        w.write_all(&line)?;
        progress("csv", written, a.samples, started);
        written += 1;
    }
    w.flush()?;
    eprintln!();
    Ok(())
}

fn main() -> io::Result<()> {
    let mut a = Args::parse();
    if let Some(size) = a.size {
        a.samples = size / (4 * a.channels.max(1) as u64);
    }
    if a.channels == 0 || a.rate == 0 {
        eprintln!("tlmbench-gen: --channels and --rate must be at least 1");
        std::process::exit(2);
    }
    let t0 = parse_iso(&a.start).unwrap_or_else(|| {
        eprintln!("tlmbench-gen: --start {:?} is not an ISO 8601 time", a.start);
        std::process::exit(2);
    });
    let out = a.out.clone().unwrap_or_else(|| {
        std::env::var_os("TLMBENCH_DATA")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::home_dir().unwrap_or_default().join("tlmbench-data"))
    });
    std::fs::create_dir_all(&out)?;
    let name = a.name.clone().unwrap_or_else(|| format!("synth-{}-{}ch", a.samples, a.channels));
    for f in &a.format {
        let (path, started) = match f {
            Format::Tlm => (out.join(format!("{name}.{}", reference::EXTENSION)), Instant::now()),
            Format::Csv => (out.join(format!("{name}.csv")), Instant::now()),
        };
        match f {
            Format::Tlm => write_tlm(&path, &a, t0)?,
            Format::Csv => write_csv(&path, &a, t0)?,
        }
        let size = std::fs::metadata(&path)?.len();
        println!("{} ({:.1} MB, {:.1} s)", path.display(), size as f64 / 1e6, started.elapsed().as_secs_f64());
    }
    Ok(())
}
