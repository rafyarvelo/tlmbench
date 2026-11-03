//! Any bytes as a Parquet file, decoded into a throwaway cache. Errors are fine; panics and hangs are not.
//!
//! The decoder turns panics inside the `parquet` crate into errors, so the default panic hook replaces libFuzzer's
//! (which aborts as soon as anything panics); a panic that escapes still aborts the run.
#![no_main]

use libfuzzer_sys::fuzz_target;
use tlmbench_core::Recording;

static HOOK: std::sync::Once = std::sync::Once::new();

fuzz_target!(|data: &[u8]| {
    HOOK.call_once(|| drop(std::panic::take_hook()));
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("f.parquet");
    std::fs::write(&path, data).unwrap();
    if let Ok(r) = tlmbench_decode::parquet::open_parquet(&path, &tmp.path().join("cache")) {
        let info = r.info();
        assert!(info.t_first <= info.t_last);
    }
});
