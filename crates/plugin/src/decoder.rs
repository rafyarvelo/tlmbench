//! A WASM decoder plugin: the `decoder` world of `wit/tlmbench.wit`, as a [`DecoderPlugin`].

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tlmbench_decode::descriptor::DType;
use tlmbench_decode::plugin::{Chunk, DecoderPlugin, DecoderSession, PluginChannel, PluginInfo, PluginSchema, Tier};
use wasmtime::Store;
use wasmtime::component::{Component, HasSelf, Linker};

use crate::wasm::{self, Host, Limits};

mod bindings {
    wasmtime::component::bindgen!({ path: "wit/tlmbench.wit", world: "decoder" });
}
use bindings::Decoder;
use bindings::tlmbench::plugin::types::Dtype;

impl bindings::tlmbench::plugin::types::Host for Host {}

impl bindings::tlmbench::plugin::source::Host for Host {
    fn len(&mut self) -> u64 {
        self.file.as_ref().map_or(0, |f| f.1)
    }
    fn read(&mut self, offset: u64, len: u32) -> Vec<u8> {
        // a plugin may ask for a lot at once, never more than this
        const MAX: usize = 256 << 20;
        match &self.file {
            Some((f, size)) if offset < *size => {
                wasm::read_at(f, offset, (len as usize).min(MAX).min((*size - offset) as usize))
            }
            _ => Vec::new(),
        }
    }
}

fn dtype(d: Dtype) -> DType {
    match d {
        Dtype::I8 => DType::I8,
        Dtype::I16 => DType::I16,
        Dtype::I32 => DType::I32,
        Dtype::I64 => DType::I64,
        Dtype::U8 => DType::U8,
        Dtype::U16 => DType::U16,
        Dtype::U32 => DType::U32,
        Dtype::U64 => DType::U64,
        Dtype::F32 => DType::F32,
        Dtype::F64 => DType::F64,
    }
}

/// A loaded decoder component. Cheap to share: each file gets an instance of its own.
pub struct WasmDecoder {
    component: Component,
    linker: Linker<Host>,
    info: PluginInfo,
    limits: Limits,
    /// Instances for `sniff`: it runs once per unknown file, so one is kept.
    sniffer: Mutex<Option<(Store<Host>, Decoder)>>,
}

impl std::fmt::Debug for WasmDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WasmDecoder({})", self.info.name)
    }
}

fn instantiate(
    component: &Component,
    linker: &Linker<Host>,
    limits: Limits,
    file: Option<std::fs::File>,
) -> Result<(Store<Host>, Decoder), String> {
    let mut store = wasm::store(limits, file);
    wasm::arm(&mut store, limits);
    let d = Decoder::instantiate(&mut store, component, linker).map_err(|e| wasm::describe_error(e, "starting"))?;
    Ok((store, d))
}

impl WasmDecoder {
    pub fn load(path: &Path) -> Result<Self, String> {
        Self::load_with(path, Limits::DECODER)
    }

    pub fn load_with(path: &Path, limits: Limits) -> Result<Self, String> {
        let component = wasm::load(path)?;
        let mut linker = wasm::linker().map_err(|e| e.to_string())?;
        Decoder::add_to_linker::<Host, HasSelf<Host>>(&mut linker, |h| h).map_err(|e| e.to_string())?;
        let (mut store, d) = instantiate(&component, &linker, limits, None)?;
        wasm::arm(&mut store, limits);
        let about = d.call_describe(&mut store).map_err(|e| wasm::describe_error(e, "describe"))?;
        let info = PluginInfo {
            name: about.name,
            version: about.version,
            tier: Tier::Wasm,
            description: about.description,
            files: about.files,
            path: PathBuf::from(path),
        };
        Ok(Self { component, linker, info, limits, sniffer: Mutex::new(Some((store, d))) })
    }
}

impl DecoderPlugin for WasmDecoder {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn sniff(&self, head: &[u8], name: &str) -> bool {
        let mut guard = self.sniffer.lock().unwrap();
        let mut ok = false;
        if let Some((store, d)) = guard.as_mut() {
            wasm::arm(store, self.limits);
            match d.call_sniff(&mut *store, head, name) {
                Ok(b) => ok = b,
                // a trapped instance is not reused
                Err(_) => *guard = None,
            }
        }
        if guard.is_none() {
            *guard = instantiate(&self.component, &self.linker, self.limits, None).ok();
        }
        ok
    }

    fn open(&self, path: &Path) -> Result<Box<dyn DecoderSession>, String> {
        let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let (mut store, d) = instantiate(&self.component, &self.linker, self.limits, Some(file))?;
        wasm::arm(&mut store, self.limits);
        let schema = d
            .call_start(&mut store)
            .map_err(|e| wasm::describe_error(e, "open"))?
            .map_err(|e| format!("can't read this file: {e}"))?;
        let schema = PluginSchema {
            channels: schema
                .channels
                .into_iter()
                .map(|c| PluginChannel { name: c.name, unit: c.unit, dtype: dtype(c.dtype) })
                .collect(),
            timed: schema.timed,
        };
        Ok(Box::new(Session { store, d, schema, limits: self.limits }))
    }
}

struct Session {
    store: Store<Host>,
    d: Decoder,
    schema: PluginSchema,
    limits: Limits,
}

impl DecoderSession for Session {
    fn schema(&self) -> &PluginSchema {
        &self.schema
    }

    fn next(&mut self, max_rows: u32) -> Result<Option<Chunk>, String> {
        wasm::arm(&mut self.store, self.limits);
        let r = self.d.call_next(&mut self.store, max_rows).map_err(|e| wasm::describe_error(e, "decoding"))?;
        Ok(r.map_err(|e| format!("can't decode this file: {e}"))?.map(|c| Chunk {
            rows: c.rows as usize,
            time: c.time,
            columns: c.columns,
        }))
    }
}
