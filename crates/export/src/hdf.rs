//! The HDF5 and MAT v7.3 writers. MAT v7.3 is HDF5 underneath, so one writer serves both.
//!
//! - **HDF5**: a 1-D dataset per channel at the root, named by the channel (`/` becomes `_`), with its unit in a
//!   `unit` attribute; the time in `time_ns` (int64; ISO asks for the same) or `time_s` (f64 seconds).
//! - **MAT v7.3**: the file opens in MATLAB with `load`: a column vector per channel, named as a MATLAB variable
//!   (letters, digits and `_`, starting with a letter, up to 63 characters, made unique), and `time_ns` or `time`.
//!   Units and the original names are in `<name>.channels.csv`. The 512-byte header MATLAB reads comes first.
//!
//! Datasets are chunked (64k samples) and deflated, and grow block by block.

use std::path::Path;

use hdf5::types::FixedAscii;
use hdf5::{Dataset, Extent, Extents, File, Location};
use ndarray::ArrayView2;
use tlmbench_core::Column;
use tlmbench_core::chunks::CHUNK_SAMPLES;

use crate::{Block, Error, Plan, Result, Sink, TimeFormat};

fn err(e: hdf5::Error) -> Error {
    Error::Write(format!("HDF5: {e}"))
}

pub(crate) struct HdfSink<'a> {
    file: Option<File>,
    path: &'a Path,
    mat: bool,
    time: Option<Dataset>,
    /// The time in f64 seconds.
    seconds: bool,
    columns: Vec<Dataset>,
    rows: usize,
}

/// MATLAB variable names for `names`: invalid characters become `_`, a leading non-letter gets `x`, at most 63
/// characters, and repeats (or a clash with `taken`) get `_2`, `_3`, ….
pub(crate) fn matlab_names(names: &[&str], taken: &[&str]) -> Vec<String> {
    let mut used: std::collections::HashSet<String> = taken.iter().map(|s| (*s).to_owned()).collect();
    names
        .iter()
        .map(|n| {
            let mut v: String = n.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
            if !v.starts_with(|c: char| c.is_ascii_alphabetic()) {
                v.insert(0, 'x');
            }
            v.truncate(63);
            let mut out = v.clone();
            let mut k = 2;
            while used.contains(&out) {
                let suffix = format!("_{k}");
                out = format!("{}{suffix}", &v[..v.len().min(63 - suffix.len())]);
                k += 1;
            }
            used.insert(out.clone());
            out
        })
        .collect()
}

/// A fixed-length ASCII string attribute (as MATLAB writes `MATLAB_class`).
fn str_attr(loc: &Location, name: &str, value: &str) -> Result<()> {
    macro_rules! put {
        ($($n:literal)*) => {
            match value.len() {
                $($n => loc
                    .new_attr::<FixedAscii<$n>>()
                    .create(name)
                    .and_then(|a| a.write_scalar(&FixedAscii::<$n>::from_ascii(value).expect("ASCII"))),)*
                _ => unreachable!("class names are 4 to 7 characters"),
            }
        };
    }
    put!(4 5 6 7).map_err(err)
}

fn matlab_class(dtype: &str) -> &'static str {
    match dtype {
        "i8" => "int8",
        "i16" => "int16",
        "i32" => "int32",
        "i64" => "int64",
        "u8" => "uint8",
        "u16" => "uint16",
        "u32" => "uint32",
        "u64" => "uint64",
        "f32" => "single",
        _ => "double",
    }
}

