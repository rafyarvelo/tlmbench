//! A check of a Parquet file's page headers before the `parquet` crate decodes it: that crate sizes buffers from
//! what a header claims (a dictionary of 800M values in a 2 KB file, found by fuzzing), so a damaged file could make
//! it allocate gigabytes. Each page's header is read (Thrift compact encoding) and the file is refused when a page
//! claims more than 1 GiB, or a dictionary more values than its bytes can hold.

use std::fs::File;

use parquet::basic::Type as Physical;
use parquet::file::metadata::ColumnChunkMetaData;

use crate::bytes::read_exact_at;

/// The most a page may hold, uncompressed.
const MAX_PAGE: i64 = 1 << 30;

/// Checks every page of a column chunk; the error says what's wrong.
pub(crate) fn check_chunk(file: &File, len: u64, col: &ColumnChunkMetaData) -> Result<(), String> {
    let (start, size) = col.byte_range();
    let end = start.checked_add(size).filter(|&e| e <= len).ok_or("a column chunk lies outside the file")?;
    // the narrowest a dictionary value can be, in bytes (byte arrays carry a 4-byte length)
    let width = match col.column_type() {
        Physical::BOOLEAN => 0,
        Physical::INT32 | Physical::FLOAT | Physical::BYTE_ARRAY => 4,
        Physical::INT64 | Physical::DOUBLE => 8,
        Physical::INT96 => 12,
        Physical::FIXED_LEN_BYTE_ARRAY => i64::from(col.column_descr().type_length().max(0)),
    };
    let mut at = start;
    while at < end {
        let mut want = 256usize;
        let header = loop {
            let n = want.min((len - at) as usize);
            let mut buf = vec![0; n];
            read_exact_at(file, &mut buf, at).map_err(|e| e.to_string())?;
            match PageHeader::parse(&buf) {
                Ok(h) => break h,
                Err(Short) if n == want && want < 1 << 20 => want *= 8,
                Err(_) => return Err("a page header can't be read".into()),
            }
        };
        if !(0..=MAX_PAGE).contains(&header.uncompressed) || header.compressed < 0 {
            return Err(format!("a page claims {} bytes", header.uncompressed));
        }
        if let Some(n) = header.dict_values
            && (n < 0 || n.saturating_mul(width) > header.uncompressed)
        {
            return Err(format!("a dictionary claims {n} values in {} bytes", header.uncompressed));
        }
        at = at.saturating_add(header.len as u64).saturating_add(header.compressed as u64);
    }
    Ok(())
}

/// The fields of a page header that bound what decoding it allocates.
#[derive(Debug, PartialEq)]
struct PageHeader {
    uncompressed: i64,
    compressed: i64,
    /// A dictionary page's value count.
    dict_values: Option<i64>,
    /// Bytes the header takes.
    len: usize,
}

/// The buffer ended inside the header.
#[derive(Debug, PartialEq)]
struct Short;

