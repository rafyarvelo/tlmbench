//! Live sources (design: Live streaming). A source reads frames from a socket, or follows a growing file, and records
//! them as they arrive.
//!
//! Each source runs two threads:
//!
//! - **ingest** reads the socket or file, cuts whole records (frames) by the descriptor's record size (or turns CSV
//!   rows into records, see [`tail`]), and pushes them into a single-producer ring buffer sized for about 10 s at the
//!   source's rate. It never blocks: records that don't fit are dropped and counted.
//! - **writer** drains the ring and appends the records to the source's `.tlm` file (the reference format, with the
//!   descriptor in its header), checks the descriptor's sequence field for lost or repeated frames, and publishes how
//!   many records are on disk. Readers of the growing file see that many.
//!
//! A source's recording comes in parts: one file, unless a tailed file is truncated or replaced (log rotation), when
//! the source continues in `source-N-2.tlm` and so on, its times as written. The ingest thread queues each part's
//! descriptor in order with the records, so the writer starts each file at the right record.
//!
//! A live session is a folder, `session-<start>/`, with a `source-N.tlm` per source and a `session.json`. Sessions go
//! to the app's data folder (see [`recordings_dir`]), never next to the data: they are kept, not cache.

pub mod replay;
pub mod tail;

use std::collections::VecDeque;
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
    /// Follow a growing file (CSV, or .tlm): what it holds so far, then each row or record appended.
    File(PathBuf),
    /// Read a serial port (`/dev/tty.usbserial…`, `COM3`) at a baud rate.
    Serial { port: String, baud: u32 },
    /// Replay a recording as if it were arriving now (see [`replay`]).
    Replay(replay::Replay),
}

impl FromStr for Endpoint {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        match s.split_once("://") {
            Some(("tcp", a)) if !a.is_empty() => Ok(Endpoint::Tcp(a.to_owned())),
            Some(("udp", a)) if !a.is_empty() => Ok(Endpoint::Udp(a.to_owned())),
            Some(("file", p)) if !p.is_empty() => Ok(Endpoint::File(PathBuf::from(p))),
            Some(("serial", rest)) if !rest.is_empty() => {
                let (port, query) = rest.split_once('?').unwrap_or((rest, ""));
                let baud = match query.strip_prefix("baud=") {
                    Some(b) => b.parse().map_err(|_| format!("{s:?}: baud={b} isn't a number"))?,
                    None if query.is_empty() => DEFAULT_BAUD,
                    None => return Err(format!("{s:?}: expected ?baud=<rate>")),
                };
                Ok(Endpoint::Serial { port: port.to_owned(), baud })
            }
            Some(("replay", rest)) if !rest.is_empty() => {
                let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
                let mut r = replay::Replay { path: PathBuf::from(path), speed_milli: 1000, start_ms: 0, looped: false };
                for kv in query.split('&').filter(|kv| !kv.is_empty()) {
                    let bad = || format!("{s:?}: couldn't read {kv:?}");
                    match kv.split_once('=') {
                        Some(("speed", v)) => {
                            let x: f64 = v.parse().map_err(|_| bad())?;
                            if !(x > 0.0 && x <= 1000.0) {
                                return Err(format!("{s:?}: speed is between 0 and 1000"));
                            }
                            r.speed_milli = (x * 1000.0).round() as u32;
                        }
                        Some(("start", v)) => r.start_ms = (v.parse::<f64>().map_err(|_| bad())? * 1000.0) as u64,
                        Some(("loop", v)) => r.looped = matches!(v, "1" | "true"),
                        _ => return Err(bad()),
                    }
                }
                Ok(Endpoint::Replay(r))
            }
            _ => Err(format!(
                "{s:?}: expected tcp://host:port, udp://host:port, serial://port?baud=<rate>, file:///path or replay:///path?speed=<x>"
            )),
        }
    }
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Endpoint::Tcp(a) => write!(f, "tcp://{a}"),
            Endpoint::Udp(a) => write!(f, "udp://{a}"),
            Endpoint::File(p) => write!(f, "file://{}", p.display()),
            Endpoint::Serial { port, baud } => write!(f, "serial://{port}?baud={baud}"),
            Endpoint::Replay(r) => write!(
                f,
                "replay://{}?speed={}&start={}&loop={}",
                r.path.display(),
                f64::from(r.speed_milli) / 1000.0,
                r.start_ms as f64 / 1000.0,
                u8::from(r.looped)
            ),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Options {
    /// Ring buffer size in bytes (default: about 10 s at the descriptor's rate, 16–256 MB; 64 MB without a rate).
    pub ring_bytes: Option<usize>,
    /// How long a TCP source keeps trying to connect.
    pub connect_timeout: Duration,
    /// A UDP source ends after this long without a datagram, once one has arrived, and a file source after this
    /// long without growing (None: only when stopped).
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
    /// File sources: rows whose time didn't parse, and rows earlier than the row before (both left out).
    pub rows_skipped: u64,
    pub rows_back_in_time: u64,
    /// The source's recording, part by part (more than one after a tailed file was rotated).
    pub files: Vec<PathBuf>,
    pub ring_bytes: u64,
    /// The most the ring buffer has held.
    pub ring_high_water_bytes: u64,
    /// The stream has ended (the sender closed it, it went quiet, or the source was stopped).
    pub ended: bool,
    pub error: Option<String>,
    pub paused: bool,
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
    rows_skipped: AtomicU64,
    rows_back: AtomicU64,
    ring_high: AtomicU64,
    ended: AtomicBool,
    /// Reading is paused (a replay's clock stops; a device's frames are let go by).
    paused: AtomicBool,
    error: Mutex<Option<String>>,
}

