use tlmbench_core::chunks::Held;
use tlmbench_core::{Recording, TraceKind, Viewport};

use super::*;
use crate::descriptor::{Field, LengthField, TimeUnit};

/// 12-byte header: sync EB90, u16 payload length, u32 packet counter, 4 spare bytes. Records: u64 time (µs) and
/// an f32 and an i16 (scaled), 14 bytes.
fn desc(compression: Compression, checksum: Checksum) -> Descriptor {
    let field = |name: &str, dtype, offset| Field {
        name: name.into(),
        dtype,
        offset,
        unit: None,
        scale: 1.0,
        value_offset: 0.0,
    };
    let mut w = field("w", DType::I16, 24);
    w.scale = 0.5;
    Descriptor {
        schema: 1,
        name: Some("bench packets".into()),
        endian: Endian::Big,
        header_size: 0,
        record_size: 14,
        sequence: None,
        time: TimeSpec::Field { field: "t".into(), unit: TimeUnit::Us, t0_ns: 0 },
        fields: vec![field("count", DType::U32, 4), field("t", DType::U64, 12), field("v", DType::F32, 20), w],
        files: Vec::new(),
        packet_types: Vec::new(),
        packets: Some(Packets {
            sync: Some("EB90".into()),
            header_size: 12,
            id: None,
            length: Some(LengthField { offset: 2, dtype: DType::U16, counts: Counts::Payload, adjust: 0 }),
            checksum,
            compression,
        }),
    }
}

/// `packets` packets of `per` records each; record i has t = 1000·i µs, v = i / 4, w = i mod 100.
fn bytes(d: &Descriptor, packets: usize, per: usize) -> Vec<u8> {
    let p = d.packets.as_ref().unwrap();
    let mut out = Vec::new();
    for k in 0..packets {
        let mut header = vec![0u8; 12];
        header[4..8].copy_from_slice(&(k as u32).to_be_bytes());
        let mut payload = Vec::new();
        for j in 0..per {
            let i = (k * per + j) as u64;
            payload.extend_from_slice(&(i * 1000).to_be_bytes());
            payload.extend_from_slice(&(i as f32 / 4.0).to_be_bytes());
            payload.extend_from_slice(&((i % 100) as i16).to_be_bytes());
        }
        out.extend(encode_packet(p, true, &header, &payload).unwrap());
    }
    out
}

fn open(dir: &Path, name: &str, d: &Descriptor, data: &[u8]) -> PacketFile {
    let path = dir.join(name);
    fs::write(&path, data).unwrap();
    PacketFile::open(&path, d.clone(), None, "bench", &dir.join("cache")).unwrap()
}

#[test]
fn every_compression_reads_back() {
    let tmp = tempfile::tempdir().unwrap();
    for c in [Compression::None, Compression::Zstd, Compression::Lz4, Compression::Gzip] {
        let d = desc(c, Checksum::Crc16Ccitt);
        let f = open(tmp.path(), &format!("{c:?}.bin"), &d, &bytes(&d, 300, 500));
        let n = 150_000;
        assert_eq!((f.info().samples, f.bad_packets, f.skipped_bytes), (n, 0, 0), "{c:?}");
        let names: Vec<_> = f.info().channels.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["count", "v", "w"]);
        assert_eq!((f.info().t_first, f.info().t_last), (Some(0), Some((n as i64 - 1) * 1_000_000)));
        let h = Held::default();
        // header fields repeat for each record of a packet
        assert_eq!((f.value_at(0, 499, &h), f.value_at(0, 500, &h)), (0.0, 1.0), "{c:?}");
        assert_eq!(f.value_at(1, 123_457, &h), 123_457.0 / 4.0);
        assert_eq!(f.value_at(2, 123_457, &h), 57.0 * 0.5);
        assert_eq!(f.time_at(70_001), 70_001_000_000);
        assert_eq!(f.lower_bound(70_000_500_000), 70_001);
        // decoded chunks and summaries agree with reads one by one
        let chunks = Channels::decode_chunks(&f, &[1, 2], 1..3);
        assert_eq!(chunks[0][1].values::<f32>()[5], (2 * CHUNK_SAMPLES + 5) as f32 / 4.0);
        assert_eq!(chunks[1][0].values::<i16>()[7], ((CHUNK_SAMPLES + 7) % 100) as i16);
        let all = Channels::summarize_all(&f, 1024, 1024 + 4096);
        assert_eq!(all[1], f.summarize(1, 1024, 1024 + 4096));
        let tr = f.trace(1, Viewport { t0: 0, t1: n as i64 * 1_000_000, width_px: 200 });
        assert_eq!(tr.kind, TraceKind::M4);
        assert_eq!(f.read(1, 10, 13), tlmbench_core::Column::F32(vec![2.5, 2.75, 3.0]));
    }
}

