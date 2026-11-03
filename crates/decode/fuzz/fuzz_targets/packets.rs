//! Any bytes as a zstd packet file with a CRC (sync EB90, a u16 length), indexed into a throwaway cache, then read
//! through views and decoded chunks. Errors are fine; panics and hangs are not.
#![no_main]

use libfuzzer_sys::fuzz_target;
use tlmbench_core::{Channels, Recording, Viewport};
use tlmbench_decode::descriptor::Descriptor;
use tlmbench_decode::packets::PacketFile;

const DESC: &str = r#"
schema = 1
endian = "big"
record_size = 6
[time]
source = "field"
field = "t"
unit = "ms"
[[fields]]
name = "t"
type = "u32"
offset = 8
[[fields]]
name = "v"
type = "i16"
offset = 12
[packets]
sync = "EB90"
header_size = 8
length = { offset = 2, type = "u16" }
checksum = "crc16-ccitt"
compression = "zstd"
"#;

fuzz_target!(|data: &[u8]| {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("f.bin");
    std::fs::write(&path, data).unwrap();
    let desc = Descriptor::parse(DESC).unwrap();
    if let Ok(f) = PacketFile::open(&path, desc, None, "fuzz", &tmp.path().join("cache")) {
        let n = f.info().samples;
        if n > 0 {
            let (a, b) = (f.info().t_first.unwrap(), f.info().t_last.unwrap());
            assert!(a <= b);
            let tr = f.trace(0, Viewport { t0: a, t1: b + 1, width_px: 50 });
            assert!(tr.t.windows(2).all(|w| w[0] <= w[1]));
            let chunks = Channels::decode_chunks(&f, &[0], 0..n.div_ceil(1 << 16));
            assert_eq!(chunks[0].iter().map(|c| c.len()).sum::<usize>(), n);
        }
    }
});
