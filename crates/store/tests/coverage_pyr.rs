//! A build that is stopped and resumed on a recording with a time column (coverage, R6c.3, Nov 26): the times kept for
//! every 64k-th sample have to come out the same as in a build that was never stopped.

use std::fs;
use std::io::Write;
use std::path::Path;

use tlmbench_decode::descriptor::{DType, Descriptor, Endian, Field, SCHEMA, TimeSpec, TimeUnit};
use tlmbench_decode::reference;
use tlmbench_store::{Status, Store};

fn field(name: &str, dtype: DType, offset: u64) -> Field {
    Field {
        name: name.into(),
        dtype,
        offset,
        unit: None,
        scale: 1.0,
        value_offset: 0.0,
        category: None,
        bits: None,
        plugin: None,
        size: 0,
        params: None,
    }
}

fn write(path: &Path, n: usize) {
    let d = Descriptor {
        sequence: None,
        files: Vec::new(),
        packet_types: Vec::new(),
        events: Vec::new(),
        packets: None,
        schema: SCHEMA,
        name: None,
        endian: Endian::Little,
        header_size: 0,
        record_size: 12,
        time: TimeSpec::Field { field: "time".into(), unit: TimeUnit::Ns, t0_ns: 0 },
        fields: vec![field("time", DType::I64, 0), field("v", DType::F32, 8)],
    };
    let mut w = std::io::BufWriter::new(fs::File::create(path).unwrap());
    reference::write_header(&mut w, &d).unwrap();
    for i in 0..n {
        // a little jitter, so the times are not on a grid
        w.write_all(&((i as i64) * 1_000_000 + (i as i64 % 7) * 1000).to_le_bytes()).unwrap();
        w.write_all(&((i % 1000) as f32).to_le_bytes()).unwrap();
    }
    w.flush().unwrap();
}

#[test]
fn a_resumed_build_of_a_time_column_keeps_the_times_of_the_blocks_already_done() {
    let data = tempfile::tempdir().unwrap();
    let path = data.path().join("t.tlm");
    let n = (1usize << 20) + (1 << 18);
    write(&path, n);
    let (c1, c2) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let whole = Store::new(Some(c1.path())).open(&path, None).unwrap();
    whole.build(&mut |_, _| {}).unwrap();

    let store = Store::new(Some(c2.path()));
    let first = store.open(&path, None).unwrap();
    let blocks = std::cell::Cell::new(0);
    first
        .build(&mut |done, _| {
            if done > 0 {
                blocks.set(blocks.get() + 1);
            }
            if blocks.get() == 1 {
                first.cancel();
            }
        })
        .unwrap();
    assert_eq!(first.status(), Status::Building { done: 1 << 20, total: n as u64 });
    drop(first);
    let second = store.open(&path, None).unwrap();
    let mut starts = None;
    second
        .build(&mut |done, _| {
            starts.get_or_insert(done);
        })
        .unwrap();
    assert_eq!(starts, Some(1 << 20), "resumed after the first block");
    assert_eq!(second.status(), Status::Built);
    let dir = |root: &Path| root.join("pyr/v2").join(second.key());
    for f in ["c0.pyr", "gaps.bin", "times.bin"] {
        assert_eq!(fs::read(dir(c1.path()).join(f)).unwrap(), fs::read(dir(c2.path()).join(f)).unwrap(), "{f}");
    }
    // the time index answers a search near the end the same in both
    let (a, b) = (whole.recording(), second.recording());
    let t = a.time_at(n - 5000);
    assert_eq!((a.lower_bound(t), b.lower_bound(t)), (n - 5000, n - 5000));
}