/// A Thrift compact-protocol reader, just enough to skip what isn't needed.
struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> Result<u8, Short> {
        let v = *self.b.get(self.p).ok_or(Short)?;
        self.p += 1;
        Ok(v)
    }

    fn varint(&mut self) -> Result<u64, Short> {
        let mut v = 0u64;
        for shift in (0..70).step_by(7) {
            let b = self.byte()?;
            v |= u64::from(b & 0x7f).checked_shl(shift).unwrap_or(0);
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Ok(v) // over-long: the value is garbage, and the checks refuse it
    }

    fn int(&mut self) -> Result<i64, Short> {
        let v = self.varint()?;
        Ok((v >> 1) as i64 ^ -((v & 1) as i64))
    }

    fn skip_bytes(&mut self, n: u64) -> Result<(), Short> {
        let end = (self.p as u64).checked_add(n).filter(|&e| e <= self.b.len() as u64).ok_or(Short)?;
        self.p = end as usize;
        Ok(())
    }

    /// Bytes left.
    fn left(&self) -> u64 {
        (self.b.len() - self.p) as u64
    }

    /// Skips a collection element of compact type `t` (booleans take a byte there).
    fn skip_element(&mut self, t: u8, depth: u32) -> Result<(), Short> {
        if t == 1 || t == 2 { self.byte().map(drop) } else { self.skip(t, depth) }
    }

    /// Skips a value of compact type `t`.
    fn skip(&mut self, t: u8, depth: u32) -> Result<(), Short> {
        if depth > 32 {
            return Err(Short); // nesting this deep is garbage; reading more won't help, and the caller gives up
        }
        match t {
            1 | 2 => Ok(()),
            3 => self.byte().map(drop),
            4..=6 => self.varint().map(drop),
            7 => self.skip_bytes(8),
            8 => {
                let n = self.varint()?;
                self.skip_bytes(n)
            }
            9 | 10 => {
                let h = self.byte()?;
                let n = if h >> 4 == 15 { self.varint()? } else { u64::from(h >> 4) };
                // every element takes a byte at least, so a longer count can't fit what's left
                if n > self.left() {
                    return Err(Short);
                }
                for _ in 0..n {
                    self.skip_element(h & 0x0f, depth + 1)?;
                }
                Ok(())
            }
            11 => {
                let n = self.varint()?;
                if n > self.left() {
                    return Err(Short);
                }
                if n > 0 {
                    let kv = self.byte()?;
                    for _ in 0..n {
                        self.skip_element(kv >> 4, depth + 1)?;
                        self.skip_element(kv & 0x0f, depth + 1)?;
                    }
                }
                Ok(())
            }
            12 => self.fields(depth + 1, |r, _, t| r.skip(t, depth + 1)),
            _ => Err(Short),
        }
    }

    /// Reads a struct's fields up to its stop byte, calling `f(reader, id, type)` for each.
    fn fields(&mut self, depth: u32, mut f: impl FnMut(&mut Self, i16, u8) -> Result<(), Short>) -> Result<(), Short> {
        let mut id = 0i16;
        loop {
            let h = self.byte()?;
            if h == 0 {
                return Ok(());
            }
            let delta = h >> 4;
            id = if delta == 0 { self.int()? as i16 } else { id.wrapping_add(i16::from(delta)) };
            f(self, id, h & 0x0f)?;
            if depth > 32 {
                return Err(Short);
            }
        }
    }
}

impl PageHeader {
    fn parse(b: &[u8]) -> Result<Self, Short> {
        let mut r = Reader { b, p: 0 };
        let (mut uncompressed, mut compressed, mut dict_values) = (None, None, None);
        r.fields(0, |r, id, t| match (id, t) {
            (2, 5) => r.int().map(|v| uncompressed = Some(v)),
            (3, 5) => r.int().map(|v| compressed = Some(v)),
            (7, 12) => r.fields(1, |r, id, t| match (id, t) {
                (1, 5) => r.int().map(|v| dict_values = Some(v)),
                _ => r.skip(t, 2),
            }),
            _ => r.skip(t, 1),
        })?;
        Ok(Self {
            uncompressed: uncompressed.unwrap_or(-1),
            compressed: compressed.unwrap_or(-1),
            dict_values,
            len: r.p,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_dictionary_page_header() {
        // type 2 (dictionary), 100 bytes, 40 compressed, crc skipped, dictionary header { 25 values, encoding 0 }
        let b = [0x15, 0x04, 0x15, 0xc8, 0x01, 0x15, 0x50, 0x15, 0x02, 0x3c, 0x15, 0x32, 0x15, 0x00, 0x00, 0x00];
        let h = PageHeader::parse(&b).unwrap();
        assert_eq!(h, PageHeader { uncompressed: 100, compressed: 40, dict_values: Some(25), len: 16 });
        assert_eq!(PageHeader::parse(&b[..9]), Err(Short));
    }

    #[test]
    fn a_huge_collection_of_nothing_ends_at_once() {
        // field 1 a map of 2^62 bool pairs, in 12 bytes: once looped forever
        let b = [0x1b, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x40, 0x11, 0x00];
        assert_eq!(PageHeader::parse(&b), Err(Short));
    }
}
