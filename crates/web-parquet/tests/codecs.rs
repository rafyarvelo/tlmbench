//! Every codec pyarrow writes reads the same: the same channels, units, times, nulls and values (the files come from
//! `tools/testdata/parquet_codecs.py`). Zstandard goes through the shim (ruzstd), the others through pure-Rust codecs.

use serde_json::Value;
use tlmbench_web_parquet::parquet_dump;

struct Dump {
    meta: Value,
    t: Vec<i64>,
    cols: Vec<Vec<f64>>,
}

fn read(codec: &str) -> Result<Dump, String> {
    let path = format!("{}/tests/fixtures/codecs-{codec}.parquet", env!("CARGO_MANIFEST_DIR"));
    let bytes = std::fs::read(&path).map_err(|e| format!("{path}: {e}"))?;
    let out = parquet_dump(&path, &bytes)?;
    let n = u64::from_le_bytes(out[..8].try_into().unwrap()) as usize;
    let meta: Value = serde_json::from_slice(&out[8..8 + n]).unwrap();
    let samples = meta["samples"].as_u64().unwrap() as usize;
    let mut at = 8 + n;
    let mut take = |len: usize| -> Vec<[u8; 8]> {
        let v = out[at..at + len * 8].chunks_exact(8).map(|b| b.try_into().unwrap()).collect();
        at += len * 8;
        v
    };
    let t = take(samples).into_iter().map(i64::from_le_bytes).collect();
    let cols = (0..meta["channels"].as_array().unwrap().len())
        .map(|_| take(samples).into_iter().map(f64::from_le_bytes).collect())
        .collect();
    assert_eq!(at, out.len(), "the dump holds exactly the times and the channels");
    Ok(Dump { meta, t, cols })
}

#[test]
fn every_codec_reads_the_same_file() {
    for codec in ["none", "snappy", "gzip", "brotli", "lz4", "zstd"] {
        let d = read(codec).unwrap_or_else(|e| panic!("{codec}: {e}"));
        let m = &d.meta;
        assert_eq!(m["format"], "Parquet", "{codec}");
        assert_eq!(m["samples"], 1999, "{codec}: row 7 has no time");
        assert_eq!(m["skipped"], serde_json::json!(["label"]), "{codec}");
        assert_eq!(m["time"], serde_json::json!({ "kind": "column", "name": "ts" }), "{codec}");
        let chans: Vec<(&str, Option<&str>)> = m["channels"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| (c["name"].as_str().unwrap(), c["unit"].as_str()))
            .collect();
        assert_eq!(chans, [("speed", Some("m/s")), ("Temp", Some("°C")), ("armed", None)], "{codec}");
        // microseconds to nanoseconds, row 7 gone
        assert_eq!((d.t[0], d.t[6], d.t[7], d.t[1998]), (0, 6_000_000, 8_000_000, 1_999_000_000), "{codec}");
        let (speed, temp, armed) = (&d.cols[0], &d.cols[1], &d.cols[2]);
        assert_eq!((speed[6], speed[7], speed[1998]), (3.0, 4.0, 999.5), "{codec}");
        assert!(speed[8].is_nan(), "{codec}: a null float is NaN");
        assert!(
            temp[10].is_nan() && temp[9] == 10.0 && temp[300] == 1.0,
            "{codec}: an integer with a null widens to f64"
        );
        assert_eq!((armed[0], armed[1], armed[2]), (1.0, 0.0, 1.0), "{codec}");
    }
}

#[test]
fn what_is_not_parquet_or_is_cut_short_is_an_error_not_a_crash() {
    assert!(parquet_dump("a.parquet", b"time,x\n0,1\n").unwrap_err().contains("not a Parquet file"));
    let path = format!("{}/tests/fixtures/codecs-snappy.parquet", env!("CARGO_MANIFEST_DIR"));
    let bytes = std::fs::read(path).unwrap();
    let e = parquet_dump("a.parquet", &bytes[..bytes.len() / 2]).unwrap_err();
    assert!(e.starts_with("a.parquet: Parquet:"), "{e}");
}
