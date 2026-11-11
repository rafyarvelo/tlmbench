//! A WASM alignment estimator: the `estimator` world of `wit/tlmbench.wit`, as a store [`EstimatorPlugin`].
//!
//! Every call runs in a fresh instance (nothing carries over between runs, so the answer depends on the inputs
//! alone) with the design's limits: 2 s of CPU and 256 MB, no files, no network.

use std::path::Path;

use tlmbench_core::timemap::TimeMap;
use tlmbench_store::align::{EstimatorPlugin, PluginEstimate, Series};
use wasmtime::component::{Component, HasSelf, Linker};

use crate::wasm::{self, Host, Limits};

mod bindings {
    wasmtime::component::bindgen!({ path: "wit/tlmbench.wit", world: "estimator" });
}
use bindings::{Estimator, Knot, Series as WitSeries};

impl bindings::tlmbench::plugin::types::Host for Host {}

pub struct WasmEstimator {
    component: Component,
    linker: Linker<Host>,
    info: tlmbench_decode::plugin::PluginInfo,
    limits: Limits,
}

impl std::fmt::Debug for WasmEstimator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WasmEstimator({})", self.info.name)
    }
}

impl WasmEstimator {
    pub fn load(path: &Path) -> Result<Self, String> {
        Self::load_with(path, Limits::ESTIMATOR)
    }

    pub fn load_with(path: &Path, limits: Limits) -> Result<Self, String> {
        let component = wasm::load(path)?;
        let mut linker = wasm::linker().map_err(|e| e.to_string())?;
        Estimator::add_to_linker::<Host, HasSelf<Host>>(&mut linker, |h| h).map_err(|e| e.to_string())?;
        let mut s = wasm::store(limits, None);
        wasm::arm(&mut s, limits);
        let e = Estimator::instantiate(&mut s, &component, &linker).map_err(|e| wasm::describe_error(e, "starting"))?;
        let about = e.call_describe(&mut s).map_err(|e| wasm::describe_error(e, "describe"))?;
        let info = tlmbench_decode::plugin::PluginInfo {
            name: about.name,
            version: about.version,
            tier: tlmbench_decode::plugin::Tier::Wasm,
            description: about.description,
            files: Vec::new(),
            path: path.to_owned(),
        };
        Ok(Self { component, linker, info, limits })
    }

    pub fn info(&self) -> &tlmbench_decode::plugin::PluginInfo {
        &self.info
    }

    fn fresh(&self) -> Result<(wasmtime::Store<Host>, Estimator), String> {
        let mut s = wasm::store(self.limits, None);
        wasm::arm(&mut s, self.limits);
        let e = Estimator::instantiate(&mut s, &self.component, &self.linker)
            .map_err(|e| wasm::describe_error(e, "starting"))?;
        Ok((s, e))
    }
}

fn wit(series: &[Series]) -> Vec<WitSeries> {
    series.iter().map(|s| WitSeries { name: s.name.clone(), t: s.t.clone(), v: s.v.clone() }).collect()
}

impl EstimatorPlugin for WasmEstimator {
    fn name(&self) -> &str {
        &self.info.name
    }

    fn needs(&self, params: &str) -> Result<Vec<String>, String> {
        let (mut s, e) = self.fresh()?;
        wasm::arm(&mut s, self.limits);
        e.call_needs(&mut s, params).map_err(|e| wasm::describe_error(e, "needs"))?
    }

    fn estimate(
        &self,
        reference: &[Series],
        target: &[Series],
        hint: &TimeMap,
        params: &str,
    ) -> Result<PluginEstimate, String> {
        let (mut s, e) = self.fresh()?;
        wasm::arm(&mut s, self.limits);
        let hint: Vec<Knot> = hint.knots.iter().map(|&(file, shared)| Knot { file, shared }).collect();
        let out = e
            .call_estimate(&mut s, &wit(reference), &wit(target), &hint, params)
            .map_err(|e| wasm::describe_error(e, "estimate"))??;
        Ok(PluginEstimate {
            knots: out.knots.into_iter().map(|k| (k.file, k.shared)).collect(),
            residuals: out.residuals,
            quality: out.quality,
            note: out.note,
        })
    }
}
