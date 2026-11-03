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
        packets: Some(Packets {
            sync: Some("EB90".into()),
            header_size: 12,
            length: LengthField { offset: 2, dtype: DType::U16, counts: Counts::Payload, adjust: 0 },
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
    PacketFile::open(&path, d.clone(), "bench", &dir.join("cache")).unwrap()
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
    let b = PacketFile::open(&tmp.path().join("once.bin"), d, "bench", &tmp.path().join("cache")).unwrap();
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
    let err = PacketFile::open(&path, d, "bench", &tmp.path().join("cache")).unwrap_err().to_string();
    assert!(err.contains("time goes backwards"), "{err}");
    assert!(
        !tmp.path().join("cache/packets").exists()
            || fs::read_dir(tmp.path().join("cache/packets")).unwrap().count() == 0
    );
}