impl Counters {
    fn fail(&self, e: impl fmt::Display) {
        self.error.lock().unwrap().get_or_insert_with(|| e.to_string());
    }
}

/// A part of a source's recording: its file starts once `at` records have been pushed into the ring.
#[derive(Debug)]
struct Part {
    at: u64,
    descriptor: Descriptor,
}

/// What the ingest and writer threads share besides the ring.
#[derive(Debug, Default)]
struct Parts {
    queue: Mutex<VecDeque<Part>>,
    files: Mutex<Vec<PathBuf>>,
}

/// A running source: frames from `endpoint` recorded to `<base>.tlm` (and `<base>-2.tlm`… after a rotation).
#[derive(Debug)]
pub struct Source {
    endpoint: Endpoint,
    ring_bytes: usize,
    counters: Arc<Counters>,
    parts: Arc<Parts>,
    stop: Arc<AtomicBool>,
    started: Instant,
    ingest: Option<JoinHandle<()>>,
    writer: Option<JoinHandle<()>>,
}

impl Source {
    /// Starts reading `endpoint` into `<base>.tlm` (which must not exist). Sockets need the frames' `descriptor`; a
    /// file source takes its layout from the file (a .tlm's header, or a CSV's header and first rows).
    pub fn start(
        endpoint: Endpoint,
        descriptor: Option<&Descriptor>,
        base: &Path,
        opts: &Options,
    ) -> io::Result<Source> {
        let invalid = |m: String| io::Error::new(ErrorKind::InvalidInput, m);
        let first = match (&endpoint, descriptor) {
            (Endpoint::File(_) | Endpoint::Replay(_), _) => None,
            (_, Some(d)) => {
                d.validate().map_err(|e| invalid(e.to_string()))?;
                if d.packets.is_some() {
                    return Err(invalid(
                        "live sources don't read [packets] framing yet: frames are fixed-width records".into(),
                    ));
                }
                Some(d.clone())
            }
            (_, None) => return Err(invalid(format!("{endpoint} needs a descriptor for its frames"))),
        };
        let ring_bytes = opts.ring_bytes.unwrap_or_else(|| first.as_ref().map_or(64 << 20, default_ring));
        let (producer, consumer) = rtrb::RingBuffer::<u8>::new(ring_bytes);
        let counters = Arc::new(Counters::default());
        let parts = Arc::new(Parts::default());
        let stop = Arc::new(AtomicBool::new(false));
        // a socket's single part is known now: its file exists from the start
        let mut writer_state = Writer::new(base, consumer, parts.clone());
        if let Some(d) = &first {
            writer_state.open(d)?;
        }

        let ingest = {
            let (endpoint, counters, parts, stop, opts) =
                (endpoint.clone(), counters.clone(), parts.clone(), stop.clone(), opts.clone());
            thread::Builder::new().name(format!("ingest {endpoint}")).spawn(move || {
                let rs = first.as_ref().map_or(1, |d| d.record_size as usize);
                let mut ring = Ring { producer, rs, cap: ring_bytes, pushed: 0, counters: &counters, parts: &parts };
                if let Err(e) = match &endpoint {
                    Endpoint::Tcp(a) => ingest_tcp(a, &mut ring, &stop, &opts),
                    Endpoint::Udp(a) => ingest_udp(a, &mut ring, &stop, &opts),
                    Endpoint::File(p) => tail::follow(p, &mut ring, &stop, &opts),
                    Endpoint::Replay(r) => replay::run(r, &mut ring, &stop),
                    Endpoint::Serial { port, baud } => ingest_serial(port, *baud, &mut ring, &stop),
                } {
                    counters.fail(e);
                }
                counters.ended.store(true, Ordering::Release);
            })?
        };
        let writer = {
            let counters = counters.clone();
            thread::Builder::new().name(format!("record {}", base.display())).spawn(move || {
                if let Err(e) = writer_state.run(&counters) {
                    counters.fail(e);
                }
            })?
        };
        Ok(Source {
            endpoint,
            ring_bytes,
            counters,
            parts,
            stop,
            started: Instant::now(),
            ingest: Some(ingest),
            writer: Some(writer),
        })
    }

