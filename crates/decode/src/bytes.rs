//! A file's bytes: memory-mapped, or owned (empty files, and data built in memory for tests and fuzzing).

use std::fs::File;
use std::io::Read;
use std::ops::Deref;
use std::path::Path;

use memmap2::Mmap;

use crate::error::{Result, io_err};

#[derive(Debug)]
pub(crate) enum Bytes {
    Mapped(Mmap),
    Owned(Vec<u8>),
}

impl Deref for Bytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            Bytes::Mapped(m) => m,
            Bytes::Owned(v) => v,
        }
    }
}

/// Maps a file read-only. Mapping is page-aligned, so typed views of it are aligned too.
pub(crate) fn map(path: &Path) -> Result<Bytes> {
    let file = File::open(path).map_err(io_err(path))?;
    let len = file.metadata().map_err(io_err(path))?.len();
    if len == 0 {
        return Ok(Bytes::Owned(Vec::new()));
    }
    // SAFETY: the map is read-only. Another process truncating the file while it is open would fault; that is
    // the usual trade-off of reading recordings through mmap, and recordings are not edited in place.
    let map = unsafe { Mmap::map(&file) }.map_err(io_err(path))?;
    Ok(Bytes::Mapped(map))
}

/// The first `n` bytes of a file (fewer if it is shorter).
pub(crate) fn read_head(path: &Path, n: usize) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(n);
    File::open(path).and_then(|f| f.take(n as u64).read_to_end(&mut buf)).map_err(io_err(path))?;
    Ok(buf)
}
