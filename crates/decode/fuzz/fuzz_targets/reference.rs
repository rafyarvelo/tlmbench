//! Any bytes as a tlmbench reference file: the header and descriptor parse, then every channel reduces over the
//! whole file and over a viewport taken from the input, read in place and through decoded chunks (which must give the
//! same trace). Errors are fine; panics and hangs are not.
#![no_main]

use libfuzzer_sys::fuzz_target;
use std::sync::Arc;

use tlmbench_core::chunks::{CHUNK_SAMPLES, Held};
use tlmbench_core::{Recording, Viewport};
use tlmbench_decode::fixed::FixedWidth;

fuzz_target!(|data: &[u8]| {
    let Ok(file) = FixedWidth::from_reference_bytes(data.to_vec()) else { return };
    let info = file.info();
    let (Some(a), Some(b)) = (info.t_first, info.t_last) else { return };
    let tail = data.len().saturating_sub(8);
    let seed = i64::from_le_bytes(data[tail..].try_into().unwrap_or([0; 8]));
    let chunks = info.samples.div_ceil(CHUNK_SAMPLES);
    let all: Vec<usize> = (0..info.channels.len()).filter(|&c| file.chunked(c)).collect();
    let mut decoded = file.decode_chunks(&all, 0..chunks);
    for ch in 0..info.channels.len() {
        let whole = Viewport { t0: a, t1: b.saturating_add(1), width_px: 64, raw_max: 0 };
        let near = Viewport { t0: seed, t1: seed.saturating_add(seed.unsigned_abs() as i64 / 3 + 1), width_px: 7, raw_max: 0 };
        let held = match all.iter().position(|&c| c == ch) {
            Some(i) => Held {
                first: 0,
                chunks: std::mem::take(&mut decoded[i]).into_iter().map(|c| Some(Arc::new(c))).collect(),
            },
            None => Held::default(),
        };
        for vp in [whole, near] {
            let (x, y) = (file.trace(ch, vp), file.view(ch, vp, None, &[], &held));
            assert_eq!(x.t, y.t);
            assert!(x.y.iter().zip(&y.y).all(|(p, q)| p.to_bits() == q.to_bits()));
        }
    }
});
