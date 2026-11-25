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

    // -- files written with the `parquet` crate's options, and Thrift headers written by hand --

    use std::io::Write;

    use std::sync::Arc;
    use tlmbench_core::Recording;

    use arrow_array::types::{ArrowPrimitiveType, Float16Type};
    use arrow_array::{
        ArrayRef, BooleanArray, Float16Array, Float32Array, Float64Array, Int32Array, Int64Array, RecordBatch,
        StringArray,
    };
    use arrow_schema::{Field, Schema};
    use parquet::arrow::ArrowWriter;
    use parquet::arrow::arrow_reader::{ArrowReaderMetadata, ArrowReaderOptions};
    use parquet::basic::Compression;
    use parquet::file::properties::{WriterProperties, WriterVersion};

    const ROWS: usize = 20_000;

    fn f16(x: f32) -> <Float16Type as ArrowPrimitiveType>::Native {
        <Float16Type as ArrowPrimitiveType>::Native::from_f32(x)
    }

    /// A Thrift compact-protocol writer for a struct's fields.
    struct Enc {
        b: Vec<u8>,
        last: i16,
    }

    impl Enc {
        fn new() -> Self {
            Enc { b: Vec::new(), last: 0 }
        }
        /// A field header: the id as a delta when it fits, else in full.
        fn field(&mut self, id: i16, t: u8) -> &mut Self {
            let d = id - self.last;
            if (1..=15).contains(&d) {
                self.b.push((d as u8) << 4 | t);
            } else {
                self.b.push(t);
                self.int(i64::from(id));
            }
            self.last = id;
            self
        }
        fn varint(&mut self, mut v: u64) -> &mut Self {
            loop {
                let x = (v & 0x7f) as u8;
                v >>= 7;
                if v == 0 {
                    self.b.push(x);
                    return self;
                }
                self.b.push(x | 0x80);
            }
        }
        fn int(&mut self, v: i64) -> &mut Self {
            self.varint(((v << 1) ^ (v >> 63)) as u64)
        }
        fn raw(&mut self, b: &[u8]) -> &mut Self {
            self.b.extend_from_slice(b);
            self
        }
        fn stop(&mut self) -> Vec<u8> {
            self.b.push(0);
            std::mem::take(&mut self.b)
        }
    }

    /// A page header: type, sizes and (for a dictionary page) a value count; `pad` one-byte boolean fields follow.
    fn page(unc: i64, comp: i64, dict: Option<i64>, pad: usize) -> Vec<u8> {
        let mut e = Enc::new();
        e.field(1, 5).int(if dict.is_some() { 2 } else { 0 });
        e.field(2, 5).int(unc);
        e.field(3, 5).int(comp);
        if let Some(n) = dict {
            let mut d = Enc::new();
            d.field(1, 5).int(n);
            d.field(2, 5).int(0);
            e.field(7, 12).raw(&d.stop());
        }
        for i in 0..pad {
            e.field(20 + i as i16, 1);
        }
        e.stop()
    }

    /// A header and what follows it, as the only thing in a column chunk of `size` bytes that starts at `start`.
    fn check_crafted(col: &ColumnChunkMetaData, bytes: &[u8], extra: usize) -> Result<(), String> {
        let (start, size) = col.byte_range();
        let len = start as usize + (size as usize).max(bytes.len()) + extra;
        let mut file = vec![0u8; len];
        file[start as usize..][..bytes.len()].copy_from_slice(bytes);
        let mut f = tempfile::tempfile().unwrap();
        f.write_all(&file).unwrap();
        // a chunk is only as long as the file holds
        check_chunk(&f, len as u64, col)
    }

    struct Written {
        _dir: tempfile::TempDir,
        path: std::path::PathBuf,
        file: File,
        chunks: Vec<ColumnChunkMetaData>,
    }

    fn open_chunks(dir: tempfile::TempDir, path: std::path::PathBuf) -> Written {
        let file = File::open(&path).unwrap();
        let meta = ArrowReaderMetadata::load(&file, ArrowReaderOptions::new()).unwrap();
        let chunks = meta.metadata().row_groups().iter().flat_map(|g| g.columns().iter().cloned()).collect();
        Written { _dir: dir, path, file, chunks }
    }

    /// Columns of every physical type a dictionary page can sit in, `ROWS` rows; "k" repeats 50 values (a dictionary).
    fn batch() -> RecordBatch {
        let n = ROWS as i32;
        let cols: Vec<(&str, ArrayRef)> = vec![
            ("k", Arc::new(Int32Array::from_iter_values((0..n).map(|i| i % 50)))),
            ("wide", Arc::new(Int64Array::from_iter_values((0..n).map(|i| i64::from(i % 7) << 33)))),
            ("f", Arc::new(Float32Array::from_iter_values((0..n).map(|i| (i % 9) as f32 * 0.5)))),
            ("d", Arc::new(Float64Array::from_iter_values((0..n).map(|i| (i % 11) as f64 * 0.25)))),
            ("flag", Arc::new(BooleanArray::from_iter((0..n).map(|i| Some(i % 3 == 0))))),
            ("s", Arc::new(StringArray::from_iter_values((0..n).map(|i| format!("label {}", i % 13))))),
            ("h", Arc::new(Float16Array::from_iter_values((0..n).map(|i| f16((i % 5) as f32))))),
        ];
        let schema = Arc::new(Schema::new(
            cols.iter().map(|(n, a)| Field::new(*n, a.data_type().clone(), false)).collect::<Vec<_>>(),
        ));
        RecordBatch::try_new(schema, cols.into_iter().map(|c| c.1).collect()).unwrap()
    }

    fn written(props: WriterProperties) -> Written {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.parquet");
        let b = batch();
        let mut w = ArrowWriter::try_new(File::create(&path).unwrap(), b.schema(), Some(props)).unwrap();
        w.write(&b).unwrap();
        w.close().unwrap();
        open_chunks(dir, path)
    }

    fn check_all(w: &Written) -> Result<(), String> {
        let len = w.file.metadata().unwrap().len();
        w.chunks.iter().try_for_each(|c| check_chunk(&w.file, len, c))
    }

    #[test]
    fn pages_of_every_compression_dictionary_and_version_pass() {
        use parquet::basic::{BrotliLevel, GzipLevel, ZstdLevel};
        let codecs = [
            Compression::UNCOMPRESSED,
            Compression::SNAPPY,
            Compression::GZIP(GzipLevel::default()),
            Compression::BROTLI(BrotliLevel::default()),
            Compression::ZSTD(ZstdLevel::default()),
            Compression::LZ4_RAW,
        ];
        for (i, codec) in codecs.into_iter().enumerate() {
            for dict in [true, false] {
                let version = if i % 2 == 0 { WriterVersion::PARQUET_1_0 } else { WriterVersion::PARQUET_2_0 };
                let props = WriterProperties::builder()
                    .set_compression(codec)
                    .set_dictionary_enabled(dict)
                    .set_writer_version(version)
                    .set_data_page_row_count_limit(1500)
                    .set_write_batch_size(500)
                    .set_max_row_group_row_count(Some(8000))
                    .build();
                let w = written(props);
                let what = format!("{codec:?}, dictionary {dict}, {version:?}");
                assert_eq!(w.chunks.len(), 3 * 7, "{what}: three row groups of seven columns");
                assert_eq!(w.chunks[0].dictionary_page_offset().is_some(), dict, "{what}");
                assert_eq!(check_all(&w), Ok(()), "{what}");
                // and the file opens with the values it was written with
                let cache = tempfile::tempdir().unwrap();
                let r = crate::parquet::open_parquet(&w.path, cache.path()).unwrap();
                assert_eq!(r.info().samples, ROWS, "{what}");
                let k = r.info().channels.iter().position(|c| c.name == "k").unwrap();
                let held = tlmbench_core::chunks::Held::default();
                assert_eq!([0, 49, 12345].map(|i| r.value_at(k, i, &held)), [0.0, 49.0, 45.0], "{what}");
            }
        }
    }

    #[test]
    fn a_dictionary_claiming_more_values_than_its_bytes_hold_is_refused() {
        let w = written(WriterProperties::builder().build());
        let unc = 64;
        for col in &w.chunks[..7] {
            // the narrowest value, in bytes: the page's own count of values at that width just fits
            let width = match col.column_type() {
                Physical::BOOLEAN => 0,
                Physical::INT32 | Physical::FLOAT | Physical::BYTE_ARRAY => 4,
                Physical::INT64 | Physical::DOUBLE => 8,
                Physical::FIXED_LEN_BYTE_ARRAY => 2,
                Physical::INT96 => unreachable!(),
            };
            let (_, size) = col.byte_range();
            let name = col.column_path().string();
            let claim = |n: i64| check_crafted(col, &page(unc, size as i64, Some(n), 0), 0);
            if width == 0 {
                assert_eq!(claim(3), Ok(()), "{name}");
                continue;
            }
            assert_eq!(claim(unc / width), Ok(()), "{name}");
            let err = claim(unc / width + 1).unwrap_err();
            assert_eq!(err, format!("a dictionary claims {} values in 64 bytes", unc / width + 1), "{name}");
            assert!(claim(-1).unwrap_err().starts_with("a dictionary claims -1 values"), "{name}");
            assert!(claim(i64::MAX).unwrap_err().contains("a dictionary claims"), "{name}: no overflow");
        }
    }

    #[test]
    fn an_int96_column_takes_twelve_bytes_a_value() {
        use parquet::data_type::{Int96, Int96Type};
        use parquet::file::writer::SerializedFileWriter;
        use parquet::schema::parser::parse_message_type;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t96.parquet");
        let schema = Arc::new(parse_message_type("message m { required int96 t; }").unwrap());
        let mut w = SerializedFileWriter::new(
            File::create(&path).unwrap(),
            schema,
            Arc::new(WriterProperties::builder().build()),
        )
        .unwrap();
        let mut rg = w.next_row_group().unwrap();
        let mut col = rg.next_column().unwrap().unwrap();
        let values: Vec<Int96> = (0..10u32).map(|i| Int96::from(vec![i, 0, 2_440_588])).collect();
        col.typed::<Int96Type>().write_batch(&values, None, None).unwrap();
        col.close().unwrap();
        rg.close().unwrap();
        w.close().unwrap();
        let w = open_chunks(dir, path);
        assert_eq!(w.chunks[0].column_type(), Physical::INT96);
        assert_eq!(check_all(&w), Ok(()));
        let (_, size) = w.chunks[0].byte_range();
        let claim = |n: i64| check_crafted(&w.chunks[0], &page(120, size as i64, Some(n), 0), 0);
        assert_eq!(claim(10), Ok(()));
        assert_eq!(claim(11).unwrap_err(), "a dictionary claims 11 values in 120 bytes");
    }

    #[test]
    fn a_page_claiming_too_much_is_refused() {
        let w = written(WriterProperties::builder().build());
        let col = &w.chunks[0];
        let (_, size) = col.byte_range();
        let size = size as i64;
        assert_eq!(check_crafted(col, &page(1 << 30, size, None, 0), 0), Ok(()), "a gigabyte is the limit");
        assert_eq!(
            check_crafted(col, &page((1 << 30) + 1, size, None, 0), 0).unwrap_err(),
            "a page claims 1073741825 bytes"
        );
        assert_eq!(check_crafted(col, &page(-5, size, None, 0), 0).unwrap_err(), "a page claims -5 bytes");
        assert_eq!(check_crafted(col, &page(100, -1, None, 0), 0).unwrap_err(), "a page claims 100 bytes");
        // no sizes at all
        assert_eq!(check_crafted(col, &[0], 0).unwrap_err(), "a page claims -1 bytes");
        // the walk goes page to page: a good page, its 10 bytes, then a bad header
        let mut two = page(10, 10, None, 0);
        two.extend_from_slice(&[0; 10]);
        two.extend_from_slice(&page(i64::from(i32::MAX) * 4, 0, None, 0));
        assert_eq!(check_crafted(col, &two, 0).unwrap_err(), "a page claims 8589934588 bytes");
    }

    #[test]
    fn a_chunk_outside_the_file_or_a_header_that_cant_be_read_is_refused() {
        let w = written(WriterProperties::builder().build());
        let col = &w.chunks[0];
        let (start, size) = col.byte_range();
        assert_eq!(check_chunk(&w.file, start + size - 1, col).unwrap_err(), "a column chunk lies outside the file");
        assert_eq!(check_chunk(&w.file, 0, col).unwrap_err(), "a column chunk lies outside the file");
        // the file is shorter than the length it was said to have: the read fails
        let short = tempfile::tempfile().unwrap();
        let len = w.file.metadata().unwrap().len();
        let err = check_chunk(&short, len, col).unwrap_err();
        assert!(!err.contains("claims") && !err.contains("page header"), "a failed read, not a verdict: {err}");
        // not Thrift: a type no one defines, and a header cut off
        assert_eq!(check_crafted(col, &[0xff; 40], 0).unwrap_err(), "a page header can't be read");
        // a header over 256 bytes is read again with a bigger buffer; over a megabyte it is refused
        let big = |n: usize| {
            let mut e = Enc::new();
            e.field(1, 5).int(0).field(2, 5).int(10).field(3, 5).int(size as i64).field(10, 8).varint(n as u64);
            e.raw(&vec![7u8; n]);
            e.stop()
        };
        let (b, huge) = (big(3000), big(2 << 20));
        assert_eq!(check_crafted(col, &b, 0), Ok(()), "a 3 KB header");
        assert_eq!(check_crafted(col, &huge, 3 << 20).unwrap_err(), "a page header can't be read");
    }

    /// A damaged file is refused with an error that names Parquet, or opens whole: it never panics out of
    /// `open_parquet`, hangs or asks for the memory a header claims.
    #[test]
    fn a_damaged_file_is_refused_or_opens_whole() {
        let w = written(
            WriterProperties::builder()
                .set_compression(Compression::SNAPPY)
                .set_writer_version(WriterVersion::PARQUET_2_0)
                .set_data_page_row_count_limit(4000)
                .build(),
        );
        let bytes = std::fs::read(&w.path).unwrap();
        let cache = tempfile::tempdir().unwrap();
        let open = |name: &str, b: &[u8]| {
            let p = cache.path().join(name);
            std::fs::write(&p, b).unwrap();
            crate::parquet::open_parquet(&p, &cache.path().join("c"))
                .map(|r| r.info().samples)
                .map_err(|e| e.to_string())
        };
        assert_eq!(open("good.parquet", &bytes), Ok(ROWS));
        // cut anywhere: the footer is at the end, so every cut loses it
        for cut in [4, 5, 100, bytes.len() / 2, bytes.len() - 9, bytes.len() - 1] {
            let err = open(&format!("cut{cut}.parquet"), &bytes[..cut]).unwrap_err();
            assert!(err.contains("Parquet"), "cut at {cut}: {err}");
        }
        // every bit of one byte flipped, over the first page header of each column
        for (c, col) in w.chunks.iter().take(7).enumerate() {
            let (start, _) = col.byte_range();
            for at in start as usize..start as usize + 40 {
                let mut b = bytes.clone();
                b[at] ^= 0xff;
                match open(&format!("flip{c}_{at}.parquet"), &b) {
                    Ok(n) => assert_eq!(n, ROWS, "column {c}, byte {at}"),
                    Err(e) => assert!(e.contains("Parquet"), "column {c}, byte {at}: {e}"),
                }
            }
        }
    }

    /// The same file with its first dictionary page's header rewritten to claim
    /// more values than its bytes can hold.
    #[test]
    fn a_dictionary_page_claiming_a_million_values_refuses_the_file() {
        let w = written(WriterProperties::builder().build());
        let mut bytes = std::fs::read(&w.path).unwrap();
        let col = &w.chunks[0];
        assert!(col.dictionary_page_offset().is_some());
        let (start, _) = col.byte_range();
        let start = start as usize;
        let old = PageHeader::parse(&bytes[start..start + 256]).unwrap();
        // the header is longer, so the later chunks' offsets move; the first chunk is checked first
        let new = page(old.uncompressed, old.compressed, Some(1_000_000), 0);
        bytes.splice(start..start + old.len, new);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claim.parquet");
        std::fs::write(&path, &bytes).unwrap();
        let err = crate::parquet::open_parquet(&path, &dir.path().join("c")).err().unwrap().to_string();
        assert!(
            err.contains(&format!("damaged: a dictionary claims 1000000 values in {} bytes", old.uncompressed)),
            "{err}"
        );
    }

    /// Fields a header doesn't use are skipped, whatever their type, and the ones after them still read.
    #[test]
    fn every_kind_of_unknown_field_is_skipped_exactly() {
        let mut nested = Enc::new();
        nested.field(1, 5).int(-3).field(2, 8).varint(2).raw(b"hi").field(3, 12).raw(&[0x15, 0x02, 0x00]);
        let nested = nested.stop();
        let long_list: Vec<u8> = [&[0xf1, 17][..], &[1; 17]].concat();
        // (compact type, the bytes after the field header)
        let skipped: Vec<(u8, Vec<u8>)> = vec![
            (1, vec![]),                                         // true
            (2, vec![]),                                         // false
            (3, vec![0x7f]),                                     // byte
            (4, vec![0x80, 0x01]),                               // i16
            (5, vec![0xff, 0xff, 0x03]),                         // i32
            (6, vec![0x80, 0x80, 0x01]),                         // i64
            (7, vec![1, 2, 3, 4, 5, 6, 7, 8]),                   // double
            (8, vec![3, b'a', b'b', b'c']),                      // binary
            (9, vec![0x35, 2, 4, 6]),                            // a list of three i32
            (9, long_list),                                      // a list of 17 booleans
            (9, vec![0x2c, 0x15, 0x02, 0x00, 0x15, 0x02, 0x00]), // a list of two structs
            (10, vec![0x15, 2]),                                 // a set of one i32
            (11, vec![2, 0x58, 2, 1, b'k', 4, 1, b'j']),         // a map of two i32 -> binary
            (11, vec![2, 0x11, 1, 0, 0, 1]),                     // a map of two bool -> bool
            (11, vec![0]),                                       // an empty map
            (12, vec![0]),                                       // an empty struct
            (12, nested),                                        // a struct with a struct in it
        ];
        let mut e = Enc::new();
        for (k, (t, body)) in skipped.iter().enumerate() {
            e.field(20 + k as i16, *t).raw(body);
        }
        e.field(2, 5).int(100).field(3, 5).int(40);
        let b = e.stop();
        assert_eq!(
            PageHeader::parse(&b),
            Ok(PageHeader { uncompressed: 100, compressed: 40, dict_values: None, len: b.len() })
        );
        // each cut short anywhere inside is "short", never a wrong header
        for cut in 0..b.len() {
            assert_eq!(PageHeader::parse(&b[..cut]), Err(Short), "cut at {cut}");
        }
        // a field's id written in full (a delta of 0), and a dictionary header with fields we don't use
        let mut d = Enc::new();
        d.field(1, 5).int(9).field(4, 8).varint(1).raw(b"z").field(9, 1);
        let mut e = Enc::new();
        e.field(2, 5).int(36).field(3, 5).int(36).field(7, 12).raw(&d.stop());
        let b = e.stop();
        assert_eq!(PageHeader::parse(&b).unwrap().dict_values, Some(9));
    }

    #[test]
    fn a_header_that_lies_about_its_own_size_is_short_not_trusted() {
        let field = |t: u8, body: &[u8]| Enc::new().field(30, t).raw(body).stop();
        for (what, t, body) in [
            ("a type no one defines", 13, &[0u8][..]),
            ("another", 15, &[0]),
            ("a binary longer than the header", 8, &[200, 1, 2]),
            ("a double cut short", 7, &[1, 2, 3]),
            ("a varint cut short", 5, &[0x80]),
            ("a list of 100 elements in 3 bytes", 9, &[0xf5, 100, 1, 2]),
            ("a set of 100 elements in 2 bytes", 10, &[0xf5, 100]),
            ("a map of 100 pairs in 2 bytes", 11, &[100, 0x55]),
            ("a map cut short", 11, &[1]),
        ] {
            assert_eq!(PageHeader::parse(&field(t, body)), Err(Short), "{what}");
        }
        // a varint with no end: ten bytes, all with their top bit, is garbage the sizes' checks refuse
        let mut e = Enc::new();
        e.field(2, 5).raw(&[0xff; 10]).field(3, 5).int(0);
        let h = PageHeader::parse(&e.stop()).unwrap();
        assert!(h.uncompressed < 0 || h.uncompressed > MAX_PAGE, "{h:?}");
    }

    #[test]
    fn nesting_past_32_levels_is_short() {
        // `levels` structs in one another, the innermost holding an i32
        let nest = |levels: usize| {
            let mut b = Enc::new().field(30, 12).b.clone();
            b.extend(std::iter::repeat_n(0x1c, levels - 1));
            b.extend([0x15, 0x02]);
            b.extend(std::iter::repeat_n(0, levels + 1));
            b
        };
        assert!(PageHeader::parse(&nest(31)).is_ok());
        assert_eq!(PageHeader::parse(&nest(32)), Err(Short));
        assert_eq!(PageHeader::parse(&nest(40)), Err(Short));
    }
}
