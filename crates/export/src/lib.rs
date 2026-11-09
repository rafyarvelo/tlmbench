//! Data export: a recording's samples written as CSV, Parquet, Arrow IPC, HDF5 or MAT v7.3, chunk by chunk, so memory stays flat
//! however large the recording. The CLI's `convert` and the app's Export write through here.
//!
//! - **Wide** (the default): a time column, then a column per channel, one row per sample. A recording's channels
//!   share their times, so one file holds them all with only real samples.
//! - **Long**: one tidy row per sample and channel: time, channel, value.
//! - **Resampled**: one row per step of a fixed-rate grid; each channel holds its last value at or before the step
//!   (the rule derived channels use), and the file says so (a `#` comment line in CSV, file metadata otherwise).
//!
//! Values are shown values (scale and offset applied, f64 where those change them); `raw` writes stored values in
//! their own type. Time is written exactly: seconds with nine decimals for relative times and ISO 8601 for dates in
//! CSV, and nanoseconds (a UTC timestamp for dates) in Parquet and Arrow. Recordings without time (sample numbers)
//! get no time column. Each file is written under a temporary name and renamed when complete, so a failed export
//! leaves nothing; beside it goes `<name>.channels.csv`, listing each channel's unit, type, rate and file.

mod columnar;
mod csv;
mod hdf;

use std::fs;
use std::path::{Path, PathBuf};

use tlmbench_core::chunks::CHUNK_SAMPLES;
use tlmbench_core::{Column, Recording, TimeSource, Timestamp};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{}: {source}", path.display())]
    Io { path: PathBuf, source: std::io::Error },

    #[error("{0}")]
    Write(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> Error + '_ {
    move |source| Error::Io { path: path.to_owned(), source }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Csv,
    Parquet,
    Arrow,
    Hdf5,
    Mat,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Format::Csv => "csv",
            Format::Parquet => "parquet",
            Format::Arrow => "arrow",
            Format::Hdf5 => "h5",
            Format::Mat => "mat",
        }
    }
}

/// How times are written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimeFormat {
    /// Exact and readable: seconds for relative times and ISO 8601 for dates in CSV; nanoseconds in Parquet and
    /// Arrow (a UTC timestamp for dates).
    #[default]
    Auto,
    /// Seconds (from the epoch for dates), nine decimals in CSV, f64 in Parquet and Arrow.
    Seconds,
    /// ISO 8601 in UTC (a UTC timestamp in Parquet and Arrow).
    Iso,
    /// Integer nanoseconds.
    Ns,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layout {
    #[default]
    Wide,
    Long,
}

#[derive(Clone, Debug)]
pub struct Options {
    pub format: Format,
    /// Channels to write, in order.
    pub channels: Vec<usize>,
    /// Samples `start..end`.
    pub start: usize,
    pub end: usize,
    pub time: TimeFormat,
    pub layout: Layout,
    /// One row per step of this rate (Hz) instead of per sample.
    pub resample_hz: Option<f64>,
    /// Stored values, not shown ones.
    pub raw: bool,
    /// CSV only.
    pub delimiter: u8,
    /// CSV only: units in a second header row instead of in brackets after each name.
    pub units_row: bool,
    /// The recording's sample rate (Hz) for the channel list, when known.
    pub rate_hz: Option<f64>,
}

impl Options {
    pub fn new(format: Format, channels: Vec<usize>, start: usize, end: usize) -> Self {
        Self {
            format,
            channels,
            start,
            end,
            time: TimeFormat::Auto,
            layout: Layout::Wide,
            resample_hz: None,
            raw: false,
            delimiter: b',',
            units_row: false,
            rate_hz: None,
        }
    }
}

/// What an export wrote.
#[derive(Clone, Debug, PartialEq)]
pub struct Written {
    pub path: PathBuf,
    pub manifest: PathBuf,
    pub rows: u64,
    pub bytes: u64,
}

/// One output column's description.
#[derive(Clone, Debug)]
pub(crate) struct OutChannel {
    pub name: String,
    pub unit: Option<String>,
    /// The type written: "f64" for shown values that scale or offset change, else the stored type.
    pub dtype: &'static str,
}

/// A block of rows: times (None without a time column) and a column per channel.
#[derive(Debug)]
pub(crate) struct Block {
    pub times: Option<Vec<Timestamp>>,
    pub columns: Vec<Column>,
}