#[test]
fn damaged_packets_are_skipped_and_counted() {
    let tmp = tempfile::tempdir().unwrap();
    let d = desc(Compression::Zstd, Checksum::Crc32);
    let mut data = b"junk before".to_vec();
    let good = bytes(&d, 10, 100);
    data.extend_from_slice(&good);
    // flip a byte inside the third packet's payload
    let starts: Vec<usize> = data.windows(2).enumerate().filter(|(_, w)| w == &[0xeb, 0x90]).map(|(i, _)| i).collect();
    data[starts[2] + 20] ^= 0xff;
    data.extend_from_slice(b"trailing");
    let f = open(tmp.path(), "damaged.bin", &d, &data);
    assert_eq!((f.info().samples, f.bad_packets), (900, 1));
    assert!(f.skipped_bytes >= 19, "{}", f.skipped_bytes);
    // record 200 is now the fourth packet's first
    assert_eq!(f.time_at(200), 300 * 1_000_000);
}

#[test]
fn without_a_sync_word_a_bad_length_ends_the_file() {
    let tmp = tempfile::tempdir().unwrap();
    let mut d = desc(Compression::None, Checksum::Sum8);
    let mut data = bytes(&d, 5, 10);
    d.packets.as_mut().unwrap().sync = None;
    let per = data.len() / 5;
    data[2 * per + 2] = 0xff; // the third packet's length now runs past the end
    let f = open(tmp.path(), "nosync.bin", &d, &data);
    assert_eq!((f.info().samples, f.bad_packets), (20, 0));
}

#[test]
fn the_index_is_built_once() {
    let tmp = tempfile::tempdir().unwrap();
    let d = desc(Compression::Lz4, Checksum::None);
    let data = bytes(&d, 20, 50);
    let a = open(tmp.path(), "once.bin", &d, &data);
    let dirs: Vec<_> = fs::read_dir(tmp.path().join("cache/packets")).unwrap().map(|e| e.unwrap().path()).collect();
    assert_eq!(dirs.len(), 1);
    let before = fs::metadata(dirs[0].join("index.bin")).unwrap().modified().unwrap();
    let b = PacketFile::open(&tmp.path().join("once.bin"), d, None, "bench", &tmp.path().join("cache")).unwrap();
    assert_eq!(a.info(), b.info());
    assert_eq!(fs::metadata(dirs[0].join("index.bin")).unwrap().modified().unwrap(), before);
}

#[test]
fn backwards_time_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    let d = desc(Compression::None, Checksum::None);
    let mut data = bytes(&d, 2, 3);
    let second = data.len() / 2;
    data.extend_from_within(..second); // packet 0 again, after packet 1
    let path = tmp.path().join("back.bin");
    fs::write(&path, &data).unwrap();
    let err = PacketFile::open(&path, d, None, "bench", &tmp.path().join("cache")).unwrap_err().to_string();
    assert!(err.contains("time goes backwards"), "{err}");
    assert!(
        !tmp.path().join("cache/packets").exists()
            || fs::read_dir(tmp.path().join("cache/packets")).unwrap().count() == 0
    );
}

/// The TOML the prototype's descriptor form writes for the ESC logger: fixed 24-byte frames (sync, CRC-16, no length).
const ESC: &str = r#"schema = 1
name = "ESC telemetry"
files = ["esc_bench_*.bin"]
endian = "little"
record_size = 20

