//! A person's adjustments to channels (scale, offset, missing values) as the app states them, turned into what the
//! decoders apply (see [`tlmbench_core::adjust`]). A "missing when" condition is an expression over the file's own
//! channels; it is evaluated here, once, into the samples it marks.

use tlmbench_core::Recording;
use tlmbench_core::adjust::ChannelAdjust;
use tlmbench_core::mask::Mask;

use crate::derived::RecEnv;
use crate::{Error, Result};

/// One channel's adjustments, with the missing-when condition still as text.
#[derive(Clone, Debug, PartialEq)]
pub struct Adjust {
    pub scale: f64,
    pub offset: f64,
    pub values: Vec<f64>,
    pub lo: Option<f64>,
    pub hi: Option<f64>,
    pub when: Option<String>,
}

impl Default for Adjust {
    fn default() -> Self {
        Self { scale: 1.0, offset: 0.0, values: Vec::new(), lo: None, hi: None, when: None }
    }
}

/// The samples of `rec` where `expr` holds (a finite, non-zero result), as a mask.
pub fn when_mask(rec: &dyn Recording, expr: &str) -> Result<Mask> {
    let bad = |m: String| Error::Invalid(m);
    let e = tlmbench_expr::parse(expr).map_err(|e| bad(format!("Missing when: {} (at {})", e.msg, e.at)))?;
    let env = RecEnv::new(rec);
    let mut runs: Vec<std::ops::Range<usize>> = Vec::new();
    let flat = tlmbench_expr::run(&e, &env, 1 << 20, &mut |start, s| {
        for (k, v) in s.v.iter().enumerate() {
            if !(v.is_finite() && *v != 0.0) {
                continue;
            }
            let i = start + k;
            match runs.last_mut() {
                Some(r) if r.end == i => r.end = i + 1,
                _ => runs.push(i..i + 1),
            }
        }
    })
    .map_err(|e| bad(format!("Missing when: {} (at {})", e.msg, e.at)))?;
    if let Some(x) = flat {
        // a condition that is the same everywhere: all samples, or none
        if x.is_finite() && x != 0.0 {
            runs.push(0..rec.info().samples);
        }
    }
    Ok(Mask::from_runs(runs))
}

/// `adjust` as the decoders take it, with each condition evaluated against the plain recording that has its channel
/// (`plain` are the file's recordings opened without any adjustment).
pub fn resolve(adjust: &[(String, Adjust)], plain: &[&dyn Recording]) -> Result<Vec<(String, ChannelAdjust)>> {
    adjust
        .iter()
        .map(|(name, a)| {
            let when = match &a.when {
                Some(w) if !w.trim().is_empty() => {
                    let rec = plain.iter().find(|r| r.info().channels.iter().any(|c| &c.name == name));
                    rec.map(|r| when_mask(*r, w)).transpose()?
                }
                _ => None,
            };
            Ok((
                name.clone(),
                ChannelAdjust { scale: a.scale, offset: a.offset, values: a.values.clone(), lo: a.lo, hi: a.hi, when },
            ))
        })
        .collect()
}
