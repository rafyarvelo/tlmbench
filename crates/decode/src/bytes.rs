//! A file's bytes: memory-mapped, or owned (empty files, and data built in memory for tests and fuzzing).

use std::fs::File;
use std::io::{self, Read};
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

impl Bytes {
    /// Hints that the bytes will be read front to back, so the OS reads ahead (a no-op where unsupported).
    pub(crate) fn advise_sequential(&self) {
        #[cfg(unix)]
        if let Bytes::Mapped(m) = self {
            let _ = m.advise(memmap2::Advice::Sequential);
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

/// Reads exactly `buf.len()` bytes at `at`.
#[cfg(unix)]
pub(crate) fn read_exact_at(file: &File, buf: &mut [u8], at: u64) -> io::Result<()> {
    std::os::unix::fs::FileExt::read_exact_at(file, buf, at)
}

#[cfg(windows)]
pub(crate) fn read_exact_at(file: &File, mut buf: &mut [u8], mut at: u64) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buf.is_empty() {
        match file.seek_read(buf, at) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => {
                buf = &mut buf[n..];
                at += n as u64;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
