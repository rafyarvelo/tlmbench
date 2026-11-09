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
pub mod preview;
pub mod profile;
pub mod reference;
mod strided;

use std::path::{Path, PathBuf};

pub use bytes::resident_share;

/// The first `n` bytes of a file (fewer if it is shorter).
pub fn read_head(path: &Path, n: usize) -> Result<Vec<u8>> {
    bytes::read_head(path, n)
}
pub use error::{Error, Result};
use tlmbench_core::adjust::{Adjusted, ChannelAdjust};
use tlmbench_core::{Channels, Recording};

#[derive(Clone, Debug, Default)]
pub struct OpenOptions {
    /// A format descriptor (TOML) for a fixed-width file without an embedded one.
    pub descriptor: Option<PathBuf>,
    /// The packet type to open, for a descriptor with `[[packet_types]]` (see [`open_all`]).
    pub part: Option<String>,
    /// An import profile for a CSV file (ignored by other formats).
    pub profile: Option<profile::ImportProfile>,
    /// Cache root; defaults to [`cache::cache_root`].
    pub cache_dir: Option<PathBuf>,
    /// A person's scale, offset and missing values by channel name (see [`tlmbench_core::adjust`]): applied inside
    /// the reads, and part of the layout id so a pyramid is built for them.
    pub adjust: Vec<(String, ChannelAdjust)>,
}

/// `c` as a recording, with `opts`' adjustments applied when there are any.
fn boxed<C: Channels + 'static>(c: C, opts: &OpenOptions) -> Box<dyn Recording> {
    if opts.adjust.iter().all(|(_, a)| a.is_identity()) { Box::new(c) } else { Box::new(Adjusted::new(c, &opts.adjust)) }
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
                .map(|(d, q)| Ok(boxed(packets::PacketFile::open(path, d, q, &name, &root)?, opts)))
                .collect();
        }
        return Ok(vec![boxed(fixed::FixedWidth::open_with_descriptor(path, desc)?, opts)]);
    }
    let head = bytes::read_head(path, 4096)?;
    if head.starts_with(reference::MAGIC) {
        return Ok(vec![boxed(fixed::FixedWidth::open_reference(path)?, opts)]);
    }
    // a descriptor whose `files` patterns name this file (in the folder's .tlmbench/decoders or the app cache)
    let root = cache::cache_root(opts.cache_dir.as_deref());
    if let Some(desc) = descriptor::discover(path, &root) {
        return open_all(path, &OpenOptions { descriptor: Some(desc), ..opts.clone() });
    }
    if parquet::looks_like_parquet(&head) {
        let root = cache::cache_root(opts.cache_dir.as_deref());
        return Ok(vec![boxed(parquet::open_parquet(path, &root)?, opts)]);
    }
    if csv::looks_like_csv(path, &head) {
        let root = cache::cache_root(opts.cache_dir.as_deref());
        return Ok(vec![boxed(cache::open_csv_with(path, &root, opts.profile.as_ref())?, opts)]);
    }
    Err(Error::UnknownFormat(path.to_owned()))
}
