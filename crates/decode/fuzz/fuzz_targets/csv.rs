//! Any bytes as CSV: schema detection and chunked parsing. Errors are fine; panics and hangs are not.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok((schema, rows)) = tlmbench_decode::csv::parse_all(data) {
        assert!(rows.columns.iter().all(|c| c.len() == rows.rows));
        assert!(schema.time == tlmbench_decode::csv::TimeColumn::Index || rows.t.len() == rows.rows);
    }
});