    /// The recording's files so far, part by part.
    pub fn files(&self) -> Vec<PathBuf> {
        self.parts.files.lock().unwrap().clone()
    }

    /// Records on disk so far, in all parts: readers of the files see at least this many.
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
            rows_skipped: get(&c.rows_skipped),
            rows_back_in_time: get(&c.rows_back),
            files: self.files(),
            ring_bytes: self.ring_bytes as u64,
            ring_high_water_bytes: get(&c.ring_high),
            ended: c.ended.load(Ordering::Acquire),
            error: c.error.lock().unwrap().clone(),
            paused: c.paused.load(Ordering::Relaxed),
            seconds: self.started.elapsed().as_secs_f64(),
        }
    }

    /// Pauses or resumes reading. A replay's clock stops, so it resumes without a gap; a device keeps sending, its
    /// frames in the meantime are let go, so resuming leaves a gap in the recording.
    pub fn set_paused(&self, paused: bool) {
        self.counters.paused.store(paused, Ordering::Release);
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
    /// Records pushed so far (dropped ones aren't).
    pushed: u64,
    counters: &'a Counters,
    parts: &'a Parts,
}

impl Ring<'_> {
    /// Pushes the whole records in `bytes` (a multiple of the record size); those that don't fit are dropped.
    fn push(&mut self, bytes: &[u8]) {
        // a device's frames are let go by while paused (a gap)
        if self.counters.paused.load(Ordering::Relaxed) {
            return;
        }
        self.push_always(bytes);
    }

    /// Pushes whatever the pause: a replay stops its own clock, so a batch it has already made is never thrown away
    /// (pausing between building a batch and pushing it would lose the batch).
    fn push_always(&mut self, bytes: &[u8]) {
        let n = bytes.len() / self.rs;
        let fit = n.min(self.producer.slots() / self.rs);
        if fit > 0 {
            let mut chunk = self.producer.write_chunk(fit * self.rs).expect("fits: checked against slots");
            let (a, b) = chunk.as_mut_slices();
            a.copy_from_slice(&bytes[..a.len()]);
            b.copy_from_slice(&bytes[a.len()..fit * self.rs]);
            chunk.commit_all();
        }
        self.pushed += fit as u64;
        let c = self.counters;
        c.records_in.fetch_add(n as u64, Ordering::Relaxed);
        c.dropped.fetch_add((n - fit) as u64, Ordering::Relaxed);
        c.ring_high.fetch_max((self.cap - self.producer.slots()) as u64, Ordering::Relaxed);
    }

    /// Starts the recording's next part: records pushed from now on go to a new file laid out by `d`.
    fn part(&mut self, d: Descriptor) {
        self.rs = d.record_size as usize;
        self.parts.queue.lock().unwrap().push_back(Part { at: self.pushed, descriptor: d });
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
    read_stream(&mut stream, ring, stop)
}

