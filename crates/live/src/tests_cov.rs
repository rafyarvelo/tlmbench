//! Tests for the paths the first tests don't reach (coverage, R6c.3, Nov 26): refusals, errors, the writer's parts and
//! clock resets on a ring filled by hand, the time watch for every type, a paused source, a connection that comes
//! up late, UDP's idle end, and sessions. Each asserts what the source records or says, not that a line ran.

use std::collections::VecDeque;
use std::io::Write;
use std::net::{TcpListener, UdpSocket};

use tlmbench_decode::descriptor::{Bits, Field, TimeUnit};

use super::*;
use crate::tests_support::desc;

fn records(seqs: impl IntoIterator<Item = u32>) -> Vec<u8> {
    seqs.into_iter().flat_map(|s| [s.to_le_bytes(), (s as f32 * 0.5).to_le_bytes()].concat()).collect()
}

fn body(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap();
    let (start, _) = reference::read_header(&bytes).unwrap();
    bytes[start..].to_vec()
}

fn until(what: &str, mut f: impl FnMut() -> bool) {
    let t = Instant::now();
    while !f() {
        assert!(t.elapsed() < Duration::from_secs(10), "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(2));
    }
}

/// A port nothing listens on (and no datagram socket is bound to) for the moment.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

#[test]
fn endpoints_refuse_what_they_cannot_read() {
    let err = |s: &str| s.parse::<Endpoint>().unwrap_err();
    assert!(err("serial://COM3?baud=fast").contains("baud=fast isn't a number"));
    assert!(err("replay:///a.csv?speed=fast").contains("couldn't read"));
    assert!(err("replay:///a.csv?speed=1001").contains("speed is between 0 and 1000"));
    assert!(err("replay:///a.csv?start=soon").contains("couldn't read"));
    assert!(err("replay:///a.csv?speed").contains("couldn't read"));
    assert!(err("tcp://").contains("expected tcp://host:port"));

    let replay = |q: &str| match format!("replay:///a.csv?{q}").parse::<Endpoint>().unwrap() {
        Endpoint::Replay(r) => r,
        e => panic!("{e:?}"),
    };
    assert!(replay("loop=true").looped && replay("loop=1").looped);
    assert!(!replay("loop=0").looped && !replay("loop=yes").looped);
    assert_eq!(replay("start=1.5").start_ms, 1500);
    assert_eq!(replay("speed=0.25").speed_milli, 250);
    let shown = Endpoint::Replay(replay("speed=2&start=3&loop=0")).to_string();
    assert_eq!(shown, "replay:///a.csv?speed=2&start=3&loop=0");
    assert_eq!(Endpoint::File("/x/y.csv".into()).to_string(), "file:///x/y.csv");
}

const PLAIN: &str = r#"
schema = 1
name = "plain"
[packets]
sync = "EB90"
header_size = 6
length = { offset = 2, type = "u16" }
id = { offset = 4, type = "u8" }
checksum = "crc16-ccitt"

[[packet_types]]
id = 1
name = "imu"
record_size = 9
time = { source = "field", field = "t", unit = "us" }
fields = [
  { name = "t", type = "u64", offset = 6 },
  { name = "count", type = "u8", offset = 14 },
]
"#;

/// A plain packet of two imu records (`k` and `k + 1`).
fn imu_packet(d: &Descriptor, k: u64) -> Vec<u8> {
    let imu: Vec<u8> = (k..k + 2).flat_map(|i| [(i * 1000).to_le_bytes().to_vec(), vec![i as u8]].concat()).collect();
    tlmbench_decode::packets::encode_packet(d.packets.as_ref().unwrap(), false, &[0, 0, 0, 0, 1, 0], &imu).unwrap()
}

#[test]
fn a_source_that_cannot_start_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("s");
    let opts = Options::default();
    let start = |ep: Endpoint, d: Option<&Descriptor>| Source::start(ep, d, &base, &opts).unwrap_err();

    let mut bad = desc();
    bad.record_size = 4; // the f32 channel at 4..8 is outside the record
    let e = start(Endpoint::Tcp("127.0.0.1:1".into()), Some(&bad));
    assert_eq!(e.kind(), ErrorKind::InvalidInput);

    let packets = Descriptor::parse(PLAIN).unwrap();
    let e = start(Endpoint::Tcp("127.0.0.1:1".into()), Some(&packets));
    assert!(e.to_string().contains("start_packets"), "{e}");

    // the file of the source's one part exists already: nothing is overwritten
    std::fs::write(dir.path().join("s.tlm"), b"mine").unwrap();
    let e = start(Endpoint::Tcp("127.0.0.1:1".into()), Some(&desc()));
    assert_eq!(e.kind(), ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read(dir.path().join("s.tlm")).unwrap(), b"mine");
}