impl<'a> HdfSink<'a> {
    pub(crate) fn create(path: &'a Path, plan: &Plan, mat: bool) -> Result<Self> {
        let file =
            if mat { File::with_options().with_fcpl(|p| p.userblock(512)).create(path) } else { File::create(path) }
                .map_err(err)?;
        let seconds = plan.time == Some(TimeFormat::Seconds);
        let time_name = match (seconds, mat) {
            (true, true) => "time",
            (true, false) => "time_s",
            (false, _) => "time_ns",
        };
        let raw_names: Vec<&str> = plan.channels.iter().map(|c| c.name.as_str()).collect();
        let names: Vec<String> = if mat {
            matlab_names(&raw_names, &plan.time.map(|_| time_name).into_iter().collect::<Vec<_>>())
        } else {
            let mut seen = std::collections::HashSet::new();
            raw_names
                .iter()
                .map(|n| {
                    let base = if n.is_empty() { "_".to_owned() } else { n.replace('/', "_") };
                    let mut out = base.clone();
                    let mut k = 2;
                    while out == time_name || !seen.insert(out.clone()) {
                        out = format!("{base} ({k})");
                        k += 1;
                    }
                    out
                })
                .collect()
        };
        let make = |name: &str, dtype: &str| -> Result<Dataset> {
            let b = file.new_dataset_builder().chunk(if mat { vec![1, CHUNK_SAMPLES] } else { vec![CHUNK_SAMPLES] });
            let b = b.deflate(4);
            // MATLAB stores arrays column-major: an N×1 column vector is (1, N) in HDF5
            let shape = if mat {
                Extents::new(vec![Extent::new(1, Some(1)), Extent::resizable(0)])
            } else {
                Extents::new(vec![Extent::resizable(0)])
            };
            macro_rules! ds {
                ($t:ty) => {
                    b.empty::<$t>().shape(shape).create(name).map_err(err)?
                };
            }
            let ds = match dtype {
                "i8" => ds!(i8),
                "i16" => ds!(i16),
                "i32" => ds!(i32),
                "i64" => ds!(i64),
                "u8" => ds!(u8),
                "u16" => ds!(u16),
                "u32" => ds!(u32),
                "u64" => ds!(u64),
                "f32" => ds!(f32),
                _ => ds!(f64),
            };
            if mat {
                str_attr(&ds, "MATLAB_class", matlab_class(dtype))?;
            }
            Ok(ds)
        };
        let time = match plan.time {
            Some(_) => {
                let ds = make(time_name, if seconds { "f64" } else { "i64" })?;
                if !mat {
                    let unit = if seconds { "s" } else { "ns" };
                    ds.new_attr::<hdf5::types::VarLenUnicode>()
                        .create("unit")
                        .and_then(|a| a.write_scalar(&unit.parse::<hdf5::types::VarLenUnicode>().unwrap()))
                        .map_err(err)?;
                }
                Some(ds)
            }
            None => None,
        };
        let mut columns = Vec::new();
        for (c, name) in plan.channels.iter().zip(&names) {
            let ds = make(name, c.dtype)?;
            if let (Some(u), false) = (&c.unit, mat) {
                let u: hdf5::types::VarLenUnicode = u.parse().map_err(|e| Error::Write(format!("{e}")))?;
                ds.new_attr::<hdf5::types::VarLenUnicode>()
                    .create("unit")
                    .and_then(|a| a.write_scalar(&u))
                    .map_err(err)?;
            }
            columns.push(ds);
        }
        if let (Some(note), false) = (&plan.note, mat) {
            let n: hdf5::types::VarLenUnicode = note.parse().map_err(|e| Error::Write(format!("{e}")))?;
            file.new_attr::<hdf5::types::VarLenUnicode>()
                .create("tlmbench")
                .and_then(|a| a.write_scalar(&n))
                .map_err(err)?;
        }
        Ok(Self { file: Some(file), path, mat, time, seconds, columns, rows: 0 })
    }

    fn append<T: hdf5::H5Type>(&self, ds: &Dataset, values: &[T]) -> Result<()> {
        let (a, b) = (self.rows, self.rows + values.len());
        if self.mat {
            ds.resize((1, b)).map_err(err)?;
            let view = ArrayView2::from_shape((1, values.len()), values).expect("one row");
            ds.write_slice(view, (0..1, a..b)).map_err(err)
        } else {
            ds.resize(b).map_err(err)?;
            ds.write_slice(values, a..b).map_err(err)
        }
    }
}

impl Sink for HdfSink<'_> {
    fn write(&mut self, block: &Block) -> Result<()> {
        let n = block.columns.first().map_or(0, Column::len);
        if n == 0 {
            return Ok(());
        }
        if let (Some(ds), Some(t)) = (&self.time, &block.times) {
            if self.seconds {
                let s: Vec<f64> = t.iter().map(|&t| t as f64 / 1e9).collect();
                self.append(ds, &s)?;
            } else {
                self.append(ds, t)?;
            }
        }
        for (ds, c) in self.columns.iter().zip(&block.columns) {
            match c {
                Column::I8(v) => self.append(ds, v)?,
                Column::I16(v) => self.append(ds, v)?,
                Column::I32(v) => self.append(ds, v)?,
                Column::I64(v) => self.append(ds, v)?,
                Column::U8(v) => self.append(ds, v)?,
                Column::U16(v) => self.append(ds, v)?,
                Column::U32(v) => self.append(ds, v)?,
                Column::U64(v) => self.append(ds, v)?,
                Column::F32(v) => self.append(ds, v)?,
                Column::F64(v) => self.append(ds, v)?,
            }
        }
        self.rows += n;
        Ok(())
    }

    fn finish(mut self: Box<Self>) -> Result<()> {
        self.time = None;
        self.columns.clear();
        if let Some(f) = self.file.take() {
            f.close().map_err(err)?;
        }
        if self.mat {
            write_mat_header(self.path)?;
        }
        Ok(())
    }
}

/// The 128 bytes MATLAB reads at the start of a v7.3 file: text padded to 116 bytes, 8 bytes of subsystem
/// offset, version 0x0200 and the endian mark `IM`.
fn write_mat_header(path: &Path) -> Result<()> {
    use std::io::{Seek, SeekFrom, Write};
    let mut h = b"MATLAB 7.3 MAT-file, Platform: tlmbench, HDF5 schema 1.00 .".to_vec();
    h.resize(116, b' ');
    h.extend_from_slice(&[0; 8]);
    h.extend_from_slice(&[0x00, 0x02]);
    h.extend_from_slice(b"IM");
    let mut f = std::fs::OpenOptions::new().write(true).open(path).map_err(crate::io_err(path))?;
    f.seek(SeekFrom::Start(0)).and_then(|_| f.write_all(&h)).map_err(crate::io_err(path))
}

#[cfg(test)]
mod tests {
    #[test]
    fn matlab_names_are_identifiers_and_unique() {
        let n = super::matlab_names(&["motor rpm", "motor-rpm", "1st", "time_ns", &"a".repeat(70)], &["time_ns"]);
        assert_eq!(n[..4], ["motor_rpm", "motor_rpm_2", "x1st", "time_ns_2"]);
        assert_eq!(n[4].len(), 63);
    }
}
