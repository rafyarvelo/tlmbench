//! Any bytes as a tlmbench reference file: the header and descriptor parse, then every channel reduces over the
//! whole file and over a viewport taken from the input. Errors are fine; panics and hangs are not.
#![no_main]

use libfuzzer_sys::fuzz_target;
use tlmbench_core::{Recording, Viewport};
use tlmbench_decode::fixed::FixedWidth;

fuzz_target!(|data: &[u8]| {
    let Ok(file) = FixedWidth::from_reference_bytes(data.to_vec()) else { return };
    let info = file.info();
    let (Some(a), Some(b)) = (info.t_first, info.t_last) else { return };
    let tail = data.len().saturating_sub(8);
    let seed = i64::from_le_bytes(data[tail..].try_into().unwrap_or([0; 8]));
    for ch in 0..info.channels.len() {
        file.trace(ch, Viewport { t0: a, t1: b.saturating_add(1), width_px: 64 });
        file.trace(ch, Viewport { t0: seed, t1: seed.saturating_add(seed.unsigned_abs() as i64 / 3 + 1), width_px: 7 });
    }
});