#[test]
fn a_packet_source_that_cannot_start_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let opts = Options::default();
    let tcp = || Endpoint::Tcp("127.0.0.1:1".into());
    let base = |k: usize| dir.path().join(format!("p{k}"));
    let start = |ep: Endpoint, d: &Descriptor| Source::start_packets(ep, d, &base, &opts).unwrap_err();

    let d = Descriptor::parse(PLAIN).unwrap();
    let e = start(Endpoint::File("/x.csv".into()), &d);
    assert!(e.to_string().contains("open a packet file as a file"), "{e}");
    let e = start(Endpoint::Replay(replay_of("/x.csv")), &d);
    assert_eq!(e.kind(), ErrorKind::InvalidInput);

    let mut bad = d.clone();
    bad.schema = 99;
    assert!(start(tcp(), &bad).to_string().contains("schema 99"));

    // a packet type whose plugin isn't loaded
    let plug = Descriptor::parse(&PLAIN.replace(
        "record_size = 9\ntime = { source = \"field\", field = \"t\", unit = \"us\" }\nfields = [\n  { name = \"t\", type = \"u64\", offset = 6 },\n  { name = \"count\", type = \"u8\", offset = 14 },\n]",
        "plugin = \"no-such-plugin\"",
    ))
    .unwrap();
    let e = start(tcp(), &plug);
    assert!(e.to_string().contains("isn't loaded"), "{e}");

    // the type's file exists already
    std::fs::write(dir.path().join("p0.tlm"), b"mine").unwrap();
    assert_eq!(start(tcp(), &d).kind(), ErrorKind::AlreadyExists);
}

fn replay_of(path: &str) -> replay::Replay {
    replay::Replay { path: path.into(), speed_milli: 1000, start_ms: 0, looped: false }
}

/// A packet stream on a serial port that isn't there: every type's source says so.
#[test]
fn a_packet_stream_on_a_missing_serial_port_fails_every_source() {
    let dir = tempfile::tempdir().unwrap();
    let d = Descriptor::parse(PLAIN).unwrap();
    let ep: Endpoint = "serial:///dev/tlmbench-no-such-port".parse().unwrap();
    let sources = Source::start_packets(ep, &d, &|k| dir.path().join(format!("p{k}")), &Options::default()).unwrap();
    assert_eq!(sources.len(), 1);
    for s in sources {
        let h = s.wait();
        assert!(h.ended && h.error.as_deref().is_some_and(|e| e.contains("couldn't open")), "{h:?}");
    }
}

/// A packet stream over UDP: each datagram a packet, and the stream ends after it has been quiet for `idle_end`.
#[test]
fn a_packet_stream_over_udp_ends_when_it_goes_quiet() {
    let dir = tempfile::tempdir().unwrap();
    let d = Descriptor::parse(PLAIN).unwrap();
    let port = free_port();
    let addr = format!("127.0.0.1:{port}");
    let opts = Options { idle_end: Some(Duration::from_millis(300)), ..Options::default() };
    let mut sources =
        Source::start_packets(Endpoint::Udp(addr.clone()), &d, &|k| dir.path().join(format!("p{k}")), &opts).unwrap();
    let src = sources.remove(0);
    let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
    // the source binds on its own thread: send until a datagram has arrived (the packets before it are lost)
    let mut sent = 0u64;
    while src.health().bytes_in == 0 {
        assert!(sent < 500, "no datagram arrived");
        sock.send_to(&imu_packet(&d, 0), &addr).unwrap();
        sent += 1;
        thread::sleep(Duration::from_millis(10));
    }
    let h = src.wait();
    assert!(h.ended && h.error.is_none(), "{h:?}");
    assert!(h.records_written >= 2 && h.records_written % 2 == 0 && h.records_written <= 2 * sent, "{h:?}");
    assert_eq!(h.malformed_bytes, 0, "{h:?}");
}

/// A packet stream over TCP that drops: with Reconnect the source connects again and records into the same files.
#[test]
fn a_packet_stream_reconnects() {
    let dir = tempfile::tempdir().unwrap();
    let d = Descriptor::parse(PLAIN).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let (first, second) = (imu_packet(&d, 0), imu_packet(&d, 10));
    let sender = thread::spawn(move || {
        for bytes in [first, second] {
            let (mut s, _) = listener.accept().unwrap();
            s.write_all(&bytes).unwrap();
        }
    });
    let opts = Options { reconnect: true, ..Options::default() };
    let mut sources =
        Source::start_packets(Endpoint::Tcp(addr), &d, &|k| dir.path().join(format!("p{k}")), &opts).unwrap();
    let src = sources.remove(0);
    until("both links' packets", || {
        let h = src.health();
        h.records_written == 4 && h.reconnects == 1
    });
    sender.join().unwrap();
    let h = src.stop();
    assert_eq!((h.records_written, h.reconnects, h.error.as_deref()), (4, 1, None), "{h:?}");
    assert_eq!(h.files.len(), 1);
    let rec = tlmbench_decode::fixed::FixedWidth::open_reference(&h.files[0]).unwrap();
    assert_eq!(tlmbench_core::Recording::info(&rec).samples, 4);
}