[time]
source = "field"
field = "timestamp_us"
unit = "us"

[[fields]]
name = "seq"
type = "u16"
offset = 2

[[fields]]
name = "timestamp_us"
type = "u32"
offset = 4

[[fields]]
name = "battery_v"
type = "u16"
offset = 14
unit = "V"
scale = 0.001

[packets]
sync = "A55A"
header_size = 2
checksum = "crc16-ccitt"
"#;

#[test]
fn fixed_frames_without_a_length_field() {
    let tmp = tempfile::tempdir().unwrap();
    let d = Descriptor::parse(ESC).unwrap();
    assert_eq!(d.files, ["esc_bench_*.bin"]);
    let p = d.packets.as_ref().unwrap();
    let mut data = vec![0x00, 0x13]; // stray bytes before the first sync
    for k in 0..50u32 {
        let mut rec = vec![0u8; 20];
        rec[0..2].copy_from_slice(&(k as u16).to_le_bytes());
        rec[2..6].copy_from_slice(&(120_000_000 + k * 10_000).to_le_bytes());
        rec[12..14].copy_from_slice(&(24_000 + k as u16).to_le_bytes());
        // `encode_packet` puts the sync in the 2-byte header; the record follows it
        data.extend(encode_packet(p, false, &[0, 0], &rec).unwrap());
    }
    data[2 + 24 * 10 + 9] ^= 1; // a bit flip in frame 10: its CRC fails
    let f = open(tmp.path(), "esc_bench_0930.bin", &d, &data);
    assert_eq!((f.info().samples, f.bad_packets), (49, 1));
    let h = Held::default();
    assert_eq!(f.info().t_first, Some(120_000_000_000));
    assert_eq!((f.value_at(0, 9, &h), f.value_at(0, 10, &h)), (9.0, 11.0)); // frame 10 is gone
    assert!((f.value_at(1, 0, &h) - 24.0).abs() < 1e-9);
}

const TYPES: &str = r#"
    schema = 1
    name = "flight"
    endian = "big"
    [packets]
    sync = "EB90"
    header_size = 8
    length = { offset = 2, type = "u16" }
    id = { offset = 4, type = "u8" }
    checksum = "crc16-ccitt"
    compression = "zstd"
    [[packet_types]]
    id = 1
    name = "gps"
    record_size = 12
    time = { source = "field", field = "t", unit = "us" }
    fields = [ { name = "t", type = "u64", offset = 8 }, { name = "lat", type = "f32", offset = 16 } ]
    [[packet_types]]
    id = 2
    name = "imu"
    record_size = 14
    time = { source = "rate", rate_hz = 1000.0 }
    fields = [ { name = "ax", type = "i16", offset = 8 }, { name = "ay", type = "i16", offset = 10 } ]
"#;

/// Packets of both types alternating, then one with an id no type claims: gps record i (t = i s, lat = i),
/// imu record i (ax = i, ay = -i).
fn flight(d: &Descriptor, rounds: usize) -> Vec<u8> {
    let p = d.packets.as_ref().unwrap();
    let packet = |id: u8, payload: &[u8]| {
        let mut header = vec![0u8; 8];
        header[4] = id;
        encode_packet(p, true, &header, payload).unwrap()
    };
    let mut out = Vec::new();
    for r in 0..rounds {
        let gps: Vec<u8> = (0..10u64)
            .flat_map(|j| {
                let i = (r * 10) as u64 + j;
                [(i * 1_000_000).to_be_bytes().to_vec(), (i as f32).to_be_bytes().to_vec()].concat()
            })
            .collect();
        let imu: Vec<u8> = (0..20i16)
            .flat_map(|j| {
                let i = r as i16 * 20 + j;
                [0u8; 0].into_iter().chain(i.to_be_bytes()).chain((-i).to_be_bytes()).chain([0; 10])
            })
            .collect();
        out.extend(packet(1, &gps));
        out.extend(packet(2, &imu));
        if r == 1 {
            out.extend(packet(9, &[1, 2, 3]));
        }
    }
    out
}

