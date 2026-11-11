//! Decoder plugins: formats decoded by code that isn't part of tlmbench (see the design's "Decoder plugins").
//!
//! This module is the seam. It defines what a plugin is to the decoders ([`DecoderPlugin`], [`DecoderSession`]), keeps
//! the list of loaded plugins, and turns a plugin's column chunks into a cache entry like CSV and Parquet get, so the
//! store, pyramid and views never know which tier decoded a file. The plugins themselves (WASM components and native
//! libraries) live in the `tlmbench-plugin` crate, which registers them here.
//!
//! A plugin is asked in this order: `sniff` (the first bytes and the file name), then `open` (the schema), then
//! `next` until it returns no rows. Each chunk carries the sample times (nanoseconds, absent for index time) and one
//! little-endian array per channel, in the channel's type.

use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::{Arc, RwLock};

use serde::Serialize;

use crate::cache::{self, CachedRecording, MANIFEST_VERSION, Manifest, ManifestChannel, Source};
use crate::descriptor::DType;
use crate::error::{Result, format_err, io_err};

/// How much a plugin is trusted: it decides what the code can reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// In the wasmtime sandbox: no files, network or clock beyond the bytes the host hands over.
    Wasm,
    /// A shared library running in the process, with the user's own rights.
    Native,
}

/// What Settings lists about a plugin.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PluginInfo {
    /// The name its code gives (a file name stem when it gives none).
    pub name: String,
    pub version: String,
    pub tier: Tier,
    pub description: Option<String>,
    /// File name patterns it decodes (`*` matches any text), besides what `sniff` claims.
    pub files: Vec<String>,
    /// Where it was loaded from.
    pub path: std::path::PathBuf,
}

/// One channel of a plugin's schema.
#[derive(Clone, Debug, PartialEq)]
pub struct PluginChannel {
    pub name: String,
    pub unit: Option<String>,
    pub dtype: DType,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PluginSchema {
    pub channels: Vec<PluginChannel>,
    /// Chunks carry a time column in nanoseconds (false: samples are numbered).
    pub timed: bool,
}

/// Some rows: `time` is empty for index time, and `columns` has one little-endian array per channel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Chunk {
    pub rows: usize,
    pub time: Vec<i64>,
    pub columns: Vec<Vec<u8>>,
}

/// A plugin reading one file.
pub trait DecoderSession: Send {
    fn schema(&self) -> &PluginSchema;
    /// The next at most `max_rows` rows; `None` at the end.
    fn next(&mut self, max_rows: u32) -> std::result::Result<Option<Chunk>, String>;
}

pub trait DecoderPlugin: Send + Sync + std::fmt::Debug {
    fn info(&self) -> &PluginInfo;
    /// Whether this plugin decodes a file with these first bytes and this name.
    fn sniff(&self, head: &[u8], name: &str) -> bool;
    fn open(&self, path: &Path) -> std::result::Result<Box<dyn DecoderSession>, String>;
}

static PLUGINS: RwLock<Vec<Arc<dyn DecoderPlugin>>> = RwLock::new(Vec::new());

/// Replaces the list of decoder plugins (the app does this when its plugins folders are read).
pub fn set_plugins(list: Vec<Arc<dyn DecoderPlugin>>) {
    *PLUGINS.write().unwrap() = list;
}

pub fn plugins() -> Vec<Arc<dyn DecoderPlugin>> {
    PLUGINS.read().unwrap().clone()
}

/// The first plugin that claims `path`: a name pattern, or its own sniff.
pub fn find(path: &Path, head: &[u8]) -> Option<Arc<dyn DecoderPlugin>> {
    let name = path.file_name()?.to_string_lossy();
    plugins()
        .into_iter()
        .find(|p| p.info().files.iter().any(|pat| crate::descriptor::name_matches(pat, &name)) || p.sniff(head, &name))
}

/// Rows asked for at a time: a chunk of 64k rows of 8-byte values is 512 KB a channel.
const CHUNK_ROWS: u32 = 65_536;

