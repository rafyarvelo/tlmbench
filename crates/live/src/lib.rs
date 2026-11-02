//! Live sources (design: Live streaming). A source reads frames from a socket and records them as they arrive.
//!
//! Each source runs two threads:
//!
//! - **ingest** reads the socket, cuts whole records (frames) by the descriptor's record size, and pushes them into
//!   a single-producer ring buffer sized for about 10 s at the source's rate. It never blocks: records that don't fit
//!   are dropped and counted.
//! - **writer** drains the ring and appends the records to the source's `.tlm` file (the reference format, with the
//!   descriptor in its header), checks the descriptor's sequence field for lost or repeated frames, and publishes how
//!   many records are on disk. Readers of the growing file see that many.
//!
//! A live session is a folder, `session-<start>/`, with a `source-N.tlm` per source and a `session.json`. Sessions go
//! to the app's data folder (see [`recordings_dir`]), never next to the data: they are kept, not cache.

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs, UdpSocket};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tlmbench_core::datetime::format_utc;
use tlmbench_decode::descriptor::{DType, Descriptor, Endian, TimeSpec};
use tlmbench_decode::reference;

/// Where a source's frames come from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Endpoint {
    /// Connect to a device (or sender) listening at `host:port`.
    Tcp(String),
    /// Listen for datagrams on `host:port`.
    Udp(String),
}

impl FromStr for Endpoint {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        match s.split_once("://") {
            Some(("tcp", a)) if !a.is_empty() => Ok(Endpoint::Tcp(a.to_owned())),
            Some(("udp", a)) if !a.is_empty() => Ok(Endpoint::Udp(a.to_owned())),
            _ => Err(format!("{s:?}: expected tcp://host:port or udp://host:port")),
        }
    }
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Endpoint::Tcp(a) => write!(f, "tcp://{a}"),
            Endpoint::Udp(a) => write!(f, "udp://{a}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Options {
    /// Ring buffer size in bytes (default: about 10 s at the descriptor's rate, 16–256 MB; 64 MB without a rate).
    pub ring_bytes: Option<usize>,
    /// How long a TCP source keeps trying to connect.
    pub connect_timeout: Duration,
    /// A UDP source ends after this long without a datagram, once one has arrived (None: only when stopped).
    pub idle_end: Option<Duration>,
}

impl Default for Options {
    fn default() -> Self {
        Options { ring_bytes: None, connect_timeout: Duration::from_secs(10), idle_end: None }
    }
}

/// A source's stream health, as the Sources panel and the gate report show it.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Health {
    pub endpoint: String,
    pub bytes_in: u64,
    /// Whole records received.
    pub records_in: u64,
    /// Records that found the ring buffer full.
    pub records_dropped: u64,
    /// Records on disk.
    pub records_written: u64,
    /// Records missing by the sequence field, and records that came again or out of order.
    pub sequence_missing: u64,
    pub sequence_repeated: u64,
    /// Bytes of datagrams that weren't whole records.
    pub malformed_bytes: u64,
    pub ring_bytes: u64,
    /// The most the ring buffer has held.
    pub ring_high_water_bytes: u64,
    /// The stream has ended (the sender closed it, it went quiet, or the source was stopped).
    pub ended: bool,
    pub error: Option<String>,
    pub seconds: f64,
}

#[derive(Debug, Default)]
struct Counters {
    bytes_in: AtomicU64,
    records_in: AtomicU64,
    dropped: AtomicU64,
    written: AtomicU64,
    seq_missing: AtomicU64,
    seq_repeated: AtomicU64,
    malformed: AtomicU64,
    ring_high: AtomicU64,
    ended: AtomicBool,
    error: Mutex<Option<String>>,
}

impl Counters {
    fn fail(&self, e: impl fmt::Display) {
        self.error.lock().unwrap().get_or_insert_with(|| e.to_string());
    }
}

/// A running source: frames from `endpoint` recorded to `path`.
#[derive(Debug)]
pub struct Source {
    endpoint: Endpoint,
    path: PathBuf,
    ring_bytes: usize,
    counters: Arc<Counters>,
    stop: Arc<AtomicBool>,
    started: Instant,
    ingest: Option<JoinHandle<()>>,
    writer: Option<JoinHandle<()>>,
}

