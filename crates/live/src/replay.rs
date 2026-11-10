//! File playback: a recording replayed as if it were arriving now, for demos and for testing rules, checks and live
//! layouts without hardware. Each sample is due at its recorded time (from where playback starts) divided by the
//! speed; due samples become records (the time in session nanoseconds, then an f64 per channel) in the ring, so the
//! source takes the same path as a device: the writer, the growing file, the pyramid.
//!
//! Pausing a replay is a real pause: its clock stops and resumes without a gap.

use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec, TimeUnit};

use crate::Ring;

const TICK: Duration = Duration::from_millis(5);
/// At most this many samples per tick, so a fast replay of a large file yields to stop and pause.
const BATCH: usize = 8192;

/// How a replay runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replay {
    pub path: std::path::PathBuf,
    /// Speed in thousandths: 1000 is real time, 2000 twice as fast.
    pub speed_milli: u32,
    /// Where in the file playback starts, in milliseconds from its first sample.
    pub start_ms: u64,
    /// Starts again from the top at the end (else the source ends).
    pub looped: bool,
}

pub(crate) fn run(r: &Replay, ring: &mut Ring, stop: &AtomicBool) -> io::Result<()> {
    let opts = tlmbench_decode::OpenOptions::default();
    let rec = tlmbench_decode::open(&r.path, &opts).map_err(|e| io::Error::other(e.to_string()))?;
    let info = rec.info();
    let n = info.samples;
    let (Some(first), Some(last)) = (info.t_first, info.t_last) else {
        return Err(io::Error::other(format!("{}: no samples to replay", r.path.display())));
    };
    let nch = info.channels.len();
    ring.part(descriptor(&r.path, info));
    let speed = f64::from(r.speed_milli.max(1)) / 1000.0;
    let dt = if n > 1 { (last - first) as f64 / (n - 1) as f64 } else { 1e6 };
    let transforms: Vec<(f64, f64)> = (0..nch).map(|c| rec.scale_offset(c)).collect();

    let mut pos = rec.lower_bound(first.saturating_add((r.start_ms as i64).saturating_mul(1_000_000)));
    // the pass's start in the file's time, and the session time it began at
    let mut pass_start = rec.time_at(pos.min(n - 1));
    let mut played = 0.0f64;
    let begun = Instant::now();
    let mut paused_total = Duration::ZERO;
    let mut paused_since: Option<Instant> = None;
    let mut bytes = Vec::new();
    while !stop.load(Ordering::Acquire) {
        if ring.counters.paused.load(Ordering::Acquire) {
            paused_since.get_or_insert_with(Instant::now);
            thread::sleep(TICK);
            continue;
        }
        if let Some(p) = paused_since.take() {
            paused_total += p.elapsed();
        }
        let clock = (begun.elapsed() - paused_total).as_nanos() as f64;
        if pos >= n {
            if !r.looped {
                return Ok(());
            }
            // the next pass follows the last sample by one sample's time
            played += (last - pass_start) as f64 / speed + dt / speed;
            (pos, pass_start) = (0, first);
            continue;
        }
        let due = pass_start as f64 + (clock - played) * speed;
        let hi = if due >= last as f64 { n } else { rec.lower_bound(due as i64 + 1) }.min(pos + BATCH);
        if hi <= pos {
            thread::sleep(TICK);
            continue;
        }
        let times = rec.read_times(pos, hi);
        let cols: Vec<Vec<f64>> =
            (0..nch).map(|c| rec.read(c, pos, hi).to_f64(transforms[c].0, transforms[c].1)).collect();
        bytes.clear();
        for (i, t) in times.iter().enumerate() {
            let out = (played + (t - pass_start) as f64 / speed).round() as i64;
            bytes.extend_from_slice(&out.to_le_bytes());
            for c in &cols {
                bytes.extend_from_slice(&c[i].to_le_bytes());
            }
        }
        ring.counters.bytes_in.fetch_add(bytes.len() as u64, Ordering::Relaxed);
        ring.push(&bytes);
        pos = hi;
        if hi == n && !r.looped {
            return Ok(());
        }
    }
    Ok(())
}