/// What every format writer does.
pub(crate) trait Sink {
    fn write(&mut self, block: &Block) -> Result<()>;
    fn finish(self: Box<Self>) -> Result<()>;
}

/// The settled shape of an export, shared by the writers.
#[derive(Debug)]
pub(crate) struct Plan {
    pub layout: Layout,
    pub time: Option<TimeFormat>,
    /// Times are dates (since the epoch), not relative.
    pub absolute: bool,
    pub channels: Vec<OutChannel>,
    pub note: Option<String>,
}

/// Writes samples of `rec` as `opts` says to `out` (a file path; its folder must exist).
pub fn export(rec: &dyn Recording, opts: &Options, out: &Path) -> Result<Written> {
    export_with(rec, &[], opts, out)
}

/// [`export`] with more channels from recordings that share `rec`'s sample times (derived channels): channel numbers
/// in `opts` count `rec`'s channels first, then each of `extra`'s in turn.
pub fn export_with(rec: &dyn Recording, extra: &[&dyn Recording], opts: &Options, out: &Path) -> Result<Written> {
    let info = rec.info();
    if let Some(bad) = extra.iter().find(|e| e.info().samples != info.samples) {
        return Err(Error::Write(format!(
            "{} has {} samples, not {}",
            bad.info().path.display(),
            bad.info().samples,
            info.samples
        )));
    }
    // channel number → (recording, its own channel number)
    let locate = |c: usize| -> Option<(&dyn Recording, usize)> {
        let mut c = c;
        for r in std::iter::once(rec).chain(extra.iter().copied()) {
            let n = r.info().channels.len();
            if c < n {
                return Some((r, c));
            }
            c -= n;
        }
        None
    };
    if let Some(&bad) = opts.channels.iter().find(|&&c| locate(c).is_none()) {
        return Err(Error::Write(format!("no channel {bad}")));
    }
    let end = opts.end.min(info.samples);
    let start = opts.start.min(end);
    let has_time = !matches!(info.time, TimeSource::Index);
    if opts.layout == Layout::Long && matches!(opts.format, Format::Hdf5 | Format::Mat) {
        return Err(Error::Write("the long layout is for CSV, Parquet and Arrow".into()));
    }
    if opts.resample_hz.is_some() && !has_time {
        return Err(Error::Write("a recording without time can't be resampled".into()));
    }
    let transforms: Vec<(f64, f64)> = opts
        .channels
        .iter()
        .map(|&c| {
            let (r, c) = locate(c).unwrap();
            r.scale_offset(c)
        })
        .collect();
    let shown = |k: usize| !opts.raw && transforms[k] != (1.0, 0.0);
    let channels: Vec<OutChannel> = opts
        .channels
        .iter()
        .enumerate()
        .map(|(k, &c)| {
            let (r, c) = locate(c).unwrap();
            let ch = &r.info().channels[c];
            OutChannel { name: ch.name.clone(), unit: ch.unit.clone(), dtype: if shown(k) { "f64" } else { ch.dtype } }
        })
        .collect();
    let first = (start < end).then(|| rec.time_at(start));
    let absolute = first.is_some_and(|t| t.abs() >= tlmbench_core::datetime::RELATIVE_BEFORE);
    let note = opts
        .resample_hz
        .map(|hz| format!("resampled to {hz} Hz: each channel holds its last value at or before each row's time"));
    let plan = Plan { layout: opts.layout, time: has_time.then_some(opts.time), absolute, channels, note };

    let dir = out.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = out.file_name().ok_or_else(|| Error::Write(format!("{}: not a file name", out.display())))?;
    let tmp = dir.join(format!(".{}.tmp-{}", name.to_string_lossy(), std::process::id()));
    let result = (|| {
        let mut sink: Box<dyn Sink> = match opts.format {
            Format::Csv => Box::new(csv::CsvSink::create(&tmp, &plan, opts)?),
            Format::Parquet | Format::Arrow => Box::new(columnar::ColumnarSink::create(&tmp, &plan, opts.format)?),
            Format::Hdf5 | Format::Mat => Box::new(hdf::HdfSink::create(&tmp, &plan, opts.format == Format::Mat)?),
        };
        let mut rows = 0u64;
        let mut emit = |block: Block| -> Result<()> {
            rows += block.columns.first().map_or(0, |c| c.len()) as u64
                * if plan.layout == Layout::Long { plan.channels.len() as u64 } else { 1 };
            sink.write(&block)
        };
        let read = |c: usize, k: usize, a: usize, b: usize| -> Column {
            let (r, c) = locate(c).unwrap();
            let col = r.read(c, a, b);
            if shown(k) {
                let (s, o) = transforms[k];
                Column::F64(col.to_f64(s, o))
            } else {
                col
            }
        };
        match opts.resample_hz {
            None => {
                for a in (start..end).step_by(CHUNK_SAMPLES) {
                    let b = (a + CHUNK_SAMPLES).min(end);
                    let times = has_time.then(|| rec.read_times(a, b));
                    let columns = opts.channels.iter().enumerate().map(|(k, &c)| read(c, k, a, b)).collect();
                    emit(Block { times, columns })?;
                }
            }
            Some(hz) => {
                if !(hz > 0.0 && hz.is_finite()) {
                    return Err(Error::Write(format!("bad resample rate {hz}")));
                }
                if start < end {
                    let (t0, t1) = (rec.time_at(start), rec.time_at(end - 1));
                    let period = 1e9 / hz;
                    let steps = ((t1 - t0) as f64 / period).floor() as u64 + 1;
                    for s0 in (0..steps).step_by(CHUNK_SAMPLES) {
                        let s1 = (s0 + CHUNK_SAMPLES as u64).min(steps);
                        let times: Vec<Timestamp> =
                            (s0..s1).map(|k| t0 + (k as f64 * period).round() as Timestamp).collect();
                        // the last sample at or before each step, within start..end
                        let idx: Vec<usize> = times
                            .iter()
                            .map(|&t| rec.lower_bound(t.saturating_add(1)).clamp(start + 1, end) - 1)
                            .collect();
                        let (a, b) = (idx[0], idx[idx.len() - 1] + 1);
                        let columns = opts
                            .channels
                            .iter()
                            .enumerate()
                            .map(|(k, &c)| gather(&read(c, k, a, b), &idx, a))
                            .collect();
                        emit(Block { times: Some(times), columns })?;
                    }
                }
            }
        }
        sink.finish()?;
        Ok(rows)
    })();
    let rows = match result {
        Ok(r) => r,
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
    };
    fs::rename(&tmp, out).map_err(io_err(out))?;
    let bytes = fs::metadata(out).map_err(io_err(out))?.len();
    let manifest = write_manifest(out, &plan, opts)?;
    Ok(Written { path: out.to_owned(), manifest, rows, bytes })
}