/// A device's frames are let go while paused (a gap in the recording), and recorded again after.
#[test]
fn a_paused_source_lets_the_frames_go_by() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let src = Source::start(Endpoint::Tcp(addr.to_string()), Some(&desc()), &dir.path().join("s"), &Options::default())
        .unwrap();
    let (mut s, _) = listener.accept().unwrap();
    s.write_all(&records(0..5)).unwrap();
    until("the first five", || src.records_written() == 5);
    src.set_paused(true);
    assert!(src.health().paused);
    s.write_all(&records(5..10)).unwrap();
    until("the paused five to be read", || src.health().bytes_in == 80);
    thread::sleep(Duration::from_millis(100)); // the reader has looked at the pause flag for those five
    src.set_paused(false);
    s.write_all(&records(10..15)).unwrap();
    drop(s);
    let h = src.wait();
    assert_eq!((h.records_in, h.records_written, h.records_dropped), (10, 10, 0), "{h:?}");
    assert_eq!((h.sequence_missing, h.bytes_in), (5, 120), "the five let go are a gap: {h:?}");
    assert_eq!(body(&h.files[0]), [records(0..5), records(10..15)].concat());
}

/// A TCP source started before its device listens keeps trying until `connect_timeout`.
#[test]
fn a_tcp_source_connects_when_the_device_comes_up_late() {
    let port = free_port();
    let addr = format!("127.0.0.1:{port}");
    let dir = tempfile::tempdir().unwrap();
    let opts = Options { connect_timeout: Duration::from_secs(10), ..Options::default() };
    let src = Source::start(Endpoint::Tcp(addr.clone()), Some(&desc()), &dir.path().join("s"), &opts).unwrap();
    thread::sleep(Duration::from_millis(350));
    assert!(src.health().error.is_none() && !src.health().ended, "still trying");
    let listener = TcpListener::bind(&addr).unwrap();
    let (mut s, _) = listener.accept().unwrap();
    s.write_all(&records(0..10)).unwrap();
    drop(s);
    let h = src.wait();
    assert_eq!((h.records_written, h.error.as_deref()), (10, None), "{h:?}");
}

#[test]
fn an_address_that_does_not_resolve_ends_the_source_with_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    for (k, ep) in [Endpoint::Tcp("no-port".into()), Endpoint::Udp("no-port".into())].into_iter().enumerate() {
        let h =
            Source::start(ep, Some(&desc()), &dir.path().join(format!("s{k}")), &Options::default()).unwrap().wait();
        assert!(h.ended && h.error.is_some() && h.records_written == 0, "{h:?}");
    }
}

#[test]
fn a_udp_port_in_use_ends_the_source_with_the_reason() {
    let taken = UdpSocket::bind("127.0.0.1:0").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let ep = Endpoint::Udp(taken.local_addr().unwrap().to_string());
    let h = Source::start(ep, Some(&desc()), &dir.path().join("s"), &Options::default()).unwrap().wait();
    assert!(h.ended && h.error.is_some(), "{h:?}");
}

/// A UDP source ends once datagrams have come and then stopped for `idle_end`.
#[test]
fn a_udp_source_ends_when_it_goes_quiet() {
    let port = free_port();
    let addr = format!("127.0.0.1:{port}");
    let dir = tempfile::tempdir().unwrap();
    let opts = Options { idle_end: Some(Duration::from_millis(250)), ..Options::default() };
    let src = Source::start(Endpoint::Udp(addr.clone()), Some(&desc()), &dir.path().join("s"), &opts).unwrap();
    let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
    let mut sent = 0u64;
    while src.health().bytes_in == 0 {
        assert!(sent < 500, "no datagram arrived");
        // 10 whole records and 3 bytes over
        let mut d = records(0..10);
        d.extend_from_slice(&[1, 2, 3]);
        sock.send_to(&d, &addr).unwrap();
        sent += 1;
        thread::sleep(Duration::from_millis(10));
    }
    let h = src.wait();
    assert!(h.ended && h.error.is_none(), "{h:?}");
    assert!(h.records_written >= 10 && h.records_written % 10 == 0, "{h:?}");
    assert_eq!(h.malformed_bytes, 3 * h.records_written / 10, "the 3 stray bytes of each datagram: {h:?}");
}