#[test]
fn packet_types_open_one_recording_each() {
    let tmp = tempfile::tempdir().unwrap();
    let d = Descriptor::parse(TYPES).unwrap();
    let data = flight(&d, 5);
    let (path, toml) = (tmp.path().join("flight.bin"), tmp.path().join("flight.toml"));
    fs::write(&path, &data).unwrap();
    fs::write(&toml, TYPES).unwrap();
    let opts = crate::OpenOptions { descriptor: Some(toml.clone()), part: None, cache_dir: Some(tmp.path().join("c")) };
    let all = crate::open_all(&path, &opts).unwrap();
    let infos: Vec<_> = all.iter().map(|r| (r.info().part.clone().unwrap(), r.info().samples)).collect();
    assert_eq!(infos, [("gps".to_owned(), 50), ("imu".to_owned(), 100)]);
    let h = Held::default();
    assert_eq!(all[0].value_at(0, 33, &h), 33.0);
    assert_eq!(all[0].time_at(33), 33_000_000_000);
    assert_eq!((all[1].value_at(0, 77, &h), all[1].value_at(1, 77, &h)), (77.0, -77.0));
    assert_eq!(all[1].info().channels.len(), 2);
    // one type by name; without one, a file with several refuses to open
    let imu = crate::open(&path, &crate::OpenOptions { part: Some("imu".into()), ..opts.clone() }).unwrap();
    assert_eq!(imu.info().samples, 100);
    let err = crate::open(&path, &opts).err().unwrap().to_string();
    assert!(err.contains("2 packet types (gps, imu)"), "{err}");
    let err = crate::open(&path, &crate::OpenOptions { part: Some("nope".into()), ..opts }).err().unwrap().to_string();
    assert!(err.contains("no packet type \"nope\""), "{err}");
}

#[test]
fn packet_types_split_the_skipped_counts() {
    let tmp = tempfile::tempdir().unwrap();
    let d = Descriptor::parse(TYPES).unwrap();
    let mut data = flight(&d, 3);
    data.extend_from_slice(b"tail");
    let path = tmp.path().join("flight.bin");
    fs::write(&path, &data).unwrap();
    let parts = d.parts();
    let open = |k: usize| {
        let (pd, part) = parts[k].clone();
        PacketFile::open(&path, pd, part, "flight", &tmp.path().join("cache")).unwrap()
    };
    let (gps, imu) = (open(0), open(1));
    // the unknown-id packet is bad once (the first type counts it); other types' bytes are not skipped bytes
    assert_eq!((gps.bad_packets, imu.bad_packets), (1, 0));
    assert_eq!(gps.skipped_bytes, imu.skipped_bytes);
    assert!(gps.skipped_bytes < 40, "{}", gps.skipped_bytes);
    // each type has its own index
    assert_eq!(fs::read_dir(tmp.path().join("cache/packets")).unwrap().count(), 2);
}

#[test]
fn packet_types_without_a_length_frame_by_the_types_size() {
    let tmp = tempfile::tempdir().unwrap();
    let text = TYPES.replace("    length = { offset = 2, type = \"u16\" }\n", "").replace("\"zstd\"", "\"none\"");
    let d = Descriptor::parse(&text).unwrap();
    let p = d.packets.as_ref().unwrap();
    let mut data = Vec::new();
    for i in 0..6u8 {
        let (id, size) = if i % 2 == 0 { (1u8, 12) } else { (2, 14) };
        let mut header = vec![0u8; 8];
        header[4] = id;
        data.extend(encode_packet(p, true, &header, &vec![i; size]).unwrap());
    }
    let path = tmp.path().join("fixed.bin");
    fs::write(&path, &data).unwrap();
    for (k, want) in [3, 3].into_iter().enumerate() {
        let (pd, part) = d.parts()[k].clone();
        let f = PacketFile::open(&path, pd, part, "flight", &tmp.path().join("cache")).unwrap();
        assert_eq!((f.info().samples, f.bad_packets, f.skipped_bytes), (want, 0, 0), "type {k}");
    }
}
