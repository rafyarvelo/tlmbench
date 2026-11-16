//! A WASM payload plugin: the `payload` world of `wit/tlmbench.wit`, as a [`PayloadPlugin`]. A descriptor names it for a
//! packet type or a field; the host hands it bytes only (no files, network or clock), under the decoder's limits.

use std::path::{Path, PathBuf};

use tlmbench_decode::descriptor::DType;
use tlmbench_decode::payload::{PayloadPlugin, PayloadSession};
use tlmbench_decode::plugin::{Chunk, PluginChannel, PluginInfo, PluginSchema, Tier};
use wasmtime::Store;
use wasmtime::component::{Component, HasSelf, Linker};

use crate::wasm::{self, Host, Limits};

// decode-field's call takes the store and its six arguments
#[allow(clippy::too_many_arguments)]
mod bindings {
    wasmtime::component::bindgen!({ path: "wit/tlmbench.wit", world: "payload" });
}
use bindings::Payload;
use bindings::tlmbench::plugin::types::Dtype;

impl bindings::tlmbench::plugin::types::Host for Host {}

fn from_wit(d: Dtype) -> DType {
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

fn to_wit(d: DType) -> Dtype {
    match d {
        DType::I8 => Dtype::I8,
        DType::I16 => Dtype::I16,
        DType::I32 => Dtype::I32,
        DType::I64 => Dtype::I64,
        DType::U8 => Dtype::U8,
        DType::U16 => Dtype::U16,
        DType::U32 => Dtype::U32,
        DType::U64 => Dtype::U64,
        DType::F32 => Dtype::F32,
        DType::F64 => Dtype::F64,
    }
}

/// A loaded payload component. Each file that uses it gets an instance of its own.
pub struct WasmPayload {
    component: Component,
    linker: Linker<Host>,
    info: PluginInfo,
    limits: Limits,
}

impl std::fmt::Debug for WasmPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WasmPayload({})", self.info.name)
    }
}

fn instantiate(component: &Component, linker: &Linker<Host>, limits: Limits) -> Result<(Store<Host>, Payload), String> {
    let mut store = wasm::store(limits, None);
    wasm::arm(&mut store, limits);
    let p = Payload::instantiate(&mut store, component, linker).map_err(|e| wasm::describe_error(e, "starting"))?;
    Ok((store, p))
}

impl WasmPayload {
    pub fn load(path: &Path) -> Result<Self, String> {
        Self::load_with(path, Limits::DECODER)
    }

    pub fn load_with(path: &Path, limits: Limits) -> Result<Self, String> {
        let component = wasm::load(path)?;
        let mut linker = wasm::linker().map_err(|e| e.to_string())?;
        Payload::add_to_linker::<Host, HasSelf<Host>>(&mut linker, |h| h).map_err(|e| e.to_string())?;
        let (mut store, p) = instantiate(&component, &linker, limits)?;
        wasm::arm(&mut store, limits);
        let about = p.call_describe(&mut store).map_err(|e| wasm::describe_error(e, "describe"))?;
        let info = PluginInfo {
            name: about.name,
            version: about.version,
            tier: Tier::Wasm,
            description: about.description,
            files: about.files,
            path: PathBuf::from(path),
        };
        Ok(Self { component, linker, info, limits })
    }
}

impl PayloadPlugin for WasmPayload {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn session(&self) -> Result<Box<dyn PayloadSession>, String> {
        let (store, p) = instantiate(&self.component, &self.linker, self.limits)?;
        Ok(Box::new(Session { store, p, limits: self.limits }))
    }
}

struct Session {
    store: Store<Host>,
    p: Payload,
    limits: Limits,
}

impl PayloadSession for Session {
    fn packet_schema(&mut self, type_name: &str, params: &str) -> Result<PluginSchema, String> {
        wasm::arm(&mut self.store, self.limits);
        let s = self
            .p
            .call_packet_schema(&mut self.store, type_name, params)
            .map_err(|e| wasm::describe_error(e, "packet-schema"))??;
        Ok(PluginSchema {
            channels: s
                .channels
                .into_iter()
                .map(|c| PluginChannel { name: c.name, unit: c.unit, dtype: from_wit(c.dtype) })
                .collect(),
            timed: s.timed,
        })
    }

    fn decode_packets(
        &mut self,
        type_name: &str,
        params: &str,
        packets: &[(Vec<u8>, Vec<u8>)],
    ) -> Result<Chunk, String> {
        wasm::arm(&mut self.store, self.limits);
        let list: Vec<bindings::Packet> =
            packets.iter().map(|(h, p)| bindings::Packet { header: h.clone(), payload: p.clone() }).collect();
        let r = self
            .p
            .call_decode_packets(&mut self.store, type_name, params, &list)
            .map_err(|e| wasm::describe_error(e, "decoding"))?
            .map_err(|e| format!("can't decode these packets: {e}"))?;
        Ok(Chunk { rows: r.rows as usize, time: r.time, columns: r.columns })
    }

    fn decode_field(
        &mut self,
        field: &str,
        params: &str,
        dtype: DType,
        size: u32,
        count: u32,
        data: &[u8],
    ) -> Result<Vec<u8>, String> {
        wasm::arm(&mut self.store, self.limits);
        self.p
            .call_decode_field(&mut self.store, field, params, to_wit(dtype), size, count, data)
            .map_err(|e| wasm::describe_error(e, "decoding"))?
            .map_err(|e| format!("can't decode this field: {e}"))
    }
}