/// A reader scripted call by call.
struct Script(VecDeque<io::Result<Vec<u8>>>);

impl Read for Script {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.0.pop_front() {
            Some(Ok(v)) => {
                buf[..v.len()].copy_from_slice(&v);
                Ok(v.len())
            }
            Some(Err(e)) => Err(e),
            None => Ok(0),
        }
    }
}

fn kind(k: ErrorKind) -> io::Result<Vec<u8>> {
    Err(io::Error::from(k))
}

#[test]
fn a_stream_read_goes_on_past_timeouts_and_stops_at_a_real_error() {
    let (counters, parts) = (Counters::default(), Parts::default());
    let (producer, mut consumer) = rtrb::RingBuffer::<u8>::new(1 << 12);
    let mut ring = Ring { producer, rs: 8, cap: 1 << 12, pushed: 0, counters: &counters, parts: &parts };
    let stop = AtomicBool::new(false);
    let mut s = Script(VecDeque::from([
        kind(ErrorKind::WouldBlock),
        kind(ErrorKind::Interrupted),
        kind(ErrorKind::TimedOut),
        Ok(records(0..3)),
        kind(ErrorKind::ConnectionReset),
        Ok(records(3..4)),
    ]));
    let e = read_stream(&mut s, &mut ring, &stop).unwrap_err();
    assert_eq!(e.kind(), ErrorKind::ConnectionReset);
    assert_eq!((counters.records_in.load(Ordering::Relaxed), counters.bytes_in.load(Ordering::Relaxed)), (3, 24));
    let got = consumer.read_chunk(24).unwrap();
    assert_eq!(got.as_slices().0, records(0..3));
    got.commit_all();

    // a stream that ends part-way through a record counts the stray bytes
    let mut s = Script(VecDeque::from([Ok(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10])]));
    read_stream(&mut s, &mut ring, &stop).unwrap();
    assert_eq!(counters.malformed.load(Ordering::Relaxed), 2);
    assert_eq!(counters.records_in.load(Ordering::Relaxed), 4);
}

#[test]
fn a_packet_pump_goes_on_past_timeouts_and_stops_at_a_real_error() {
    let stop = AtomicBool::new(false);
    let mut s = Script(VecDeque::from([
        kind(ErrorKind::WouldBlock),
        Ok(vec![1, 2, 3]),
        kind(ErrorKind::Interrupted),
        kind(ErrorKind::TimedOut),
        Ok(vec![4]),
        kind(ErrorKind::BrokenPipe),
        Ok(vec![5]),
    ]));
    let mut got: Vec<Vec<u8>> = Vec::new();
    let e = pump(&mut s, &stop, &mut |b: &[u8]| got.push(b.to_vec())).unwrap_err();
    assert_eq!(e.kind(), ErrorKind::BrokenPipe);
    assert_eq!(got, [vec![1, 2, 3], vec![4]]);

    // a stop request ends it before another read
    stop.store(true, Ordering::Release);
    let mut s = Script(VecDeque::from([Ok(vec![9])]));
    pump(&mut s, &stop, &mut |_: &[u8]| panic!("read after stop")).unwrap();
}

/// A ring filled by hand, a writer run over it with the ingest side already ended: no timing involved.
struct Rig {
    dir: tempfile::TempDir,
    counters: Counters,
    parts: Arc<Parts>,
    producer: rtrb::Producer<u8>,
    writer: Writer,
}

impl Rig {
    fn new(d: &Descriptor, watch: bool) -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let parts = Arc::new(Parts::default());
        let (producer, consumer) = rtrb::RingBuffer::<u8>::new(1 << 16);
        let mut writer = Writer::new(&dir.path().join("w"), consumer, parts.clone());
        writer.watch = watch;
        writer.open(d).unwrap();
        Rig { dir, counters: Counters::default(), parts, producer, writer }
    }

    fn ring(&mut self, rs: usize) -> Ring<'_> {
        Ring {
            producer: std::mem::replace(&mut self.producer, rtrb::RingBuffer::new(1).0),
            rs,
            cap: 1 << 16,
            pushed: 0,
            counters: &self.counters,
            parts: &self.parts,
        }
    }
}