/// The replay's layout: session time in nanoseconds, then each channel as f64.
fn descriptor(path: &Path, info: &tlmbench_core::RecordingInfo) -> Descriptor {
    let field = |name: &str, dtype, offset, unit: Option<String>| Field {
        name: name.to_owned(),
        dtype,
        offset,
        unit,
        scale: 1.0,
        value_offset: 0.0,
        bits: None,
    };
    let mut fields = vec![field("time", DType::I64, 0, None)];
    for (i, c) in info.channels.iter().enumerate() {
        fields.push(field(&c.name, DType::F64, 8 * (i as u64 + 1), c.unit.clone()));
    }
    Descriptor {
        sequence: None,
        files: Vec::new(),
        packet_types: Vec::new(),
        packets: None,
        schema: SCHEMA,
        name: Some(format!("replay of {}", path.file_name().unwrap_or_default().to_string_lossy())),
        endian: Endian::Little,
        header_size: 0,
        record_size: 8 * fields.len() as u64,
        time: TimeSpec::Field { field: "time".into(), unit: TimeUnit::Ns, t0_ns: 0 },
        fields,
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::{Endpoint, Options, Source};

    use super::*;

    /// A CSV of `n` rows at 1 kHz with a counter and a sine-ish value.
    fn csv(dir: &Path, n: usize) -> std::path::PathBuf {
        let p = dir.join("run.csv");
        let mut t = String::from("time_s,count,volts [V]\n");
        for i in 0..n {
            t.push_str(&format!("{}.{:03},{},{:.2}\n", i / 1000, i % 1000, i, i as f64 * 0.5));
        }
        std::fs::write(&p, t).unwrap();
        p
    }

    fn replay(path: &Path, query: &str) -> Endpoint {
        Endpoint::from_str(&format!("replay://{}?{query}", path.display())).unwrap()
    }

    #[test]
    fn the_endpoint_round_trips() {
        let e = Endpoint::from_str("replay:///data/a.csv?speed=2&start=210&loop=1").unwrap();
        let Endpoint::Replay(r) = &e else { panic!("{e:?}") };
        assert_eq!((r.speed_milli, r.start_ms, r.looped), (2000, 210_000, true));
        assert_eq!(e.to_string().parse::<Endpoint>().unwrap(), e);
        assert!(Endpoint::from_str("replay:///a.csv?speed=0").is_err());
        assert!(Endpoint::from_str("replay:///a.csv?pace=2").is_err());
    }

    #[test]
    fn a_replay_records_every_sample_at_the_speed_asked() {
        let dir = tempfile::tempdir().unwrap();
        let src = csv(dir.path(), 1000); // one second of data
        let begun = Instant::now();
        let h = Source::start(replay(&src, "speed=4"), None, &dir.path().join("source-1"), &Options::default())
            .unwrap()
            .wait();
        let took = begun.elapsed().as_secs_f64();
        assert_eq!(h.records_written, 1000, "{h:?}");
        assert_eq!(h.records_dropped, 0);
        assert!((0.2..0.6).contains(&took), "a second at 4x took {took} s");
        let rec = tlmbench_decode::open(&h.files[0], &Default::default()).unwrap();
        let info = rec.info();
        assert_eq!(info.samples, 1000);
        assert_eq!(info.channels.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["count", "volts"]);
        // session time: 250 ms for the second of data
        assert_eq!(info.t_first, Some(0));
        assert!((info.t_last.unwrap() - 249_750_000).abs() < 1000, "{:?}", info.t_last);
    }

    #[test]
    fn it_starts_where_asked_and_loops() {
        let dir = tempfile::tempdir().unwrap();
        let src = csv(dir.path(), 1000);
        let src_ep = replay(&src, "speed=10&start=0.5&loop=1");
        let s = Source::start(src_ep, None, &dir.path().join("source-1"), &Options::default()).unwrap();
        thread::sleep(Duration::from_millis(400));
        let h = s.stop();
        // half a second of data is 50 ms at 10x: the loop has gone round several times
        assert!(h.records_written > 1000, "{h:?}");
        let rec = tlmbench_decode::open(&h.files[0], &Default::default()).unwrap();
        assert_eq!(rec.read(0, 0, 1).to_f64(1.0, 0.0), [500.0], "starts at 0.5 s");
        let times = rec.read_times(0, rec.info().samples);
        assert!(times.windows(2).all(|w| w[1] > w[0]), "session time only goes forward");
    }

    #[test]
    fn pausing_stops_the_clock_without_a_gap() {
        let dir = tempfile::tempdir().unwrap();
        let src = csv(dir.path(), 2000); // two seconds
        let s =
            Source::start(replay(&src, "speed=2"), None, &dir.path().join("source-1"), &Options::default()).unwrap();
        thread::sleep(Duration::from_millis(300));
        s.set_paused(true);
        thread::sleep(Duration::from_millis(50));
        let at = s.records_written();
        thread::sleep(Duration::from_millis(400));
        assert_eq!(s.records_written(), at, "nothing arrives while paused");
        assert!(s.health().paused);
        s.set_paused(false);
        let h = s.wait();
        assert_eq!(h.records_written, 2000, "{h:?}");
        let rec = tlmbench_decode::open(&h.files[0], &Default::default()).unwrap();
        let times = rec.read_times(0, 2000);
        let biggest = times.windows(2).map(|w| w[1] - w[0]).max().unwrap();
        assert!(biggest < 5_000_000, "no gap where it paused: {biggest} ns");
    }
}