/// Reads a byte stream (a TCP connection, a serial port) into whole records until it ends or the source stops.
fn read_stream(stream: &mut dyn Read, ring: &mut Ring, stop: &AtomicBool) -> io::Result<()> {
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

/// A serial port's baud rate when the endpoint names none.
pub const DEFAULT_BAUD: u32 = 115_200;

fn ingest_serial(port: &str, baud: u32, ring: &mut Ring, stop: &AtomicBool) -> io::Result<()> {
    let mut serial = serialport::new(port, baud)
        .timeout(POLL)
        .open()
        .map_err(|e| io::Error::other(format!("couldn't open {port}: {e}")))?;
    read_stream(&mut *serial, ring, stop)
}

fn ingest_udp(addr: &str, ring: &mut Ring, stop: &AtomicBool, opts: &Options) -> io::Result<()> {
    // a large receive buffer rides out the moments the thread isn't reading (the default, 768 KB on macOS, holds
    // about 30 ms at 24 MB/s); the OS may cap it
    let at = resolve(addr)?;
    let sock =
        socket2::Socket::new(socket2::Domain::for_address(at), socket2::Type::DGRAM, Some(socket2::Protocol::UDP))?;
    for bytes in [8 << 20, 4 << 20, 2 << 20] {
        if sock.set_recv_buffer_size(bytes).is_ok() {
            break;
        }
    }
    sock.bind(&at.into())?;
    let socket: UdpSocket = sock.into();
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

/// The writer thread: drains the ring into the current part's file, starting each part's file at its record.
struct Writer {
    base: PathBuf,
    consumer: rtrb::Consumer<u8>,
    parts: Arc<Parts>,
    file: Option<File>,
    rs: usize,
    seq: Option<SeqCheck>,
    /// Records taken from the ring so far.
    taken: u64,
}

impl Writer {
    fn new(base: &Path, consumer: rtrb::Consumer<u8>, parts: Arc<Parts>) -> Writer {
        Writer { base: base.to_owned(), consumer, parts, file: None, rs: 1, seq: None, taken: 0 }
    }

    /// Creates the next part's file (`<base>.tlm`, then `<base>-2.tlm`…) and writes its header.
    fn open(&mut self, d: &Descriptor) -> io::Result<()> {
        if let Some(f) = self.file.take() {
            f.sync_data()?;
        }
        let mut files = self.parts.files.lock().unwrap();
        let name = self.base.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let name = match files.len() {
            0 => format!("{name}.{}", reference::EXTENSION),
            n => format!("{name}-{}.{}", n + 1, reference::EXTENSION),
        };
        let path = self.base.with_file_name(name);
        let mut file = OpenOptions::new().write(true).create_new(true).open(&path)?;
        reference::write_header(&mut file, d)?;
        files.push(path);
        (self.file, self.rs, self.seq) = (Some(file), d.record_size as usize, SeqCheck::new(d));
        Ok(())
    }

    /// Runs until the ingest thread has ended and every record and part is written.
    fn run(&mut self, counters: &Counters) -> io::Result<()> {
        let mut straddle = Vec::new();
        loop {
            let ended = counters.ended.load(Ordering::Acquire);
            // the next part starts here: open its file first
            let next = self.parts.queue.lock().unwrap().front().map(|p| p.at);
            if next == Some(self.taken) {
                let p = self.parts.queue.lock().unwrap().pop_front().expect("just seen");
                self.open(&p.descriptor)?;
                continue;
            }
            let mut take = self.consumer.slots() / self.rs * self.rs;
            if let Some(at) = next {
                take = take.min((at - self.taken) as usize * self.rs);
            }
            if take == 0 {
                if ended && next.is_none() && self.consumer.slots() < self.rs {
                    break;
                }
                thread::sleep(Duration::from_millis(1));
                continue;
            }
            let rs = self.rs;
            let file = self.file.as_mut().expect("records come after their part");
            let chunk = self.consumer.read_chunk(take).expect("available: checked against slots");
            let (a, b) = chunk.as_slices();
            file.write_all(a)?;
            file.write_all(b)?;
            if let Some(s) = &mut self.seq {
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
            self.taken += (take / rs) as u64;
            counters.written.fetch_add((take / rs) as u64, Ordering::Release);
        }
        match self.file.take() {
            Some(f) => f.sync_data(),
            None => Ok(()),
        }
    }
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
    /// The recording's files, part by part (`source-N.tlm`, then `source-N-2.tlm`… after a rotation).
    files: Vec<String>,
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

    /// How many sources have been added.
    pub fn sources(&self) -> usize {
        self.sources.len()
    }

    /// Starts recording `endpoint` into the session as its next `source-N` (see [`Source::start`]).
    pub fn add_source(
        &mut self,
        endpoint: Endpoint,
        descriptor: Option<&Descriptor>,
        opts: &Options,
    ) -> io::Result<Source> {
        let base = self.dir.join(format!("source-{}", self.sources.len() + 1));
        let source = Source::start(endpoint.clone(), descriptor, &base, opts)?;
        let name = descriptor.and_then(|d| d.name.clone());
        self.sources.push(SourceEntry { files: Vec::new(), endpoint: endpoint.to_string(), descriptor: name });
        self.update(self.sources.len() - 1, &source)?;
        Ok(source)
    }

    /// Notes source `index`'s files in `session.json` (call it when a part starts, or when the source stops).
    pub fn update(&mut self, index: usize, source: &Source) -> io::Result<()> {
        self.update_files(index, &source.files())
    }

    /// [`Session::update`] from the files a stopped source's [`Health`] lists.
    pub fn update_files(&mut self, index: usize, files: &[PathBuf]) -> io::Result<()> {
        let names = files.iter().filter_map(|f| f.file_name()).map(|n| n.to_string_lossy().into_owned());
        self.sources[index].files = names.collect();
        self.save()
    }

    fn save(&self) -> io::Result<()> {
        let json = serde_json::json!({ "started": self.started, "sources": self.sources });
        let tmp = self.dir.join("session.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&json)?)?;
        std::fs::rename(tmp, self.dir.join("session.json"))
    }
}

#[cfg(test)]
mod tests_support {
    use tlmbench_decode::descriptor::{Field, SCHEMA};

    use super::*;

    /// seq (u32) and one f32 channel, 1 kHz.
    pub(crate) fn desc() -> Descriptor {
        let field = |name: &str, dtype, offset| Field {
            name: name.into(),
            dtype,
            offset,
            unit: None,
            scale: 1.0,
            value_offset: 0.0,
            bits: None,
        };
        Descriptor {
            sequence: Some("seq".into()),
            files: Vec::new(),
            packet_types: Vec::new(),
            packets: None,
            schema: SCHEMA,
            name: Some("test".into()),
            endian: Endian::Little,
            header_size: 0,
            record_size: 8,
            time: TimeSpec::Rate { rate_hz: 1000.0, t0_ns: 0 },
            fields: vec![field("seq", DType::U32, 0), field("v", DType::F32, 4)],
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use super::*;

    use crate::tests_support::desc;

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
        let src = session.add_source(Endpoint::Tcp(addr.to_string()), Some(&desc()), &Options::default()).unwrap();
        let h = src.wait();
        let path = h.files[0].clone();
        sender.join().unwrap();
        assert_eq!((h.records_in, h.records_written, h.records_dropped), (100_000, 100_000, 0), "{h:?}");
        assert_eq!((h.sequence_missing, h.sequence_repeated, h.malformed_bytes, h.error.as_deref()), (0, 0, 0, None));
        assert!(h.ended && h.ring_high_water_bytes > 0 && h.ring_high_water_bytes <= h.ring_bytes);
        assert_eq!(body(&path), sent);
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(session.dir().join("session.json")).unwrap()).unwrap();
        assert_eq!(json["sources"][0]["files"][0], "source-1.tlm");
        assert!(session.dir().file_name().unwrap().to_str().unwrap().starts_with("session-20"));
    }

    #[test]
    fn udp_counts_lost_and_repeated_frames() {
        let dir = tempfile::tempdir().unwrap();
        let probe = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = probe.local_addr().unwrap();
        drop(probe);
        let opts = Options { idle_end: Some(Duration::from_millis(300)), ..Options::default() };
        let src = Source::start(Endpoint::Udp(addr.to_string()), Some(&desc()), &dir.path().join("u"), &opts).unwrap();
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

    /// A pseudo-terminal stands in for a serial device: what the far end writes is recorded exactly. (Linux only:
    /// on macOS the serial library sets every baud rate with an ioctl that pseudo-terminals refuse.)
    #[cfg(target_os = "linux")]
    #[test]
    fn a_serial_port_is_recorded() {
        use std::os::fd::FromRawFd;
        let (mut master, mut slave) = (0, 0);
        // SAFETY: openpty fills the two descriptors; the name buffer is unused (null)
        let r = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut() as _,
                std::ptr::null_mut() as _,
            )
        };
        assert_eq!(r, 0, "openpty");
        // SAFETY: slave is an open descriptor from openpty
        let name = unsafe { std::ffi::CStr::from_ptr(libc::ttyname(slave)) }.to_string_lossy().into_owned();
        let dir = tempfile::tempdir().unwrap();
        let ep: Endpoint = format!("serial://{name}?baud=115200").parse().unwrap();
        let src = Source::start(ep, Some(&desc()), &dir.path().join("s"), &Options::default()).unwrap();
        thread::sleep(Duration::from_millis(300)); // the port is open and raw before anything is sent
        assert!(src.health().error.is_none(), "{:?}", src.health().error);
        // SAFETY: master is an open descriptor from openpty, owned by this File from here on
        let mut far = unsafe { File::from_raw_fd(master) };
        let sent = records(0..20_000);
        let data = sent.clone();
        // the pty's buffer is small: the far end writes on a thread of its own while the source reads
        let writer = thread::spawn(move || {
            for part in data.chunks(777) {
                far.write_all(part).unwrap();
            }
            far
        });
        let start = Instant::now();
        while src.records_written() < 20_000 && start.elapsed() < Duration::from_secs(10) {
            thread::sleep(Duration::from_millis(20));
        }
        let h = src.stop();
        drop(writer.join().unwrap());
        // SAFETY: slave is still open here, and nothing else closes it
        unsafe { libc::close(slave) };
        assert_eq!(
            (h.records_written, h.sequence_missing, h.records_dropped, &h.error),
            (20_000, 0, 0, &None),
            "{h:?}"
        );
        assert_eq!(body(&h.files[0]), sent);
    }

    #[test]
    fn a_missing_serial_port_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let ep: Endpoint = "serial:///dev/tlmbench-no-such-port".parse().unwrap();
        let h = Source::start(ep, Some(&desc()), &dir.path().join("s"), &Options::default()).unwrap().wait();
        assert!(
            h.ended && h.error.as_deref().is_some_and(|e| e.contains("couldn't open /dev/tlmbench-no-such-port")),
            "{h:?}"
        );
    }

    #[test]
    fn a_full_ring_drops_whole_records() {
        let (counters, parts) = (Counters::default(), Parts::default());
        let (producer, mut consumer) = rtrb::RingBuffer::<u8>::new(8 * 10);
        let mut ring = Ring { producer, rs: 8, cap: 80, pushed: 0, counters: &counters, parts: &parts };
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
        assert_eq!("file:///data/run.csv".parse(), Ok(Endpoint::File("/data/run.csv".into())));
        assert_eq!(
            "serial:///dev/ttyUSB0".parse(),
            Ok(Endpoint::Serial { port: "/dev/ttyUSB0".into(), baud: DEFAULT_BAUD })
        );
        let com: Endpoint = "serial://COM3?baud=921600".parse().unwrap();
        assert_eq!(
            (com.to_string(), "serial://COM3?bod=1".parse::<Endpoint>().is_err()),
            ("serial://COM3?baud=921600".into(), true)
        );
        assert!("bluetooth://dev".parse::<Endpoint>().is_err());
        assert!(Source::start(Endpoint::Tcp("127.0.0.1:1".into()), None, Path::new("x"), &Options::default()).is_err());
    }
}
