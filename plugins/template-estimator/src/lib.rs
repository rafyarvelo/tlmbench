//! An alignment estimator template. It lines two files up on the moment their chosen channel peaks, which is easy
//! to follow and works for runs that each have one clear event (a launch, a hard stop). Replace `estimate`.
//!
//! The host calls `needs` with your parameters (JSON text, whatever the person typed in the Alignment section) and
//! then `estimate` with just those channels: `reference` on the shared timeline, `target` on its own clock, both
//! decimated to at most a million samples. Return knots `(file time, shared time)`: one knot is an offset, two add
//! drift. You get no files, network or clock, 2 s of CPU and 256 MB; the answer should depend on the inputs alone.

wit_bindgen::generate!({ path: "wit", world: "estimator" });

struct Peak;

fn channel(params: &str) -> Result<String, String> {
    let v: serde_json::Value = if params.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(params).map_err(|e| format!("the parameters are not JSON: {e}"))?
    };
    v["channel"].as_str().map(str::to_owned).ok_or_else(|| "give the channel to match on: {\"channel\": \"name\"}".into())
}

/// The time of the highest value.
fn peak(s: &Series) -> Option<i64> {
    s.v.iter().enumerate().filter(|(_, v)| !v.is_nan()).max_by(|a, b| a.1.total_cmp(b.1)).map(|(i, _)| s.t[i])
}

impl Guest for Peak {
    fn describe() -> About {
        About {
            name: "peak".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            description: Some("Lines files up on the moment a channel peaks".into()),
            files: vec![],
        }
    }

    fn needs(params: String) -> Result<Vec<String>, String> {
        Ok(vec![channel(&params)?])
    }

    fn estimate(reference: Vec<Series>, target: Vec<Series>, _hint: Vec<Knot>, params: String) -> Result<Estimate, String> {
        let name = channel(&params)?;
        let find = |v: &[Series]| v.iter().find(|s| s.name == name).and_then(peak);
        let (Some(r), Some(t)) = (find(&reference), find(&target)) else {
            return Err(format!("no samples of {name} to find a peak in"));
        };
        Ok(Estimate {
            knots: vec![Knot { file: t, shared: r }],
            residuals: vec![],
            quality: 1.0,
            note: Some(format!("Peaks of {name} matched")),
        })
    }
}

export!(Peak);
