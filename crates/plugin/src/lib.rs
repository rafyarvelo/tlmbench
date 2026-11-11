//! TelemetryBench plugins: custom decoders and alignment estimators as WASM components on wasmtime (sandboxed), and
//! native decoders (trusted code).
//!
//! [`install`] reads the plugins folders and hands what it loads to the decoders (`tlmbench_decode::plugin`) and to
//! alignment (`tlmbench_store::align`); [`Found`] is what Settings lists, loaded or not.
//!
//! Trust: a `.wasm` file runs in the sandbox, so it loads from the person's own folder and from a folder's shared
//! `.tlmbench/plugins/` once the person has trusted that folder (until then they are listed, not loaded). A native library runs with the person's rights, so it loads only from their own folder and
//! only once they turn it on (by file name): until then it isn't even opened.

mod decoder;
mod estimator;
mod native;
mod wasm;

use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use decoder::WasmDecoder;
pub use estimator::WasmEstimator;
pub use native::NativeDecoder;
pub use tlmbench_decode::plugin::Tier;
pub use wasm::Limits;

use tlmbench_decode::plugin::DecoderPlugin;
use tlmbench_store::align::EstimatorPlugin;

/// What a plugin does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Decoder,
    Estimator,
}

/// A WASM plugin file loaded on its own (the author's `tlmbench plugins --check`).
#[derive(Debug)]
pub enum Loaded {
    Decoder(WasmDecoder),
    Estimator(WasmEstimator),
}

/// Loads one `.wasm` file, whichever world it implements.
pub fn load_wasm(path: &Path) -> Result<Loaded, String> {
    match wasm::kind_of(&wasm::load(path)?) {
        Some(Kind::Decoder) => WasmDecoder::load(path).map(Loaded::Decoder),
        Some(Kind::Estimator) => WasmEstimator::load(path).map(Loaded::Estimator),
        None => Err(format!("{}: it implements neither a decoder nor an estimator", path.display())),
    }
}

/// Where a plugin stands.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(tag = "state", content = "why", rename_all = "lowercase")]
pub enum Status {
    Loaded,
    /// A native plugin the person hasn't turned on: not loaded.
    Off,
    /// In a folder's shared plugins that the person hasn't trusted yet: listed, not loaded (not even compiled).
    Untrusted,
    /// Loading failed, or it isn't allowed here.
    Failed(String),
}

/// One plugin file, as Settings lists it.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Found {
    /// The file name: what turning a native plugin on is keyed by.
    pub file: String,
    pub path: PathBuf,
    pub tier: Tier,
    /// None until the plugin has been loaded.
    pub kind: Option<Kind>,
    /// The name the plugin gives; the file's stem when it isn't loaded.
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    /// Found in a folder's shared `.tlmbench/plugins/` and not the person's own.
    pub shared: bool,
    /// For a shared plugin: that plugins folder, which is what trusting it is keyed by.
    pub folder: Option<String>,
    pub status: Status,
}

/// What `install` loaded.
#[derive(Debug, Default)]
pub struct Installed {
    pub found: Vec<Found>,
}

fn files_in(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
        .collect();
    v.sort();
    v
}

/// Reads the plugins folders and replaces the loaded plugins everywhere. `own` is the person's plugins folder (the
/// only one native plugins load from), `shared` are folders' `.tlmbench/plugins/` and `trusted` the ones the person
/// has said to load (by path; an untrusted folder's plugins are only listed), `native_on` the file names of the
/// native plugins they have turned on.
pub fn install(own: Option<&Path>, shared: &[PathBuf], trusted: &[String], native_on: &[String]) -> Installed {
    let mut found = Vec::new();
    let mut decoders: Vec<Arc<dyn DecoderPlugin>> = Vec::new();
    let mut estimators: Vec<Arc<dyn EstimatorPlugin>> = Vec::new();
    let dirs = own.iter().map(|d| (d.to_path_buf(), false)).chain(shared.iter().map(|d| (d.clone(), true)));
    for (dir, is_shared) in dirs {
        for path in files_in(&dir) {
            let file = path.file_name().unwrap().to_string_lossy().into_owned();
            let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let mut f = Found {
                file: file.clone(),
                path: path.clone(),
                tier: Tier::Wasm,
                kind: None,
                name: path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
                version: None,
                description: None,
                shared: is_shared,
                folder: is_shared.then(|| dir.to_string_lossy().into_owned()),
                status: Status::Loaded,
            };
            let held_back = is_shared && !trusted.contains(&dir.to_string_lossy().into_owned());
            if ext == "wasm" && held_back {
                f.status = Status::Untrusted;
            } else if ext == "wasm" {
                match wasm::load(&path).map(|c| (wasm::kind_of(&c), c)) {
                    Err(e) => f.status = Status::Failed(e),
                    Ok((None, _)) => {
                        f.status = Status::Failed("it implements neither a decoder nor an estimator".into())
                    }
                    Ok((Some(Kind::Decoder), _)) => match WasmDecoder::load(&path) {
                        Ok(d) => {
                            fill(&mut f, Kind::Decoder, d.info());
                            decoders.push(Arc::new(d));
                        }
                        Err(e) => f.status = Status::Failed(e),
                    },
                    Ok((Some(Kind::Estimator), _)) => match WasmEstimator::load(&path) {
                        Ok(e) => {
                            fill(&mut f, Kind::Estimator, e.info());
                            estimators.push(Arc::new(e));
                        }
                        Err(e) => f.status = Status::Failed(e),
                    },
                }
            } else if ext == std::env::consts::DLL_EXTENSION {
                f.tier = Tier::Native;
                f.kind = Some(Kind::Decoder);
                if is_shared {
                    f.status = Status::Failed("Native plugins load only from your own plugins folder".into());
                } else if !native_on.contains(&file) {
                    f.status = Status::Off;
                } else {
                    match NativeDecoder::load(&path) {
                        Ok(d) => {
                            fill(&mut f, Kind::Decoder, d.info());
                            decoders.push(Arc::new(d));
                        }
                        Err(e) => f.status = Status::Failed(e),
                    }
                }
            } else {
                continue;
            }
            found.push(f);
        }
    }
    tlmbench_decode::plugin::set_plugins(decoders);
    tlmbench_store::align::set_estimators(estimators);
    Installed { found }
}

fn fill(f: &mut Found, kind: Kind, info: &tlmbench_decode::plugin::PluginInfo) {
    f.kind = Some(kind);
    f.name = info.name.clone();
    f.version = Some(info.version.clone());
    f.description = info.description.clone();
}