/// Parts queued at record counts the writer hasn't reached yet: each file starts at its record, with its own layout.
#[test]
fn queued_parts_start_their_files_at_their_records() {
    let d = desc();
    let (counters, parts) = (Counters::default(), Arc::new(Parts::default()));
    let (producer, consumer) = rtrb::RingBuffer::<u8>::new(1 << 16);
    let dir = tempfile::tempdir().unwrap();
    let mut writer = Writer::new(&dir.path().join("w"), consumer, parts.clone());
    writer.open(&d).unwrap();
    let mut ring = Ring { producer, rs: 8, cap: 1 << 16, pushed: 0, counters: &counters, parts: &parts };
    ring.push(&records(0..5));
    let mut narrow = desc();
    narrow.fields.pop();
    narrow.record_size = 4;
    ring.part(narrow);
    ring.push(&(10u32..14).flat_map(u32::to_le_bytes).collect::<Vec<u8>>());
    counters.ended.store(true, Ordering::Release);
    writer.run(&counters).unwrap();
    let files = parts.files.lock().unwrap().clone();
    assert_eq!(files.len(), 2);
    assert_eq!(body(&files[0]), records(0..5));
    assert_eq!(body(&files[1]), (10u32..14).flat_map(u32::to_le_bytes).collect::<Vec<u8>>());
    assert_eq!(counters.written.load(Ordering::Relaxed), 9);
    assert_eq!(counters.seq_missing.load(Ordering::Relaxed), 0);
}

fn ms_clock() -> Descriptor {
    let mut d = desc();
    d.sequence = None;
    d.fields[0].name = "t".into();
    d.time = TimeSpec::Field { field: "t".into(), unit: TimeUnit::Ms, t0_ns: 0 };
    d
}

/// A clock that goes back by more than a second, found part-way through a batch the writer takes at once: the
/// records before it end the part, the rest start the next.
#[test]
fn a_reset_inside_a_batch_splits_it() {
    let d = ms_clock();
    let mut rig = Rig::new(&d, true);
    let sent = [records(0..2000), records(1500..2500), records(0..1000)].concat();
    let mut ring = rig.ring(8);
    ring.push(&sent);
    rig.counters.ended.store(true, Ordering::Release);
    rig.writer.run(&rig.counters).unwrap();
    let files = rig.parts.files.lock().unwrap().clone();
    assert_eq!(files.len(), 2, "{files:?}");
    assert_eq!(body(&files[0]), [records(0..2000), records(1500..2500)].concat());
    assert_eq!(body(&files[1]), records(0..1000));
    assert_eq!(rig.counters.written.load(Ordering::Relaxed), 4000);
}

/// The writer can't start a part whose file exists: the source ends with the error and the earlier part is kept.
#[test]
fn a_part_that_cannot_be_opened_is_the_sources_error() {
    let d = ms_clock();
    let mut rig = Rig::new(&d, true);
    std::fs::write(rig.dir.path().join("w-2.tlm"), b"mine").unwrap();
    let mut ring = rig.ring(8);
    ring.push(&[records(0..2000), records(0..10)].concat());
    rig.counters.ended.store(true, Ordering::Release);
    let e = rig.writer.run(&rig.counters).unwrap_err();
    assert_eq!(e.kind(), ErrorKind::AlreadyExists);
    assert_eq!(rig.counters.written.load(Ordering::Relaxed), 2000);
    assert_eq!(std::fs::read(rig.dir.path().join("w-2.tlm")).unwrap(), b"mine");

    // the same from a queued part (a followed file that rotated)
    let mut rig = Rig::new(&d, false);
    std::fs::write(rig.dir.path().join("w-2.tlm"), b"mine").unwrap();
    let mut ring = rig.ring(8);
    ring.push(&records(0..3));
    ring.part(d.clone());
    rig.counters.ended.store(true, Ordering::Release);
    assert_eq!(rig.writer.run(&rig.counters).unwrap_err().kind(), ErrorKind::AlreadyExists);
    assert_eq!(rig.counters.written.load(Ordering::Relaxed), 3);
}

/// The same source, through a socket: the writer's error is in its health, and the stream still ends.
#[test]
fn a_source_whose_next_part_cannot_be_opened_reports_it() {
    let d = ms_clock();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("s-2.tlm"), b"mine").unwrap();
    let sender = thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        s.write_all(&[records(0..2000), records(0..10)].concat()).unwrap();
    });
    let src =
        Source::start(Endpoint::Tcp(addr.to_string()), Some(&d), &dir.path().join("s"), &Options::default()).unwrap();
    let h = src.wait();
    sender.join().unwrap();
    assert!(h.ended && h.error.as_deref().is_some_and(|e| e.contains("exist")), "{h:?}");
    assert_eq!(h.files.len(), 1);
}

fn time_desc(dtype: DType, big: bool, unit: TimeUnit) -> Descriptor {
    let mut d = desc();
    d.sequence = None;
    d.endian = if big { Endian::Big } else { Endian::Little };
    let mut f: Field = d.fields[0].clone();
    f.name = "t".into();
    f.dtype = dtype;
    f.offset = 0;
    d.record_size = dtype.size();
    d.fields = vec![f];
    d.time = TimeSpec::Field { field: "t".into(), unit, t0_ns: 0 };
    d
}