/// Opens `path` through `plugin`, decoding it into the cache under `root` first if it isn't there.
pub fn open_with(path: &Path, root: &Path, plugin: &dyn DecoderPlugin) -> Result<CachedRecording> {
    let source = Source::open(path)?;
    let info = plugin.info();
    let key = blake3::hash(format!("{}\0{}\0{}", source.fingerprint()?, info.name, info.version).as_bytes());
    let dir = root.join("plugin").join(&key.to_hex().as_str()[..32]);
    let manifest = match cache::read_manifest(&dir) {
        Some(m) => m,
        None => cache::build_entry(&dir, |tmp| build(path, &source, plugin, tmp))?,
    };
    cache::load(path, &dir, manifest, &format!("{} (plugin)", info.name))
}

fn build(path: &Path, source: &Source, plugin: &dyn DecoderPlugin, tmp: &Path) -> Result<Manifest> {
    let fail = |msg: String| format_err(path, format!("{}: {msg}", plugin.info().name));
    let mut session = plugin.open(path).map_err(fail)?;
    let schema = session.schema().clone();
    let size = |c: &PluginChannel| c.dtype.size() as usize;
    let mut seen = std::collections::HashSet::new();
    for c in &schema.channels {
        if c.name.is_empty() || !seen.insert(c.name.as_str()) {
            return Err(fail(format!("channel names must be unique and not empty (\"{}\")", c.name)));
        }
    }
    let create = |name: &str| -> Result<BufWriter<std::fs::File>> {
        Ok(BufWriter::new(std::fs::File::create(tmp.join(name)).map_err(io_err(tmp))?))
    };
    let mut t_out = if schema.timed { Some(create("t.i64")?) } else { None };
    let names: Vec<String> =
        (0..schema.channels.len()).map(|k| format!("c{k}.{}", schema.channels[k].dtype.name())).collect();
    let mut outs = names.iter().map(|n| create(n)).collect::<Result<Vec<_>>>()?;
    let (mut rows, mut t_first, mut t_last) = (0usize, None, None::<i64>);
    while let Some(chunk) = session.next(CHUNK_ROWS).map_err(fail)? {
        if chunk.rows == 0 {
            break;
        }
        if chunk.columns.len() != schema.channels.len() {
            return Err(fail(format!(
                "a chunk has {} columns, the schema {}",
                chunk.columns.len(),
                schema.channels.len()
            )));
        }
        for (c, col) in schema.channels.iter().zip(&chunk.columns) {
            if col.len() != chunk.rows * size(c) {
                return Err(fail(format!(
                    "column \"{}\" has {} bytes for {} rows of {}",
                    c.name,
                    col.len(),
                    chunk.rows,
                    c.dtype.name()
                )));
            }
        }
        if schema.timed {
            if chunk.time.len() != chunk.rows {
                return Err(fail(format!("a chunk has {} times for {} rows", chunk.time.len(), chunk.rows)));
            }
            let backwards = chunk.time.windows(2).position(|w| w[1] < w[0]).map(|i| rows + i + 2);
            let seam = t_last.zip(chunk.time.first()).filter(|&(p, f)| *f < p).map(|_| rows + 1);
            if let Some(row) = backwards.or(seam) {
                return Err(fail(format!("time goes backwards at row {row}")));
            }
            t_first = t_first.or(chunk.time.first().copied());
            t_last = chunk.time.last().copied().or(t_last);
            t_out.as_mut().unwrap().write_all(bytemuck::cast_slice(&chunk.time)).map_err(io_err(tmp))?;
        }
        for (w, col) in outs.iter_mut().zip(&chunk.columns) {
            w.write_all(col).map_err(io_err(tmp))?;
        }
        rows += chunk.rows;
    }
    for mut w in t_out.into_iter().chain(outs) {
        w.flush().map_err(io_err(tmp))?;
    }
    Ok(Manifest {
        version: MANIFEST_VERSION,
        source_size: source.len,
        rows,
        skipped_rows: 0,
        time: schema.timed.then(|| "time".to_string()),
        channels: schema
            .channels
            .iter()
            .zip(names)
            .map(|(c, file)| ManifestChannel {
                name: c.name.clone(),
                unit: c.unit.clone(),
                category: None,
                file,
                dtype: c.dtype,
            })
            .collect(),
        skipped_columns: Vec::new(),
        t_first,
        t_last,
        rate: None,
    })
}
