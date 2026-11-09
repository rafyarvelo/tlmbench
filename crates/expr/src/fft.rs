//! The Spectrum view's transform: the frequency content of a channel over the time view.

use std::f64::consts::PI;

/// A single-sided amplitude spectrum.
#[derive(Clone, Debug, PartialEq)]
pub struct Spectrum {
    /// Amplitude at `k * df`, k = 0..=n/2 (a sine of amplitude A shows as A at its frequency).
    pub mag: Vec<f64>,
    /// Hz between bins.
    pub df: f64,
    pub nyquist: f64,
    /// The strongest bin above DC.
    pub peak_hz: f64,
    pub peak_amp: f64,
    /// Samples transformed (a power of two).
    pub n: usize,
}

/// The most samples one transform uses.
pub const MAX_POINTS: usize = 16384;
/// Fewest samples in a view that can be transformed.
pub const MIN_POINTS: usize = 16;

/// The largest power of two (at most `max`) that fits in `available` samples; `None` below [`MIN_POINTS`].
pub fn points(available: usize, max: usize) -> Option<usize> {
    if available < MIN_POINTS {
        return None;
    }
    let mut n = 1;
    while n * 2 <= available.min(max) {
        n *= 2;
    }
    Some(n)
}

/// In-place radix-2 FFT of `re`/`im` (length a power of two).
fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * PI / len as f64;
        let (wr, wi) = (ang.cos(), ang.sin());
        for i in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0, 0.0);
            for k in 0..len / 2 {
                let (a, b) = (i + k, i + k + len / 2);
                let (tr, ti) = (re[b] * cr - im[b] * ci, re[b] * ci + im[b] * cr);
                (re[b], im[b]) = (re[a] - tr, im[a] - ti);
                (re[a], im[a]) = (re[a] + tr, im[a] + ti);
                (cr, ci) = (cr * wr - ci * wi, cr * wi + ci * wr);
            }
        }
        len <<= 1;
    }
}

/// The spectrum of `x`, samples on an even grid at `rate_hz` (NaN is missing: it counts as the mean). The length
/// must be a power of two; `None` when it isn't, is under [`MIN_POINTS`], or more than half the samples are missing.
/// The mean is removed and a Hann window applied, with the gain corrected.
pub fn spectrum(x: &[f64], rate_hz: f64) -> Option<Spectrum> {
    let n = x.len();
    if n < MIN_POINTS || !n.is_power_of_two() || rate_hz.is_nan() || rate_hz <= 0.0 {
        return None;
    }
    let good: Vec<f64> = x.iter().copied().filter(|v| v.is_finite()).collect();
    if (good.len() as f64) < n as f64 * 0.5 {
        return None;
    }
    let mean = good.iter().sum::<f64>() / good.len() as f64;
    let mut re: Vec<f64> = x
        .iter()
        .enumerate()
        .map(|(i, v)| if v.is_finite() { v - mean } else { 0.0 } * (0.5 - 0.5 * (2.0 * PI * i as f64 / (n - 1) as f64).cos()))
        .collect();
    let mut im = vec![0.0; n];
    fft(&mut re, &mut im);
    // ×2 for the single side, ×2 for the Hann window's gain
    let mag: Vec<f64> =
        (0..=n / 2).map(|k| re[k].hypot(im[k]) * if k > 0 && k < n / 2 { 4.0 } else { 2.0 } / n as f64).collect();
    let df = rate_hz / n as f64;
    let peak = (2..mag.len()).fold(1, |p, k| if mag[k] > mag[p] { k } else { p });
    Some(Spectrum { peak_hz: peak as f64 * df, peak_amp: mag[peak], mag, df, nyquist: rate_hz / 2.0, n })
}