impl Source {
    /// Creates `path` (it must not exist), writes its header and starts reading `endpoint`.
    pub fn start(endpoint: Endpoint, descriptor: &Descriptor, path: &Path, opts: &Options) -> io::Result<Source> {
        descriptor.validate().map_err(|e| io::Error::new(ErrorKind::InvalidInput, e.to_string()))?;
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        reference::write_header(&mut file, descriptor)?;
        let rs = descriptor.record_size as usize;
        let ring_bytes = opts.ring_bytes.unwrap_or_else(|| default_ring(descriptor)).max(rs) / rs * rs;
        let (producer, consumer) = rtrb::RingBuffer::<u8>::new(ring_bytes);
        let counters = Arc::new(Counters::default());
        let stop = Arc::new(AtomicBool::new(false));
        let seq = SeqCheck::new(descriptor);

        let ingest = {
            let (endpoint, counters, stop, opts) = (endpoint.clone(), counters.clone(), stop.clone(), opts.clone());
            thread::Builder::new().name(format!("ingest {endpoint}")).spawn(move || {
                let mut ring = Ring { producer, rs, cap: ring_bytes, counters: &counters };
                if let Err(e) = match &endpoint {
                    Endpoint::Tcp(a) => ingest_tcp(a, &mut ring, &stop, &opts),
                    Endpoint::Udp(a) => ingest_udp(a, &mut ring, &stop, &opts),
                } {
                    counters.fail(e);
                }
                counters.ended.store(true, Ordering::Release);
            })?
        };
        let writer = {
            let counters = counters.clone();
            thread::Builder::new().name(format!("record {}", path.display())).spawn(move || {
                if let Err(e) = write_records(consumer, file, rs, seq, &counters) {
                    counters.fail(e);
                }
            })?
        };
        Ok(Source {
            endpoint,
            path: path.to_owned(),
            ring_bytes,
            counters,
            stop,
            started: Instant::now(),
            ingest: Some(ingest),
            writer: Some(writer),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Records on disk so far: a reader of the file sees at least this many.
    pub fn records_written(&self) -> u64 {
        self.counters.written.load(Ordering::Acquire)
    }

    pub fn health(&self) -> Health {
        let c = &self.counters;
        let get = |a: &AtomicU64| a.load(Ordering::Relaxed);
        Health {
            endpoint: self.endpoint.to_string(),
            bytes_in: get(&c.bytes_in),
            records_in: get(&c.records_in),
            records_dropped: get(&c.dropped),
            records_written: c.written.load(Ordering::Acquire),
            sequence_missing: get(&c.seq_missing),
            sequence_repeated: get(&c.seq_repeated),
            malformed_bytes: get(&c.malformed),
            ring_bytes: self.ring_bytes as u64,
            ring_high_water_bytes: get(&c.ring_high),
            ended: c.ended.load(Ordering::Acquire),
            error: c.error.lock().unwrap().clone(),
            seconds: self.started.elapsed().as_secs_f64(),
        }
    }

    /// Waits for the stream to end, then for every received record to be on disk.
    pub fn wait(mut self) -> Health {
        self.join();
        self.health()
    }

    /// Stops reading, records what was received, and returns the final health.
    pub fn stop(mut self) -> Health {
        self.stop.store(true, Ordering::Release);
        self.join();
        self.health()
    }

    fn join(&mut self) {
        for h in [self.ingest.take(), self.writer.take()].into_iter().flatten() {
            if h.join().is_err() {
                self.counters.fail("a source thread panicked");
            }
        }
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.join();
    }
}

/// About 10 s of the descriptor's rate, within 16–256 MB; 64 MB when the rate isn't known.
fn default_ring(d: &Descriptor) -> usize {
    const MB: usize = 1 << 20;
    match d.time {
        TimeSpec::Rate { rate_hz, .. } => ((rate_hz * d.record_size as f64 * 10.0) as usize).clamp(16 * MB, 256 * MB),
        _ => 64 * MB,
    }
}

/// The ring buffer's producing end, which takes whole records or drops them.
struct Ring<'a> {
    producer: rtrb::Producer<u8>,
    rs: usize,
    cap: usize,
    counters: &'a Counters,
}

impl Ring<'_> {
    /// Pushes the whole records in `bytes` (a multiple of the record size); those that don't fit are dropped.
    fn push(&mut self, bytes: &[u8]) {
        let n = bytes.len() / self.rs;
        let fit = n.min(self.producer.slots() / self.rs);
        if fit > 0 {
            let mut chunk = self.producer.write_chunk(fit * self.rs).expect("fits: checked against slots");
            let (a, b) = chunk.as_mut_slices();
            a.copy_from_slice(&bytes[..a.len()]);
            b.copy_from_slice(&bytes[a.len()..fit * self.rs]);
            chunk.commit_all();
        }
        let c = self.counters;
        c.records_in.fetch_add(n as u64, Ordering::Relaxed);
        c.dropped.fetch_add((n - fit) as u64, Ordering::Relaxed);
        c.ring_high.fetch_max((self.cap - self.producer.slots()) as u64, Ordering::Relaxed);
    }
}

const POLL: Duration = Duration::from_millis(100);
/// A TCP source reads at most this often while frames trickle in, and at once while they arrive faster.
const GATHER: Duration = Duration::from_millis(1);
const BATCH: usize = 256 << 10;

fn resolve(addr: &str) -> io::Result<std::net::SocketAddr> {
    addr.to_socket_addrs()?.next().ok_or_else(|| io::Error::new(ErrorKind::NotFound, format!("{addr}: no address")))
}

fn ingest_tcp(addr: &str, ring: &mut Ring, stop: &AtomicBool, opts: &Options) -> io::Result<()> {
    let to = resolve(addr)?;
    let deadline = Instant::now() + opts.connect_timeout;
    let mut stream = loop {
        match TcpStream::connect_timeout(&to, POLL) {
            Ok(s) => break s,
            Err(_) if Instant::now() < deadline && !stop.load(Ordering::Acquire) => thread::sleep(POLL),
            Err(e) => return Err(io::Error::new(e.kind(), format!("couldn't connect to {addr}: {e}"))),
        }
    };
    stream.set_read_timeout(Some(POLL))?;
    stream.set_nodelay(true)?;
    let mut buf = vec![0u8; (1 << 20).max(4 * ring.rs)];
    let mut have = 0;
    while !stop.load(Ordering::Acquire) {
        match stream.read(&mut buf[have..]) {
            Ok(0) => break,
            Ok(n) => {
                ring.counters.bytes_in.fetch_add(n as u64, Ordering::Relaxed);
                have += n;
                let whole = have / ring.rs * ring.rs;
                ring.push(&buf[..whole]);
                buf.copy_within(whole..have, 0);
                have -= whole;
                // a short read means the socket is drained: let a millisecond's frames gather rather than wake for
                // every segment (that cost a core at 24 MB/s); the socket buffer holds far more than that
                if n < BATCH {
                    thread::sleep(GATHER);
                }
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted) => {}
            Err(e) => return Err(e),
        }
    }
    ring.counters.malformed.fetch_add(have as u64, Ordering::Relaxed);
    Ok(())
}

fn ingest_udp(addr: &str, ring: &mut Ring, stop: &AtomicBool, opts: &Options) -> io::Result<()> {
    let socket = UdpSocket::bind(resolve(addr)?)?;
    socket.set_read_timeout(Some(POLL))?;
    let mut buf = vec![0u8; 65_536];
    let mut last: Option<Instant> = None;
    while !stop.load(Ordering::Acquire) {
        match socket.recv(&mut buf) {
            Ok(n) => {
                last = Some(Instant::now());
                ring.counters.bytes_in.fetch_add(n as u64, Ordering::Relaxed);
                let whole = n / ring.rs * ring.rs;
                ring.counters.malformed.fetch_add((n - whole) as u64, Ordering::Relaxed);
                ring.push(&buf[..whole]);
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted) => {
                if let (Some(t), Some(idle)) = (last, opts.idle_end)
                    && t.elapsed() >= idle
                {
                    break;
                }
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Drains the ring into the file until the ingest thread has ended and the ring is empty.
fn write_records(
    mut consumer: rtrb::Consumer<u8>,
    mut file: File,
    rs: usize,
    mut seq: Option<SeqCheck>,
    counters: &Counters,
) -> io::Result<()> {
    let mut straddle = Vec::with_capacity(rs);
    loop {
        let ended = counters.ended.load(Ordering::Acquire);
        let take = consumer.slots() / rs * rs;
        if take == 0 {
            if ended {
                break;
            }
            thread::sleep(Duration::from_millis(1));
            continue;
        }
        let chunk = consumer.read_chunk(take).expect("available: checked against slots");
        let (a, b) = chunk.as_slices();
        file.write_all(a)?;
        file.write_all(b)?;
        if let Some(s) = &mut seq {
            // a record may straddle the ring's end
            let head = a.len() / rs * rs;
            a[..head].chunks_exact(rs).for_each(|r| s.next(r, counters));
            let mut rest = b;
            if head < a.len() {
                let need = rs - (a.len() - head);
                straddle.clear();
                straddle.extend_from_slice(&a[head..]);
                straddle.extend_from_slice(&b[..need]);
                s.next(&straddle, counters);
                rest = &b[need..];
            }
            rest.chunks_exact(rs).for_each(|r| s.next(r, counters));
        }
        chunk.commit_all();
        counters.written.fetch_add((take / rs) as u64, Ordering::Release);
    }
    file.sync_data()
}

/// Follows a sequence field: each record should count up by one (wrapping at the field's width).
#[derive(Debug)]
struct SeqCheck {
    offset: usize,
    dtype: DType,
    big: bool,
    mask: u64,
    expect: Option<u64>,
}

impl SeqCheck {
    fn new(d: &Descriptor) -> Option<SeqCheck> {
        let f = d.fields.iter().find(|f| Some(&f.name) == d.sequence.as_ref())?;
        let bits = 8 * f.dtype.size();
        Some(SeqCheck {
            offset: f.offset as usize,
            dtype: f.dtype,
            big: d.endian == Endian::Big,
            mask: if bits == 64 { u64::MAX } else { (1 << bits) - 1 },
            expect: None,
        })
    }

    fn next(&mut self, record: &[u8], c: &Counters) {
        let n = self.dtype.size() as usize;
        let mut raw = [0u8; 8];
        if self.big {
            raw[8 - n..].copy_from_slice(&record[self.offset..self.offset + n]);
            raw.reverse();
        } else {
            raw[..n].copy_from_slice(&record[self.offset..self.offset + n]);
        }
        let v = u64::from_le_bytes(raw) & self.mask;
        if let Some(e) = self.expect {
            let jump = v.wrapping_sub(e) & self.mask;
            if jump > self.mask / 2 {
                c.seq_repeated.fetch_add(1, Ordering::Relaxed);
                return; // behind: keep waiting for the expected one
            }
            c.seq_missing.fetch_add(jump, Ordering::Relaxed);
        }
        self.expect = Some(v.wrapping_add(1) & self.mask);
    }
}

/// Where live sessions are recorded: `$TLMBENCH_RECORDINGS`, else the OS data folder's `TelemetryBench/Recordings`.
pub fn recordings_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("TLMBENCH_RECORDINGS").filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("TelemetryBench").join("Recordings")
}

/// A live session: a folder of sources recorded side by side on one time axis.
#[derive(Debug)]
pub struct Session {
    dir: PathBuf,
    started: String,
    sources: Vec<SourceEntry>,
}

#[derive(Clone, Debug, Serialize)]
struct SourceEntry {
    file: String,
    endpoint: String,
    descriptor: Option<String>,
}

impl Session {
    /// A new, empty session folder under `root` (created if missing), named by its start time.
    pub fn create(root: &Path) -> io::Result<Session> {
        std::fs::create_dir_all(root)?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        let started = format_utc(now.as_secs() as i64 * 1_000_000_000);
        let stem = format!("session-{}", started.trim_end_matches('Z').replace(':', "-"));
        let mut dir = root.join(&stem);
        for k in 2.. {
            match std::fs::create_dir(&dir) {
                Ok(()) => break,
                Err(e) if e.kind() == ErrorKind::AlreadyExists => dir = root.join(format!("{stem}-{k}")),
                Err(e) => return Err(e),
            }
        }
        let s = Session { dir, started, sources: Vec::new() };
        s.save()?;
        Ok(s)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Starts recording `endpoint` into the session as its next `source-N.tlm`.
    pub fn add_source(&mut self, endpoint: Endpoint, descriptor: &Descriptor, opts: &Options) -> io::Result<Source> {
        let file = format!("source-{}.{}", self.sources.len() + 1, reference::EXTENSION);
        let source = Source::start(endpoint.clone(), descriptor, &self.dir.join(&file), opts)?;
        self.sources.push(SourceEntry { file, endpoint: endpoint.to_string(), descriptor: descriptor.name.clone() });
        self.save()?;
        Ok(source)
    }

    fn save(&self) -> io::Result<()> {
        let json = serde_json::json!({ "started": self.started, "sources": self.sources });
        let tmp = self.dir.join("session.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&json)?)?;
        std::fs::rename(tmp, self.dir.join("session.json"))
    }
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use tlmbench_decode::descriptor::{Field, SCHEMA};

    use super::*;

    /// seq (u32) and one f32 channel, 1 kHz.
    fn desc() -> Descriptor {
        let field = |name: &str, dtype, offset| Field {
            name: name.into(),
            dtype,
            offset,
            unit: None,
            scale: 1.0,
            value_offset: 0.0,
        };
        Descriptor {
            sequence: Some("seq".into()),
            schema: SCHEMA,
            name: Some("test".into()),
            endian: Endian::Little,
            header_size: 0,
            record_size: 8,
            time: TimeSpec::Rate { rate_hz: 1000.0, t0_ns: 0 },
            fields: vec![field("seq", DType::U32, 0), field("v", DType::F32, 4)],
        }
    }

    fn records(seqs: impl IntoIterator<Item = u32>) -> Vec<u8> {
        seqs.into_iter().flat_map(|s| [s.to_le_bytes(), (s as f32 * 0.5).to_le_bytes()].concat()).collect()
    }

    fn body(path: &Path) -> Vec<u8> {
        let bytes = std::fs::read(path).unwrap();
        let (start, d) = reference::read_header(&bytes).unwrap();
        assert_eq!(d, desc());
        bytes[start..].to_vec()
    }

    #[test]
    fn tcp_records_everything_in_order() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let sent = records(0..100_000);
        let data = sent.clone();
        let sender = thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            // odd-sized writes: records arrive split across reads
            for part in data.chunks(1001) {
                s.write_all(part).unwrap();
            }
        });
        let dir = tempfile::tempdir().unwrap();
        let mut session = Session::create(dir.path()).unwrap();
        let src = session.add_source(Endpoint::Tcp(addr.to_string()), &desc(), &Options::default()).unwrap();
        let path = src.path().to_owned();
        let h = src.wait();
        sender.join().unwrap();
        assert_eq!((h.records_in, h.records_written, h.records_dropped), (100_000, 100_000, 0), "{h:?}");
        assert_eq!((h.sequence_missing, h.sequence_repeated, h.malformed_bytes, h.error.as_deref()), (0, 0, 0, None));
        assert!(h.ended && h.ring_high_water_bytes > 0 && h.ring_high_water_bytes <= h.ring_bytes);
        assert_eq!(body(&path), sent);
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(session.dir().join("session.json")).unwrap()).unwrap();
        assert_eq!(json["sources"][0]["file"], "source-1.tlm");
        assert!(session.dir().file_name().unwrap().to_str().unwrap().starts_with("session-20"));
    }

    #[test]
    fn udp_counts_lost_and_repeated_frames() {
        let dir = tempfile::tempdir().unwrap();
        let probe = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = probe.local_addr().unwrap();
        drop(probe);
        let opts = Options { idle_end: Some(Duration::from_millis(300)), ..Options::default() };
        let src = Source::start(Endpoint::Udp(addr.to_string()), &desc(), &dir.path().join("u.tlm"), &opts).unwrap();
        thread::sleep(Duration::from_millis(100));
        let out = UdpSocket::bind("127.0.0.1:0").unwrap();
        // 0..10, then 15..20 (5 lost), then 17 again, plus 3 stray bytes
        for d in [records(0..10), records(15..20), records([17]), vec![1, 2, 3]] {
            out.send_to(&d, addr).unwrap();
        }
        let h = src.wait();
        assert_eq!(
            (h.records_written, h.sequence_missing, h.sequence_repeated, h.malformed_bytes),
            (16, 5, 1, 3),
            "{h:?}"
        );
    }

    #[test]
    fn a_full_ring_drops_whole_records() {
        let counters = Counters::default();
        let (producer, mut consumer) = rtrb::RingBuffer::<u8>::new(8 * 10);
        let mut ring = Ring { producer, rs: 8, cap: 80, counters: &counters };
        ring.push(&records(0..6));
        ring.push(&records(6..12)); // 4 fit, 2 dropped
        assert_eq!(counters.records_in.load(Ordering::Relaxed), 12);
        assert_eq!(counters.dropped.load(Ordering::Relaxed), 2);
        assert_eq!(counters.ring_high.load(Ordering::Relaxed), 80);
        let got = consumer.read_chunk(80).unwrap();
        let (a, b) = got.as_slices();
        assert_eq!([a, b].concat(), records(0..10));
    }

    #[test]
    fn sequence_wraps_at_its_width() {
        let c = Counters::default();
        let mut d = desc();
        d.fields[0].dtype = DType::U16;
        let mut s = SeqCheck::new(&d).unwrap();
        for v in [65_534u32, 65_535, 0, 1, 3] {
            s.next(&records([v]), &c);
        }
        assert_eq!((c.seq_missing.load(Ordering::Relaxed), c.seq_repeated.load(Ordering::Relaxed)), (1, 0));
    }

    #[test]
    fn endpoints_parse() {
        assert_eq!("tcp://127.0.0.1:9000".parse(), Ok(Endpoint::Tcp("127.0.0.1:9000".into())));
        assert_eq!("udp://0.0.0.0:5000".parse::<Endpoint>().unwrap().to_string(), "udp://0.0.0.0:5000");
        assert!("serial:///dev/tty".parse::<Endpoint>().is_err());
    }
}
