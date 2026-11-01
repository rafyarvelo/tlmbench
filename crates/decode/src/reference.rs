//! The tlmbench reference format: fixed-width records behind a small header that embeds the file's own
//! descriptor, so a file opens with no extra configuration and every open still goes through the descriptor
//! parser. It is the reference for the descriptor schema (the design's "one custom binary format we define").
//!
//! Layout (little-endian):
//!
//! | Offset | Size | Field |
//! | --- | --- | --- |
//! | 0 | 8 | magic `TLMBENCH` |
//! | 8 | 2 | format version (1) |
//! | 10 | 2 | flags (0) |
//! | 12 | 4 | header length: bytes before the first record, a multiple of 64 |
//! | 16 | 4 | descriptor length in bytes |
//! | 20 | 4 | reserved (0) |
//! | 24 | n | descriptor (UTF-8 TOML), zero-padded to the header length |
//!
//! Records follow the header back to back; a trailing partial record is ignored. The descriptor's
//! `header_size` is ignored: the header length above wins.

use std::io::{self, Write};

use crate::descriptor::Descriptor;
use crate::error::{Error, Result};

pub const MAGIC: &[u8; 8] = b"TLMBENCH";
pub const VERSION: u16 = 1;
/// File extension the generator and exports use.
pub const EXTENSION: &str = "tlm";
const FIXED: usize = 24;
const ALIGN: usize = 64;

/// Writes the header for `descriptor` and returns its length (where the first record starts).
pub fn write_header(w: &mut impl Write, descriptor: &Descriptor) -> io::Result<usize> {
    let text = descriptor.to_toml();
    let header_len = (FIXED + text.len()).div_ceil(ALIGN) * ALIGN;
    let mut h = Vec::with_capacity(header_len);
    h.extend_from_slice(MAGIC);
    h.extend_from_slice(&VERSION.to_le_bytes());
    h.extend_from_slice(&0u16.to_le_bytes());
    h.extend_from_slice(&(header_len as u32).to_le_bytes());
    h.extend_from_slice(&(text.len() as u32).to_le_bytes());
    h.extend_from_slice(&0u32.to_le_bytes());
    h.extend_from_slice(text.as_bytes());
    h.resize(header_len, 0);
    w.write_all(&h)?;
    Ok(header_len)
}

/// Reads the header: where the records start, and the descriptor.
pub fn read_header(bytes: &[u8]) -> Result<(usize, Descriptor)> {
    let bad = |msg: &str| Err(Error::Descriptor(format!("reference header: {msg}")));
    if bytes.len() < FIXED || &bytes[..8] != MAGIC {
        return bad("not a tlmbench reference file");
    }
    let u16_at = |o: usize| u16::from_le_bytes([bytes[o], bytes[o + 1]]);
    let u32_at = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap()) as usize;
    let version = u16_at(8);
    if version != VERSION {
        return bad(&format!("version {version} is not supported (this version reads {VERSION})"));
    }
    let (header_len, desc_len) = (u32_at(12), u32_at(16));
    if header_len % ALIGN != 0 || header_len < FIXED + desc_len || header_len > bytes.len() {
        return bad(&format!("header length {header_len} doesn't fit a {desc_len}-byte descriptor in this file"));
    }
    let Ok(text) = std::str::from_utf8(&bytes[FIXED..FIXED + desc_len]) else {
        return bad("descriptor is not UTF-8");
    };
    Ok((header_len, Descriptor::parse(text)?))
}
