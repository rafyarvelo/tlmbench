//! TelemetryBench decoders. Every format opens as a [`Recording`] the CLI and the app query the same way.
//!
//! - CSV and Parquet: decoded once, in parallel, into a columnar cache in the app cache dir (never beside the
//!   source), then read through memory maps.
//! - Fixed-width records: read in place from the memory-mapped file, laid out by a format [`descriptor`]. The
//!   tlmbench [`reference`] format carries its descriptor in its header; other files take one with
//!   `OpenOptions::descriptor`.
//! - Packet files: a descriptor with `[packets]` frames records in packets, compressed or not; a packet index is
//!   built once in the cache and packets are decompressed on demand (see [`packets`]).
//!
//! Nothing here writes next to the data: the only writes are cache files under the cache root.

mod bytes;
pub mod cache;
pub mod csv;
pub mod descriptor;
mod error;
pub mod fixed;
pub mod packets;
pub mod parquet;
mod parquet_pages;
pub mod reference;
mod strided;

use std::path::{Path, PathBuf};

pub use bytes::resident_share;
pub use error::{Error, Result};
use tlmbench_core::Recording;

#[derive(Clone, Debug, Default)]
pub struct OpenOptions {
    /// A format descriptor (TOML) for a fixed-width file without an embedded one.
    pub descriptor: Option<PathBuf>,
    /// The packet type to open, for a descriptor with `[[packet_types]]` (see [`open_all`]).
    pub part: Option<String>,
    /// Cache root; defaults to [`cache::cache_root`].
    pub cache_dir: Option<PathBuf>,
}

/// Opens a recording, choosing the decoder from the options and the file's first bytes. A packet file with several
/// packet types needs `opts.part` to say which (or use [`open_all`]).
pub fn open(path: &Path, opts: &OpenOptions) -> Result<Box<dyn Recording>> {
    let mut all = open_all(path, opts)?;
    if all.len() > 1 {
        let names: Vec<String> = all.iter().filter_map(|r| r.info().part.clone()).collect();
        return Err(Error::Descriptor(format!(
            "{} holds {} packet types ({}): say which one to open",
            path.display(),
            all.len(),
            names.join(", ")
        )));
    }
    Ok(all.pop().expect("open_all returns at least one recording"))
}

/// Opens every recording a file holds: one, or one per packet type for a descriptor with `[[packet_types]]`
/// (only the one `opts.part` names), in the descriptor's order.
pub fn open_all(path: &Path, opts: &OpenOptions) -> Result<Vec<Box<dyn Recording>>> {
    if let Some(desc) = &opts.descriptor {
        let d = descriptor::Descriptor::load(desc)?;
        if d.packets.is_some() {
            let root = cache::cache_root(opts.cache_dir.as_deref());
            let name = d.name.clone().unwrap_or_else(|| "packets".into());
            let mut parts = d.parts();
            if let Some(want) = &opts.part {
                parts.retain(|(_, q)| q.as_ref().is_some_and(|q| &q.name == want));
                if parts.is_empty() {
                    let types: Vec<&str> = d.packet_types.iter().map(|t| t.name.as_str()).collect();
                    return Err(Error::Descriptor(format!(
                        "{}: no packet type {want:?} (the types are {})",
                        desc.display(),
                        types.join(", ")
                    )));
                }
            }
            return parts
                .into_iter()
                .map(|(d, q)| Ok(Box::new(packets::PacketFile::open(path, d, q, &name, &root)?) as Box<dyn Recording>))
                .collect();
        }
        return Ok(vec![Box::new(fixed::FixedWidth::open_with_descriptor(path, desc)?)]);
    }
    let head = bytes::read_head(path, 4096)?;
    if head.starts_with(reference::MAGIC) {
        return Ok(vec![Box::new(fixed::FixedWidth::open_reference(path)?)]);
    }
    if parquet::looks_like_parquet(&head) {
        let root = cache::cache_root(opts.cache_dir.as_deref());
        return Ok(vec![Box::new(parquet::open_parquet(path, &root)?)]);
    }
    if csv::looks_like_csv(path, &head) {
        let root = cache::cache_root(opts.cache_dir.as_deref());
        return Ok(vec![Box::new(cache::open_csv(path, &root)?)]);
    }
    Err(Error::UnknownFormat(path.to_owned()))
}