/// `values` encoded as `d`'s time field, one record each.
fn encode(d: &Descriptor, values: &[f64]) -> Vec<u8> {
    let dt = d.fields[0].dtype;
    let big = d.endian == Endian::Big;
    let mut out = Vec::new();
    for &v in values {
        let mut b = match dt {
            DType::F32 => (v as f32).to_le_bytes().to_vec(),
            DType::F64 => v.to_le_bytes().to_vec(),
            _ => (v as i64).to_le_bytes()[..dt.size() as usize].to_vec(),
        };
        if big {
            b.reverse();
        }
        out.extend(b);
    }
    out
}

/// The first reset in `values` as the writer finds it, for every way the ring could cut the bytes in two.
fn reset_at(d: &Descriptor, values: &[f64]) -> Option<usize> {
    let bytes = encode(d, values);
    let rs = d.record_size as usize;
    let mut found = None;
    for cut in 0..=bytes.len() {
        let mut w = TimeWatch::new(d).expect("a time field to watch");
        let r = w.first_reset(&bytes[..cut], &bytes[cut..], rs);
        assert!(found.is_none() || found == Some(r), "cut at {cut} gave {r:?}, other cuts {found:?}");
        found = Some(r);
    }
    found.unwrap()
}

#[test]
fn the_clock_watch_reads_every_kind_of_time_field() {
    // 1000 ms back is not a reset, more is; a step back is held (the latest time stays)
    for (dtype, big) in [
        (DType::F32, false),
        (DType::F32, true),
        (DType::F64, false),
        (DType::F64, true),
        (DType::U16, false),
        (DType::U32, true),
        (DType::I16, true),
        (DType::I32, false),
        (DType::I64, true),
        (DType::U64, false),
    ] {
        let d = time_desc(dtype, big, TimeUnit::Ms);
        assert_eq!(reset_at(&d, &[5000.0, 5000.0, 4500.0, 6000.0, 6000.0]), None, "{dtype:?} big={big}");
        assert_eq!(reset_at(&d, &[5000.0, 6000.0, 3000.0, 7000.0]), Some(2), "{dtype:?} big={big}");
        assert_eq!(
            reset_at(&d, &[5000.0, 6000.0, 5000.0, 6400.0, 5600.0, 0.0]),
            Some(5),
            "held at the latest: {dtype:?}"
        );
    }
    // signed: the sign is read
    let d = time_desc(DType::I16, false, TimeUnit::Ms);
    assert_eq!(reset_at(&d, &[1000.0, -1000.0, 2000.0]), Some(1));
    assert_eq!(reset_at(&d, &[-3000.0, -2500.0, -2000.0]), None);
    // the unit decides what a second is
    let d = time_desc(DType::U32, false, TimeUnit::Us);
    assert_eq!(reset_at(&d, &[5_000_000.0, 4_500_000.0, 6_000_000.0]), None);
    assert_eq!(reset_at(&d, &[5_000_000.0, 3_000_000.0]), Some(1));
}

#[test]
fn the_clock_watch_reads_the_bits_of_a_packed_time() {
    let mut d = time_desc(DType::U16, false, TimeUnit::Ms);
    d.fields[0].bits = Some(Bits { shift: 4, width: 12 });
    // the low 4 bits hold something else and are ignored: 0x0F, 0x1F... keep the time field's bits (value << 4)
    let packed = |ms: u16, low: u16| f64::from((ms << 4) | low);
    assert_eq!(reset_at(&d, &[packed(3000, 0xF), packed(3500, 0x0), packed(1000, 0x7)]), Some(2));
    assert_eq!(reset_at(&d, &[packed(3000, 0x0), packed(3500, 0xF), packed(2900, 0x1)]), None);
}

#[test]
fn a_counters_wrap_is_not_a_reset_but_a_big_step_back_is() {
    let mut d = time_desc(DType::U8, false, TimeUnit::Ns);
    d.time = TimeSpec::Counter { field: "t".into(), clock_hz: 100.0, wraps_at: None, t0_ns: 0 };
    // 250 -> 3: a drop of 247 of a range of 256 is a wrap
    assert_eq!(reset_at(&d, &[200.0, 250.0, 3.0, 40.0]), None);
    // 100 -> 20: a drop of 80 ticks is 0.8 s at 100 Hz: held
    assert_eq!(reset_at(&d, &[100.0, 20.0, 110.0]), None);
    // 126 -> 0 is 1.26 s at 100 Hz and under half of the range: a reset
    assert_eq!(reset_at(&d, &[10.0, 126.0, 0.0]), Some(2));
    // a range that is given
    let mut d = time_desc(DType::U16, false, TimeUnit::Ns);
    d.time = TimeSpec::Counter { field: "t".into(), clock_hz: 1000.0, wraps_at: Some(1000), t0_ns: 0 };
    assert_eq!(reset_at(&d, &[900.0, 990.0, 5.0, 100.0]), None, "wraps at 1000");
}

