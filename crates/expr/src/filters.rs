//! Butterworth-style filters as RBJ biquads. On recorded files they run forward and backward (zero phase, 4th-order
//! roll-off); a chunk is filtered with a margin past its end, long enough for the backward pass's start-up to die
//! away (see [`margin`]). Gaps are bridged at the channel's rate.

/// A second-order section's coefficients.
#[derive(Clone, Copy, Debug)]
pub struct Biquad {
    b: [f64; 3],
    a1: f64,
    a2: f64,
    low: bool,
}

impl Biquad {
    /// `None` unless `0 < fc < fs / 2`.
    pub fn new(low: bool, fc: f64, fs: f64) -> Option<Self> {
        if !(fc > 0.0 && fc < fs / 2.0) {
            return None;
        }
        let w0 = 2.0 * std::f64::consts::PI * fc / fs;
        let (cw, al) = (w0.cos(), w0.sin() / std::f64::consts::SQRT_2);
        let a0 = 1.0 + al;
        let b = if low {
            [(1.0 - cw) / 2.0, 1.0 - cw, (1.0 - cw) / 2.0]
        } else {
            [(1.0 + cw) / 2.0, -(1.0 + cw), (1.0 + cw) / 2.0]
        };
        Some(Self { b: [b[0] / a0, b[1] / a0, b[2] / a0], a1: -2.0 * cw / a0, a2: (1.0 - al) / a0, low })
    }

    fn start(&self, first: f64) -> [f64; 4] {
        [first, first, if self.low { first } else { 0.0 }, if self.low { first } else { 0.0 }]
    }

    /// Runs over `v` from `state` (x1, x2, y1, y2), leaving it after the last sample.
    fn run(&self, state: &mut [f64; 4], v: &[f64]) -> Vec<f64> {
        let [mut x1, mut x2, mut y1, mut y2] = *state;
        let out = v
            .iter()
            .map(|&x| {
                let y = self.b[0] * x + self.b[1] * x1 + self.b[2] * x2 - self.a1 * y1 - self.a2 * y2;
                (x2, x1, y2, y1) = (x1, x, y1, y);
                y
            })
            .collect();
        *state = [x1, x2, y1, y2];
        out
    }
}

/// How a biquad's zero-phase pass over a chunk carries on: the forward state after the committed samples.
#[derive(Clone, Copy, Debug, Default)]
pub struct Carry {
    pub state: [f64; 4],
    pub started: bool,
}

/// Zero-phase filtering of the non-gap samples `v` of a chunk whose first `k` samples are committed (the rest is the
/// margin): the forward pass continues from `carry` and leaves it after sample `k`; the backward pass starts at the
/// end of `v`, so its start-up transient falls in the margin.
pub fn zero_phase(bq: &Biquad, carry: &mut Carry, v: &[f64], k: usize) -> Vec<f64> {
    if v.is_empty() {
        return Vec::new();
    }
    if !carry.started {
        if v.len() < 3 {
            return v.to_vec();
        }
        carry.state = bq.start(v[0]);
        carry.started = true;
    }
    let mut fwd = bq.run(&mut carry.state, &v[..k]);
    if k < v.len() {
        let mut tmp = carry.state;
        fwd.extend(bq.run(&mut tmp, &v[k..]));
    }
    fwd.reverse();
    let mut back_state = bq.start(fwd[0]);
    let mut back = bq.run(&mut back_state, &fwd);
    back.reverse();
    back
}

/// Samples after a chunk's end that a zero-phase pass at cutoff `fc` (sample rate `fs`) needs: the start-up of the
/// backward pass decays as exp(-n * w0 / sqrt 2); 20 time constants leave less than 1e-8 of it.
pub fn margin(fc: f64, fs: f64) -> usize {
    let w0 = 2.0 * std::f64::consts::PI * fc / fs;
    ((20.0 * std::f64::consts::SQRT_2 / w0).ceil() as usize).min(1 << 24)
}
