//! Butterworth-style filters as RBJ biquads, run forward and backward on recorded files (zero phase, 4th-order
//! roll-off). Gaps are bridged at the channel's rate.

use crate::ast::ExprError;
use crate::eval::{Val, sample_op};
use crate::series::{Grid, Series};

pub fn pass(s: &Series, low: bool, fc: Option<f64>, f: &str, at: usize) -> Result<Val, ExprError> {
    let kind = if low { "low" } else { "high" };
    let Some(fc) = fc.filter(|c| *c > 0.0) else {
        return Err(ExprError::new(
            format!("{f}(x, Hz): the cutoff is a frequency in Hz, like {kind}pass(accel_x, 5)"),
            at,
        ));
    };
    let fs = match &s.grid {
        Grid::Rate(hz) => *hz,
        Grid::Stamps(t) if t.len() > 1 => (t.len() - 1) as f64 / (t[t.len() - 1] - t[0]),
        Grid::Stamps(_) => 1.0,
    };
    if fc >= fs / 2.0 {
        return Err(ExprError::new(
            format!("The cutoff ({fc} Hz) must be below half the sample rate ({} Hz)", fs / 2.0),
            at,
        ));
    }
    let w0 = 2.0 * std::f64::consts::PI * fc / fs;
    let (cw, al) = (w0.cos(), w0.sin() / std::f64::consts::SQRT_2);
    let a0 = 1.0 + al;
    let b = if low {
        [(1.0 - cw) / 2.0, 1.0 - cw, (1.0 - cw) / 2.0]
    } else {
        [(1.0 + cw) / 2.0, -(1.0 + cw), (1.0 + cw) / 2.0]
    };
    let (a1, a2) = (-2.0 * cw / a0, (1.0 - al) / a0);
    let (b0, b1, b2) = (b[0] / a0, b[1] / a0, b[2] / a0);
    let run = |v: &[f64]| -> Vec<f64> {
        let (mut x1, mut x2) = (v[0], v[0]);
        let (mut y1, mut y2) = if low { (v[0], v[0]) } else { (0.0, 0.0) };
        v.iter()
            .map(|&x| {
                let y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
                (x2, x1, y2, y1) = (x1, x, y1, y);
                y
            })
            .collect()
    };
    Ok(sample_op(s, |v, _| {
        if v.len() < 3 {
            return v.to_vec();
        }
        let mut fwd = run(v);
        fwd.reverse();
        let mut back = run(&fwd);
        back.reverse();
        back
    }))
}