#[test]
fn some_times_cannot_be_watched() {
    let mut d = desc();
    d.time = TimeSpec::Index;
    assert!(TimeWatch::new(&d).is_none(), "rows are numbered");
    d.time = TimeSpec::Rate { rate_hz: 10.0, t0_ns: 0 };
    assert!(TimeWatch::new(&d).is_none(), "a rate has no field");
    let mut d = time_desc(DType::U16, false, TimeUnit::Ms);
    d.time = TimeSpec::Field { field: "gone".into(), unit: TimeUnit::Ms, t0_ns: 0 };
    assert!(TimeWatch::new(&d).is_none());
    d.time = TimeSpec::Counter { field: "gone".into(), clock_hz: 1.0, wraps_at: None, t0_ns: 0 };
    assert!(TimeWatch::new(&d).is_none());
    // a 64-bit counter has no range to wrap in
    let mut d = time_desc(DType::U64, false, TimeUnit::Ms);
    d.time = TimeSpec::Counter { field: "t".into(), clock_hz: 1.0, wraps_at: None, t0_ns: 0 };
    assert!(TimeWatch::new(&d).is_none());
}

#[test]
fn the_sequence_check_reads_big_endian_and_wide_fields() {
    let c = Counters::default();
    let mut d = desc();
    d.endian = Endian::Big;
    let mut s = SeqCheck::new(&d).unwrap();
    for v in [0u32, 1, 3, 3, 2, 4] {
        let r = [v.to_be_bytes(), 0f32.to_be_bytes()].concat();
        s.next(&r, &c);
    }
    // 2 was skipped once; 3 again and 2 came after the 3: both are behind
    assert_eq!((c.seq_missing.load(Ordering::Relaxed), c.seq_repeated.load(Ordering::Relaxed)), (1, 2));

    let c = Counters::default();
    let mut d = desc();
    d.fields[0].dtype = DType::U64;
    d.fields[0].size = 0;
    d.record_size = 12;
    d.fields[1].offset = 8;
    let mut s = SeqCheck::new(&d).unwrap();
    for v in [u64::MAX - 1, u64::MAX, 0, 1, 5] {
        s.next(&[v.to_le_bytes().to_vec(), vec![0; 4]].concat(), &c);
    }
    assert_eq!((c.seq_missing.load(Ordering::Relaxed), c.seq_repeated.load(Ordering::Relaxed)), (3, 0));
    d.sequence = Some("nope".into());
    assert!(SeqCheck::new(&d).is_none());
}

#[test]
fn a_session_makes_its_folder_and_keeps_the_sources_files() {
    let root = tempfile::tempdir().unwrap();
    // a root that is a file can't hold a session
    let file = root.path().join("a-file");
    std::fs::write(&file, b"x").unwrap();
    assert!(Session::create(&file).is_err());
    assert!(Session::create(&file.join("inside")).is_err());

    // folders exist already for this second and the next ones: the session takes the next free name
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    for t in now - 1..=now + 2 {
        let name = format_utc(t * 1_000_000_000);
        std::fs::create_dir(root.path().join(format!("session-{}", name.trim_end_matches('Z').replace(':', "-"))))
            .unwrap();
    }
    let mut session = Session::create(root.path()).unwrap();
    assert!(session.dir().file_name().unwrap().to_str().unwrap().ends_with("-2"), "{:?}", session.dir());
    assert_eq!(session.sources(), 0);

    // a source that can't start isn't counted
    assert!(session.add_source(Endpoint::Tcp("127.0.0.1:1".into()), None, &Options::default()).is_err());
    assert_eq!(session.sources(), 0);
    assert!(session.add_sources(Endpoint::Tcp("127.0.0.1:1".into()), None, &Options::default()).is_err());
    assert_eq!(session.sources(), 0);

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let src = session
        .add_source(
            Endpoint::Tcp(addr),
            Some(&desc()),
            &Options { connect_timeout: Duration::from_secs(1), ..Options::default() },
        )
        .unwrap();
    assert_eq!(session.sources(), 1);
    drop(listener);
    let h = src.stop();
    session.update_files(0, &h.files).unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(session.dir().join("session.json")).unwrap()).unwrap();
    assert_eq!(json["sources"][0]["files"], serde_json::json!(["source-1.tlm"]));
    assert_eq!(json["sources"][0]["descriptor"], "test");

    // the files of a stopped source can be swapped for others
    session.update_files(0, &[PathBuf::from("/x/source-1.tlm"), PathBuf::from("/x/source-1-2.tlm")]).unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(session.dir().join("session.json")).unwrap()).unwrap();
    assert_eq!(json["sources"][0]["files"], serde_json::json!(["source-1.tlm", "source-1-2.tlm"]));
}

