use std::f64::consts::PI;
use tlmbench_expr::fft::{MAX_POINTS, points, spectrum};

fn sine(n: usize, hz: f64, rate: f64, amp: f64) -> Vec<f64> {
    (0..n).map(|i| amp * (2.0 * PI * hz * i as f64 / rate).sin()).collect()
}

#[test]
fn a_sine_shows_its_frequency_and_amplitude() {
    // 125 Hz is 128 bins of 0.977 Hz
    let x: Vec<f64> =
        sine(1024, 125.0, 1000.0, 2.0).iter().map(|v| v + 7.0).collect();
    let s = spectrum(&x, 1000.0).unwrap();
    assert_eq!((s.n, s.mag.len(), s.nyquist), (1024, 513, 500.0));
    assert!((s.peak_hz - 125.0).abs() <= s.df, "{}", s.peak_hz);
    // a Hann window spreads over neighbouring bins: the peak is within 15% of the amplitude, and the offset is gone
    assert!(s.peak_amp > 1.7 && s.peak_amp < 2.1, "{}", s.peak_amp);
    assert!(s.mag[0] < 0.05, "the 7.0 offset is removed: {}", s.mag[0]);
    // two tones: both peaks, the stronger one is reported
    let two: Vec<f64> =
        sine(4096, 50.0, 1000.0, 1.0).iter().zip(sine(4096, 200.0, 1000.0, 3.0)).map(|(a, b)| a + b).collect();
    let s = spectrum(&two, 1000.0).unwrap();
    assert!((s.peak_hz - 200.0).abs() <= s.df);
    let at = |hz: f64| s.mag[(hz / s.df).round() as usize];
    assert!(at(50.0) > 0.7 && at(50.0) < 1.2 && at(200.0) > 2.5);
}

#[test]
fn refuses_what_it_cannot_transform() {
    assert!(spectrum(&sine(8, 1.0, 10.0, 1.0), 10.0).is_none(), "under 16 samples");
    assert!(spectrum(&sine(100, 1.0, 10.0, 1.0), 10.0).is_none(), "not a power of two");
    let mut gappy = sine(64, 1.0, 10.0, 1.0);
    gappy[..40].fill(f64::NAN);
    assert!(spectrum(&gappy, 10.0).is_none(), "more than half missing");
    gappy[..20].fill(1.0);
    assert!(spectrum(&gappy, 10.0).is_some());
    assert_eq!(points(15, MAX_POINTS), None);
    assert_eq!(
        (points(16, MAX_POINTS), points(1000, MAX_POINTS), points(1 << 30, MAX_POINTS)),
        (Some(16), Some(512), Some(16384))
    );
    assert_eq!(points(1 << 30, 1 << 20), Some(1 << 20));
}