/// `col` (samples from `at` on) at each index in `idx`.
fn gather(col: &Column, idx: &[usize], at: usize) -> Column {
    macro_rules! pick {
        ($v:expr, $c:ident) => {
            Column::$c(idx.iter().map(|&i| $v[i - at]).collect())
        };
    }
    match col {
        Column::I8(v) => pick!(v, I8),
        Column::I16(v) => pick!(v, I16),
        Column::I32(v) => pick!(v, I32),
        Column::I64(v) => pick!(v, I64),
        Column::U8(v) => pick!(v, U8),
        Column::U16(v) => pick!(v, U16),
        Column::U32(v) => pick!(v, U32),
        Column::U64(v) => pick!(v, U64),
        Column::F32(v) => pick!(v, F32),
        Column::F64(v) => pick!(v, F64),
    }
}

/// `<name>.channels.csv` beside the export: channel, unit, type, rate_hz, file.
fn write_manifest(out: &Path, plan: &Plan, opts: &Options) -> Result<PathBuf> {
    let stem = out.file_stem().unwrap_or_default().to_string_lossy();
    let path = out.with_file_name(format!("{stem}.channels.csv"));
    let file = out.file_name().unwrap_or_default().to_string_lossy();
    let rate = opts.resample_hz.or(opts.rate_hz).map(|r| r.to_string()).unwrap_or_default();
    let mut text = String::from("channel,unit,type,rate_hz,file\n");
    for c in &plan.channels {
        let fields = [c.name.as_str(), c.unit.as_deref().unwrap_or(""), c.dtype, &rate, &file];
        text.push_str(&fields.map(|f| csv::quote(f, b',')).join(","));
        text.push('\n');
    }
    fs::write(&path, text).map_err(io_err(&path))?;
    Ok(path)
}

#[cfg(test)]
mod tests;
