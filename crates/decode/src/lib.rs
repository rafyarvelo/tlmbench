//! TelemetryBench decoders. Every format opens as a [`Recording`] the CLI and the app query the same way.
//!
//! - CSV and Parquet: decoded once, in parallel, into a columnar cache in the app cache dir (never beside the
//!   source), then read through memory maps.
//! - Fixed-width records: read in place from the memory-mapped file, laid out by a format [`descriptor`]. The
//!   tlmbench [`reference`] format carries its descriptor in its header; other files take one with
//!   `OpenOptions::descriptor`.
//!
//! Nothing here writes next to the data: the only writes are cache files under the cache root.

mod bytes;
pub mod cache;
pub mod csv;
pub mod descriptor;
mod error;
pub mod fixed;
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
    /// Cache root; defaults to [`cache::cache_root`].
    pub cache_dir: Option<PathBuf>,
}

/// Opens a recording, choosing the decoder from the options and the file's first bytes.
pub fn open(path: &Path, opts: &OpenOptions) -> Result<Box<dyn Recording>> {
    if let Some(desc) = &opts.descriptor {
        return Ok(Box::new(fixed::FixedWidth::open_with_descriptor(path, desc)?));
    }
    let head = bytes::read_head(path, 4096)?;
    if head.starts_with(reference::MAGIC) {
        return Ok(Box::new(fixed::FixedWidth::open_reference(path)?));
    }
    if parquet::looks_like_parquet(&head) {
        let root = cache::cache_root(opts.cache_dir.as_deref());
        return Ok(Box::new(parquet::open_parquet(path, &root)?));
    }
    if csv::looks_like_csv(path, &head) {
        let root = cache::cache_root(opts.cache_dir.as_deref());
        return Ok(Box::new(cache::open_csv(path, &root)?));
    }
    Err(Error::UnknownFormat(path.to_owned()))
}
