//! TelemetryBench plugins: custom decoders and alignment estimators as WASM components on wasmtime (sandboxed), and
//! native decoders (trusted code).

mod decoder;
mod estimator;
mod wasm;

pub use decoder::WasmDecoder;
pub use estimator::WasmEstimator;
pub use wasm::Limits;

/// What a plugin does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Decoder,
    Estimator,
}