/// The recordings folder is `$TLMBENCH_RECORDINGS` when it is set, else the OS data folder's. Setting a variable
/// would race with the other tests of this process, so it checks whichever the environment gives.
#[test]
fn the_recordings_folder_is_where_the_environment_says() {
    let dir = recordings_dir();
    match std::env::var_os("TLMBENCH_RECORDINGS").filter(|p| !p.is_empty()) {
        Some(p) => assert_eq!(dir, PathBuf::from(p)),
        None => assert!(dir.ends_with("TelemetryBench/Recordings"), "{dir:?}"),
    }
}

/// A packet type's clock that goes back by more than a second starts its next part, like a fixed-record source's;
/// when that part's file can't be made the type's source says so.
#[test]
fn a_packet_types_next_part_that_cannot_be_opened_is_its_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("p0-2.tlm"), b"mine").unwrap();
    let d = Descriptor::parse(PLAIN).unwrap();
    let p = d.packets.as_ref().unwrap();
    let at = |us: u64| {
        let rec = [us.to_le_bytes().to_vec(), vec![1u8]].concat();
        tlmbench_decode::packets::encode_packet(p, false, &[0, 0, 0, 0, 1, 0], &rec).unwrap()
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    let sent = [at(5_000_000), at(5_001_000), at(10)].concat();
    let sender = thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        s.write_all(&sent).unwrap();
    });
    let mut sources =
        Source::start_packets(Endpoint::Tcp(addr), &d, &|k| dir.path().join(format!("p{k}")), &Options::default())
            .unwrap();
    let h = sources.remove(0).wait();
    sender.join().unwrap();
    assert!(h.ended && h.error.as_deref().is_some_and(|e| e.contains("exist")), "{h:?}");
    assert_eq!(h.files.len(), 1);
    assert_eq!(std::fs::read(dir.path().join("p0-2.tlm")).unwrap(), b"mine");
}

/// A session starts a packet stream's sources under their type's names, or the descriptor's when a type has none; a
/// stream it can't start adds none.
#[test]
fn a_session_adds_a_packet_stream_source_by_source() {
    let root = tempfile::tempdir().unwrap();
    let mut session = Session::create(root.path()).unwrap();
    let tcp = || Endpoint::Tcp("127.0.0.1:1".into());
    let opts = Options { connect_timeout: Duration::from_millis(50), ..Options::default() };

    let mut missing = Descriptor::parse(PLAIN).unwrap();
    missing.packet_types[0].plugin = Some("no-such-plugin".into());
    assert!(session.add_sources(tcp(), Some(&missing), &opts).is_err());
    assert_eq!(session.sources(), 0);

    let one_kind = Descriptor::parse(
        r#"
schema = 1
name = "beacon"
record_size = 2
time = { source = "rate", rate_hz = 100.0 }
fields = [{ name = "v", type = "u16", offset = 4 }]
[packets]
sync = "EB90"
header_size = 4
length = { offset = 2, type = "u16" }
"#,
    )
    .unwrap();
    for d in [&one_kind, &Descriptor::parse(PLAIN).unwrap()] {
        let sources = session.add_sources(tcp(), Some(d), &opts).unwrap();
        assert_eq!(sources.len(), 1);
        for s in sources {
            assert!(s.stop().files.len() == 1);
        }
    }
    assert_eq!(session.sources(), 2);
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(session.dir().join("session.json")).unwrap()).unwrap();
    assert_eq!(json["sources"][0]["descriptor"], "beacon", "a stream of one kind has the descriptor's name");
    assert_eq!(json["sources"][1]["descriptor"], "imu");
    assert_eq!(json["sources"][1]["files"][0], "source-2.tlm");
}

/// A big read goes to the feed whole and without the pause that gathers small ones.
#[test]
fn a_big_read_is_fed_whole() {
    let stop = AtomicBool::new(false);
    let mut s = Script(VecDeque::from([Ok(vec![7u8; BATCH]), Ok(vec![8u8; 10])]));
    let mut got: Vec<usize> = Vec::new();
    pump(&mut s, &stop, &mut |b: &[u8]| got.push(b.len())).unwrap();
    assert_eq!(got, [BATCH, 10]);
}
