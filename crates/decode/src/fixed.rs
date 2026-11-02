//! Fixed-width records read in place: record `i` is at `header + i * record_size`, so the file itself is the store
//! and nothing is copied or cached.

use std::marker::PhantomData;
use std::path::Path;
use std::sync::Mutex;
use std::thread::JoinHandle;

use rayon::prelude::*;
use tlmbench_core::chunks::{CHUNK_SAMPLES, Chunk, Chunked, Held};
use tlmbench_core::pyramid::{self, Summary};
use tlmbench_core::{ChannelInfo, ChannelOp, Channels, RateTimes, RecordingInfo, Shifted, TimeSource, Times, TimesOp};

use crate::bytes::{self, Bytes, read_exact_at};
use crate::descriptor::{DType, Descriptor, Endian, Field, TimeSpec};
use crate::error::{Error, Result, format_err, io_err};
use crate::reference;
use crate::strided::{FieldTimes, Pod, Strided};

#[derive(Debug)]
pub struct FixedWidth {
    bytes: Bytes,
    header_len: usize,
    records: usize,
    desc: Descriptor,
    /// Index into `desc.fields` of each channel.
    channels: Vec<usize>,
    info: RecordingInfo,
    /// The next block's records being read in the background while a build summarizes this one.
    read_ahead: Mutex<Option<ReadAhead>>,
    /// Buffers kept between a build's blocks: fresh ones of tens of MB per block would be zero-filled page by page,
    /// and page faults on macOS come one at a time.
    spare_reads: Mutex<Vec<Vec<u8>>>,
    spare_columns: Mutex<Vec<Vec<u64>>>,
}

/// Records `start..` read on another thread.
#[derive(Debug)]
struct ReadAhead {
    start: usize,
    end: usize,
    /// Empty when the read failed.
    bytes: JoinHandle<Vec<u8>>,
}

impl FixedWidth {
    /// Opens a tlmbench reference file (descriptor in its header).
    pub fn open_reference(path: &Path) -> Result<Self> {
        let bytes = bytes::map(path)?;
        let (header_len, desc) = reference::read_header(&bytes).map_err(|e| format_err(path, e.to_string()))?;
        Self::new(path, bytes, header_len, desc, "tlmbench reference")
    }

    /// Opens a fixed-width file laid out by a separate descriptor file.
    pub fn open_with_descriptor(path: &Path, descriptor: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(descriptor).map_err(io_err(descriptor))?;
        let desc = Descriptor::parse(&text).map_err(|e| match e {
            Error::Descriptor(msg) => Error::Descriptor(format!("{}: {msg}", descriptor.display())),
            other => other,
        })?;
        let bytes = bytes::map(path)?;
        let header_len = desc.header_size as usize;
        let name = desc.name.clone().unwrap_or_else(|| "fixed-width".into());
        Self::new(path, bytes, header_len, desc, &name)
    }

    /// From bytes already in memory (tests and fuzzing).
    pub fn from_reference_bytes(data: Vec<u8>) -> Result<Self> {
        let path = Path::new("<memory>");
        let (header_len, desc) = reference::read_header(&data).map_err(|e| format_err(path, e.to_string()))?;
        Self::new(path, Bytes::Owned(data), header_len, desc, "tlmbench reference")
    }

    fn new(path: &Path, bytes: Bytes, header_len: usize, desc: Descriptor, format: &str) -> Result<Self> {
        desc.validate().map_err(|e| format_err(path, e.to_string()))?;
        if header_len > bytes.len() {
            return Err(format_err(path, format!("{} bytes, shorter than its {header_len}-byte header", bytes.len())));
        }
        let records = (bytes.len() - header_len) / desc.record_size as usize;
        let channels: Vec<usize> = desc.channels().map(|(i, _)| i).collect();
        let mut me = Self {
            bytes,
            header_len,
            records,
            channels,
            read_ahead: Mutex::new(None),
            spare_reads: Mutex::default(),
            spare_columns: Mutex::default(),
            info: RecordingInfo {
                path: path.to_owned(),
                format: format.to_owned(),
                samples: records,
                time: TimeSource::Index,
                t_first: None,
                t_last: None,
                channels: Vec::new(),
                skipped: Vec::new(),
            },
            desc,
        };
        me.info.channels = me
            .channels
            .iter()
            .map(|&i| {
                let f = &me.desc.fields[i];
                ChannelInfo { name: f.name.clone(), unit: f.unit.clone(), dtype: f.dtype.name() }
            })
            .collect();
        me.info.time = match &me.desc.time {
            TimeSpec::Rate { rate_hz, .. } => TimeSource::Rate { hz: *rate_hz },
            TimeSpec::Field { field, .. } => TimeSource::Column { name: field.clone() },
            TimeSpec::Index => TimeSource::Index,
        };
        // fixed-rate and index times can't go backwards; a time field is checked record by record
        let check = matches!(me.desc.time, TimeSpec::Field { .. });
        let (first, last) = me
            .with_times(|t| {
                if check && let Some(i) = (1..t.len()).find(|&i| t.at(i) < t.at(i - 1)) {
                    return Err(i);
                }
                Ok((t.len() > 0).then(|| (t.at(0), t.at(t.len() - 1))))
            })
            .map_err(|i| format_err(path, format!("time goes backwards at record {i}")))?
            .unzip();
        (me.info.t_first, me.info.t_last) = (first, last);
        Ok(me)
    }

    fn records_bytes(&self) -> &[u8] {
        &self.bytes[self.header_len..self.header_len + self.records * self.desc.record_size as usize]
    }

    /// Whether `field` can be read from the map as a slice: contiguous, native order and aligned.
    fn contiguous(&self, field: &Field) -> bool {
        let size = field.dtype.size() as usize;
        self.desc.record_size as usize == size
            && field.offset == 0
            && (self.desc.endian == Endian::Little) == cfg!(target_endian = "little")
            && (self.records_bytes().as_ptr() as usize).is_multiple_of(size)
    }

    /// Records `start..end`, with one positioned read into `buf` (page faults on one map come one at a time on
    /// macOS); copied from memory for bytes not mapped.
    fn read_records(&self, start: usize, end: usize, mut buf: Vec<u8>) -> Vec<u8> {
        let stride = self.desc.record_size as usize;
        if matches!(self.bytes, Bytes::Mapped(_)) {
            buf.resize((end - start) * stride, 0);
            let at = (self.header_len + start * stride) as u64;
            if std::fs::File::open(&self.info.path).and_then(|f| read_exact_at(&f, &mut buf, at)).is_ok() {
                return buf;
            }
        }
        buf.clear();
        buf.extend_from_slice(&self.records_bytes()[start * stride..end * stride]);
        buf
    }

    /// Calls `f` with this file's sample times, whichever kind they are.
    fn with_times<R>(&self, f: impl FnOnce(&dyn TimesRef) -> R) -> R {
        match &self.desc.time {
            TimeSpec::Rate { rate_hz, t0_ns } => f(&RateTimes::from_rate(*t0_ns, *rate_hz, self.records)),
            TimeSpec::Index => f(&RateTimes::from_rate(0, 1.0, self.records)),
            TimeSpec::Field { field, unit, t0_ns } => {
                let tf = self.desc.fields.iter().find(|x| &x.name == field).expect("validated");
                f(&FieldTimes {
                    records: self.records_bytes(),
                    offset: tf.offset as usize,
                    stride: self.desc.record_size as usize,
                    big_endian: self.desc.endian == Endian::Big,
                    dtype: tf.dtype,
                    scale_ns: unit.ns(),
                    t0_ns: *t0_ns,
                })
            }
        }
    }
}

/// Object-safe view of the time types, for the open-time scan only (queries use the concrete types).
trait TimesRef {
    fn len(&self) -> usize;
    fn at(&self, i: usize) -> i64;
}

impl<T: Times> TimesRef for T {
    fn len(&self) -> usize {
        Times::len(self)
    }
    fn at(&self, i: usize) -> i64 {
        Times::at(self, i)
    }
}

impl Channels for FixedWidth {
    fn describe(&self) -> &RecordingInfo {
        &self.info
    }

    fn dispatch<O: ChannelOp>(&self, channel: usize, op: O) -> O::Out {
        self.dispatch_held(channel, &Held::default(), op)
    }

    fn dispatch_held<O: ChannelOp>(&self, channel: usize, held: &Held, op: O) -> O::Out {
        let field = &self.desc.fields[self.channels[channel]];
        let (records, stride, offset) = (self.records_bytes(), self.desc.record_size as usize, field.offset as usize);
        let big = self.desc.endian == Endian::Big;
        let contiguous = self.contiguous(field);

        struct WithTimes<'a, O> {
            op: O,
            dtype: DType,
            records: &'a [u8],
            offset: usize,
            stride: usize,
            big: bool,
            contiguous: bool,
            held: &'a Held,
        }
        impl<O: ChannelOp> TimesOp for WithTimes<'_, O> {
            type Out = O::Out;
            fn run<T: Times + ?Sized>(self, times: &T) -> O::Out {
                fn run<V: Pod, O: ChannelOp, T: Times + ?Sized>(w: WithTimes<O>, times: &T) -> O::Out {
                    // contiguous, native order and aligned: read the map as a slice (the fast path)
                    if w.contiguous {
                        return w.op.run(times, bytemuck::cast_slice::<u8, V>(w.records));
                    }
                    let (records, offset, stride, big_endian) = (w.records, w.offset, w.stride, w.big);
                    let base = Strided::<V> { records, offset, stride, big_endian, _v: PhantomData };
                    if w.held.is_empty() {
                        return w.op.run(times, &base);
                    }
                    w.op.run(times, &Chunked { base: &base, held: w.held })
                }
                match self.dtype {
                    DType::I8 => run::<i8, _, _>(self, times),
                    DType::I16 => run::<i16, _, _>(self, times),
                    DType::I32 => run::<i32, _, _>(self, times),
                    DType::I64 => run::<i64, _, _>(self, times),
                    DType::U8 => run::<u8, _, _>(self, times),
                    DType::U16 => run::<u16, _, _>(self, times),
                    DType::U32 => run::<u32, _, _>(self, times),
                    DType::U64 => run::<u64, _, _>(self, times),
                    DType::F32 => run::<f32, _, _>(self, times),
                    DType::F64 => run::<f64, _, _>(self, times),
                }
            }
        }
        self.dispatch_times(WithTimes { op, dtype: field.dtype, records, offset, stride, big, contiguous, held })
    }

    fn chunked(&self, channel: usize) -> bool {
        !self.contiguous(&self.desc.fields[self.channels[channel]])
    }

    /// Each chunk's records read once (chunks on several threads, so cold reads overlap), then every channel
    /// gathered from them.
    fn decode_chunks(&self, channels: &[usize], chunks: std::ops::Range<usize>) -> Vec<Vec<Chunk>> {
        let stride = self.desc.record_size as usize;
        let big = self.desc.endian == Endian::Big;
        let per_chunk: Vec<Vec<Chunk>> = chunks
            .into_par_iter()
            .map_init(
                || Spare(self.spare_reads.lock().unwrap().pop().unwrap_or_default(), &self.spare_reads),
                |buf, k| {
                    let (a, b) = ((k * CHUNK_SAMPLES).min(self.records), ((k + 1) * CHUNK_SAMPLES).min(self.records));
                    // in memory: gathered from the map; else one positioned read (page faults on one map come one
                    // at a time on macOS, and a cold 1M-sample view reads twice as fast this way)
                    let mapped = &self.records_bytes()[a * stride..b * stride];
                    let records = if crate::bytes::resident_share(mapped).is_none_or(|r| r >= 0.5) {
                        mapped
                    } else {
                        buf.0 = self.read_records(a, b, std::mem::take(&mut buf.0));
                        &buf.0[..]
                    };
                    let mut cols: Vec<Vec<u64>> = channels
                        .iter()
                        .map(|&c| {
                            let size = self.desc.fields[self.channels[c]].dtype.size() as usize;
                            vec![0u64; ((b - a) * size).div_ceil(8)]
                        })
                        .collect();
                    // a tile of records at a time, so each is read from cache by every channel
                    for t in (0..b - a).step_by(GATHER_TILE) {
                        let tile = &records[t * stride..((t + GATHER_TILE).min(b - a)) * stride];
                        for (&c, words) in channels.iter().zip(&mut cols) {
                            let field = &self.desc.fields[self.channels[c]];
                            gather_tile(field.dtype, tile, stride, field.offset as usize, big, words, t);
                        }
                    }
                    cols.into_iter().map(|w| Chunk::new(w, b - a)).collect()
                },
            )
            .collect();
        let mut out: Vec<Vec<Chunk>> = channels.iter().map(|_| Vec::with_capacity(per_chunk.len())).collect();
        for row in per_chunk {
            for (o, c) in out.iter_mut().zip(row) {
                o.push(c);
            }
        }
        out
    }

    fn dispatch_times<O: TimesOp>(&self, op: O) -> O::Out {
        match &self.desc.time {
            TimeSpec::Rate { rate_hz, t0_ns } => op.run(&RateTimes::from_rate(*t0_ns, *rate_hz, self.records)),
            TimeSpec::Index => op.run(&RateTimes::from_rate(0, 1.0, self.records)),
            TimeSpec::Field { field, unit, t0_ns } => {
                let tf = self.desc.fields.iter().find(|x| &x.name == field).expect("validated");
                op.run(&FieldTimes {
                    records: self.records_bytes(),
                    offset: tf.offset as usize,
                    stride: self.desc.record_size as usize,
                    big_endian: self.desc.endian == Endian::Big,
                    dtype: tf.dtype,
                    scale_ns: unit.ns(),
                    t0_ns: *t0_ns,
                })
            }
        }
    }

    /// One positioned read of the block's records (parallel page faults on one map serialize on macOS), then each
    /// channel gathered into a buffer of its own and summarized on the slice path. The next block is read ahead
    /// meanwhile.
    fn summarize_all(&self, start: usize, end: usize) -> Vec<Vec<Summary>> {
        let spare = || self.spare_reads.lock().unwrap().pop().unwrap_or_default();
        let mut ahead = self.read_ahead.lock().unwrap();
        let buf = match ahead.take() {
            Some(r) if (r.start, r.end) == (start, end) => r.bytes.join().ok(),
            _ => None,
        }
        .unwrap_or_else(|| self.read_records(start, end, spare()));
        // read the next block of the same size while this one is summarized
        let (next, next_end) = (end, (end + (end - start)).min(self.records));
        if matches!(self.bytes, Bytes::Mapped(_)) && next < next_end {
            let into = spare();
            let (path, header, stride) = (self.info.path.clone(), self.header_len, self.desc.record_size as usize);
            let bytes = std::thread::spawn(move || {
                let mut buf = into;
                buf.resize((next_end - next) * stride, 0);
                let f = std::fs::File::open(path).ok();
                match f.map(|f| read_exact_at(&f, &mut buf, (header + next * stride) as u64)) {
                    Some(Ok(())) => buf,
                    _ => Vec::new(),
                }
            });
            *ahead = Some(ReadAhead { start: next, end: next_end, bytes });
        }
        drop(ahead);
        // a failed read ahead comes back empty: read the block again
        let buf = if buf.is_empty() && start < end { self.read_records(start, end, buf) } else { buf };

        struct Op<'a> {
            me: &'a FixedWidth,
            buf: &'a [u8],
            start: usize,
        }
        impl TimesOp for Op<'_> {
            type Out = Vec<Vec<Summary>>;
            fn run<T: Times + ?Sized>(self, times: &T) -> Vec<Vec<Summary>> {
                fn one<V: Pod, T: Times + ?Sized>(op: &Op, times: &T, field: &Field) -> Vec<Summary> {
                    let (stride, big) = (op.me.desc.record_size as usize, op.me.desc.endian == Endian::Big);
                    let mut words = op.me.spare_columns.lock().unwrap().pop().unwrap_or_default();
                    let n = gather::<V>(op.buf, stride, field.offset as usize, big, &mut words);
                    let data = &bytemuck::cast_slice::<u64, V>(&words)[..n];
                    let out = pyramid::summarize(times, &Shifted { offset: op.start, data }, op.start, op.start + n);
                    op.me.spare_columns.lock().unwrap().push(words);
                    out
                }
                self.me
                    .channels
                    .par_iter()
                    .map(|&fi| {
                        let field = &self.me.desc.fields[fi];
                        match field.dtype {
                            DType::I8 => one::<i8, T>(&self, times, field),
                            DType::I16 => one::<i16, T>(&self, times, field),
                            DType::I32 => one::<i32, T>(&self, times, field),
                            DType::I64 => one::<i64, T>(&self, times, field),
                            DType::U8 => one::<u8, T>(&self, times, field),
                            DType::U16 => one::<u16, T>(&self, times, field),
                            DType::U32 => one::<u32, T>(&self, times, field),
                            DType::U64 => one::<u64, T>(&self, times, field),
                            DType::F32 => one::<f32, T>(&self, times, field),
                            DType::F64 => one::<f64, T>(&self, times, field),
                        }
                    })
                    .collect()
            }
        }
        let out = self.dispatch_times(Op { me: self, buf: &buf, start });
        self.spare_reads.lock().unwrap().push(buf);
        out
    }

    /// Reads the records' pages with positioned reads on several threads (faults on one map would come one by one).
    fn prefetch(&self, ranges: &[std::ops::Range<usize>]) {
        if !matches!(self.bytes, Bytes::Mapped(_)) {
            return;
        }
        let Ok(f) = std::fs::File::open(&self.info.path) else { return };
        let stride = self.desc.record_size as usize;
        ranges.par_iter().for_each_init(Vec::new, |buf, r| {
            let (a, b) = (r.start.min(self.records), r.end.min(self.records));
            if a < b {
                buf.resize((b - a) * stride, 0);
                let _ = read_exact_at(&f, buf, (self.header_len + a * stride) as u64);
            }
        });
    }

    fn transform(&self, channel: usize) -> (f64, f64) {
        let f = &self.desc.fields[self.channels[channel]];
        (f.scale, f.value_offset)
    }

    fn layout_id(&self) -> Vec<u8> {
        let mut id = format!("fixed\n{}\n{}\n", self.info.format, self.header_len).into_bytes();
        id.extend(toml::to_string(&self.desc).unwrap_or_default().into_bytes());
        id
    }
}

/// The field at `offset` of each `stride`-byte record in `records`, decoded into `words` as native `V`s; returns
/// how many.
fn gather<V: Pod>(records: &[u8], stride: usize, offset: usize, big: bool, words: &mut Vec<u64>) -> usize {
    let n = records.len() / stride;
    words.resize((n * V::SIZE).div_ceil(8), 0);
    gather_into::<V>(records, stride, offset, big, words, 0);
    n
}

/// A read buffer that goes back to the spares when dropped.
struct Spare<'a>(Vec<u8>, &'a Mutex<Vec<Vec<u8>>>);

impl Drop for Spare<'_> {
    fn drop(&mut self) {
        let mut spares = self.1.lock().unwrap();
        if spares.len() < 16 {
            spares.push(std::mem::take(&mut self.0));
        }
    }
}

/// Records per tile when gathering several fields.
const GATHER_TILE: usize = 1024;

/// The field at `offset` of each record in `records`, decoded into `words` (as `V`s) from value `at` on.
fn gather_into<V: Pod>(records: &[u8], stride: usize, offset: usize, big: bool, words: &mut [u64], at: usize) {
    let out = &mut bytemuck::cast_slice_mut::<u64, V>(words)[at..at + records.len() / stride];
    for (d, r) in out.iter_mut().zip(records.chunks_exact(stride)) {
        let b = &r[offset..offset + V::SIZE];
        *d = if big { V::from_be(b) } else { V::from_le(b) };
    }
}

/// [`gather_into`] for a field of type `dtype`.
fn gather_tile(dtype: DType, records: &[u8], stride: usize, offset: usize, big: bool, words: &mut [u64], at: usize) {
    match dtype {
        DType::I8 => gather_into::<i8>(records, stride, offset, big, words, at),
        DType::I16 => gather_into::<i16>(records, stride, offset, big, words, at),
        DType::I32 => gather_into::<i32>(records, stride, offset, big, words, at),
        DType::I64 => gather_into::<i64>(records, stride, offset, big, words, at),
        DType::U8 => gather_into::<u8>(records, stride, offset, big, words, at),
        DType::U16 => gather_into::<u16>(records, stride, offset, big, words, at),
        DType::U32 => gather_into::<u32>(records, stride, offset, big, words, at),
        DType::U64 => gather_into::<u64>(records, stride, offset, big, words, at),
        DType::F32 => gather_into::<f32>(records, stride, offset, big, words, at),
        DType::F64 => gather_into::<f64>(records, stride, offset, big, words, at),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::{Field, TimeUnit};
    use tlmbench_core::{Recording, TraceKind, Viewport};

    fn field(name: &str, dtype: DType, offset: u64) -> Field {
        Field { name: name.into(), dtype, offset, unit: None, scale: 1.0, value_offset: 0.0 }
    }

    fn file(desc: &Descriptor, records: impl IntoIterator<Item = Vec<u8>>) -> FixedWidth {
        let mut data = Vec::new();
        reference::write_header(&mut data, desc).unwrap();
        for r in records {
            data.extend(r);
        }
        FixedWidth::from_reference_bytes(data).unwrap()
    }

    #[test]
    fn rate_file_reads_contiguous_f32() {
        let desc = Descriptor {
            schema: 1,
            name: None,
            endian: Endian::Little,
            header_size: 0,
            record_size: 4,
            time: TimeSpec::Rate { rate_hz: 100.0, t0_ns: 1_000 },
            fields: vec![field("v", DType::F32, 0)],
        };
        let f = file(&desc, (0..1000).map(|i| (i as f32).to_le_bytes().to_vec()));
        assert_eq!(f.info().samples, 1000);
        assert_eq!((f.info().t_first, f.info().t_last), (Some(1_000), Some(1_000 + 999 * 10_000_000)));
        let tr = f.trace(0, Viewport { t0: 0, t1: 50_000_000, width_px: 100 });
        assert_eq!(tr.kind, TraceKind::Raw);
        assert_eq!(tr.y, vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]); // 5 in view plus the one after
    }

    #[test]
    fn interleaved_big_endian_with_time_field_and_scale() {
        let mut v = field("battery_v", DType::I16, 8);
        v.scale = 0.001;
        v.value_offset = 1.0;
        let desc = Descriptor {
            schema: 1,
            name: Some("bench".into()),
            endian: Endian::Big,
            header_size: 0,
            record_size: 12,
            time: TimeSpec::Field { field: "t".into(), unit: TimeUnit::Us, t0_ns: 0 },
            fields: vec![field("t", DType::U64, 0), v, field("flags", DType::U8, 10)],
        };
        let f = file(
            &desc,
            (0..10u64).map(|i| {
                let mut r = (i * 500).to_be_bytes().to_vec();
                r.extend((i as i16 * 1000).to_be_bytes());
                r.extend([i as u8, 0]);
                r
            }),
        );
        let names: Vec<_> = f.info().channels.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["battery_v", "flags"]);
        assert_eq!(f.info().time, TimeSource::Column { name: "t".into() });
        assert_eq!(f.info().t_last, Some(4_500_000));
        let tr = f.trace(0, Viewport { t0: 0, t1: 5_000_000, width_px: 100 });
        assert_eq!(tr.t[3], 1_500_000);
        assert!((tr.y[3] - 4.0).abs() < 1e-6); // 3000 * 0.001 + 1
    }

    #[test]
    fn pyramid_views_and_stats_of_a_strided_scaled_field() {
        let mut v = field("v", DType::I16, 2);
        v.scale = -0.5;
        v.value_offset = 10.0;
        let desc = Descriptor {
            schema: 1,
            name: None,
            endian: Endian::Big,
            header_size: 0,
            record_size: 6,
            time: TimeSpec::Rate { rate_hz: 1000.0, t0_ns: 0 },
            fields: vec![field("a", DType::U16, 0), v, field("b", DType::U16, 4)],
        };
        let n = 50_000usize;
        let raw = |i: usize| {
            ((i * 7919 % 2001) as i16 - 1000) * if i == 31_337 { 0 } else { 1 } + if i == 4_242 { 2000 } else { 0 }
        };
        let f = file(&desc, (0..n).map(|i| [0u16.to_be_bytes(), raw(i).to_be_bytes(), 0u16.to_be_bytes()].concat()));
        let levels = tlmbench_core::pyramid::build_levels(f.summarize(1, 0, n));
        let pyr = tlmbench_core::pyramid::Pyramid::new(n, levels.iter().map(Vec::as_slice).collect());
        let vp = Viewport { t0: 0, t1: f.info().t_last.unwrap() + 1, width_px: 30 };
        assert_eq!(f.view(1, vp, Some(&pyr), &[], &Held::default()), f.trace(1, vp));
        let st = f.stats(1, &pyr, 100, 40_000, &Held::default());
        // the largest stored value (2000 + something at 4242) becomes the smallest shown one
        let shown: Vec<f64> = (100..40_000).map(|i| raw(i) as f64 * -0.5 + 10.0).collect();
        let lo = shown.iter().copied().fold(f64::INFINITY, f64::min);
        assert_eq!(st.min.map(|m| m.0), Some(lo));
        assert_eq!(st.min.map(|m| m.1), Some(4_242));
        assert_eq!(f.value_at(1, 4_242, &Held::default()), raw(4_242) as f64 * -0.5 + 10.0);
        let mean = shown.iter().sum::<f64>() / shown.len() as f64;
        assert!((st.mean - mean).abs() < 1e-9);
    }

    /// Reading a block once for every channel gives the same summaries as summarizing each channel in place.
    #[test]
    fn block_summaries_match_per_channel_ones() {
        let desc = Descriptor {
            schema: 1,
            name: None,
            endian: Endian::Big,
            header_size: 0,
            record_size: 15,
            time: TimeSpec::Field { field: "t".into(), unit: TimeUnit::Us, t0_ns: 5 },
            fields: vec![
                field("t", DType::U32, 0),
                field("x", DType::F64, 4),
                field("k", DType::I16, 12),
                field("f", DType::U8, 14),
            ],
        };
        let n = 70_000u32;
        let mut data = Vec::new();
        reference::write_header(&mut data, &desc).unwrap();
        for i in 0..n {
            data.extend((i * 3 + i / 1000).to_be_bytes());
            data.extend(if i % 777 == 0 { f64::NAN } else { (i as f64 * 0.01).cos() }.to_be_bytes());
            data.extend(((i * 31 % 2000) as i16 - 1000).to_be_bytes());
            data.push((i % 251) as u8);
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mixed.tlm");
        std::fs::write(&path, &data).unwrap();
        let f = FixedWidth::open_reference(&path).unwrap();
        for (a, b) in [(0usize, 70_000usize), (256 * 7, 256 * 100), (65_536, 70_000)] {
            let all = Channels::summarize_all(&f, a, b);
            assert_eq!(all.len(), 3);
            for (c, got) in all.iter().enumerate() {
                let want = f.summarize(c, a, b);
                let same = |x: &Summary, y: &Summary| bytemuck::bytes_of(x) == bytemuck::bytes_of(y);
                assert!(
                    got.len() == want.len() && got.iter().zip(&want).all(|(x, y)| same(x, y)),
                    "channel {c}, {a}..{b}"
                );
            }
        }
    }

    /// Decoded chunks hold each field's values, and views read through them match views read in place.
    #[test]
    fn decoded_chunks_read_like_the_fields() {
        let desc = Descriptor {
            schema: 1,
            name: None,
            endian: Endian::Big,
            header_size: 0,
            record_size: 11,
            time: TimeSpec::Rate { rate_hz: 1000.0, t0_ns: 0 },
            fields: vec![field("x", DType::F64, 0), field("k", DType::I16, 8), field("f", DType::U8, 10)],
        };
        let n = 2 * CHUNK_SAMPLES + 777;
        let f = file(
            &desc,
            (0..n).map(|i| {
                let x = if i % 5000 == 1 { f64::NAN } else { (i as f64 * 0.001).sin() };
                [&x.to_be_bytes()[..], &((i % 3001) as i16 - 1500).to_be_bytes(), &[(i % 256) as u8]].concat()
            }),
        );
        assert!((0..3).all(|c| Channels::chunked(&f, c)));
        let mut chunks = Channels::decode_chunks(&f, &[2, 0], 1..3);
        assert_eq!((chunks.len(), chunks[0].len(), chunks[0][1].len()), (2, 2, 777));
        assert_eq!(chunks[0][0].values::<u8>()[5], ((CHUNK_SAMPLES + 5) % 256) as u8);
        assert_eq!(chunks[1][1].values::<f64>()[3], ((2 * CHUNK_SAMPLES + 3) as f64 * 0.001).sin());
        let held = Held { first: 1, chunks: chunks[1].drain(..).map(|c| Some(std::sync::Arc::new(c))).collect() };
        let t = |i: usize| f.time_at(i);
        for vp in [
            Viewport { t0: t(CHUNK_SAMPLES - 300), t1: t(CHUNK_SAMPLES + 300), width_px: 100 },
            Viewport { t0: t(1000), t1: t(n - 1) + 1, width_px: 333 },
        ] {
            let (a, b) = (f.view(0, vp, None, &[], &held), f.trace(0, vp));
            assert_eq!(a.t, b.t);
            assert!(a.y.iter().zip(&b.y).all(|(p, q)| p.to_bits() == q.to_bits()));
        }
        assert_eq!(f.value_at(0, 2 * CHUNK_SAMPLES + 3, &held), f.value_at(0, 2 * CHUNK_SAMPLES + 3, &Held::default()));
    }

    #[test]
    fn a_native_one_field_file_is_not_chunked() {
        let desc = Descriptor {
            schema: 1,
            name: None,
            endian: Endian::Little,
            header_size: 0,
            record_size: 4,
            time: TimeSpec::Index,
            fields: vec![field("v", DType::F32, 0)],
        };
        let f = file(&desc, (0..10).map(|i| (i as f32).to_le_bytes().to_vec()));
        assert!(!Channels::chunked(&f, 0));
    }

    #[test]
    fn backwards_time_is_an_error() {
        let desc = Descriptor {
            schema: 1,
            name: None,
            endian: Endian::Little,
            header_size: 0,
            record_size: 16,
            time: TimeSpec::Field { field: "t".into(), unit: TimeUnit::Ns, t0_ns: 0 },
            fields: vec![field("t", DType::I64, 0), field("v", DType::F64, 8)],
        };
        let mut data = Vec::new();
        reference::write_header(&mut data, &desc).unwrap();
        for t in [0i64, 10, 5] {
            data.extend(t.to_le_bytes());
            data.extend(1.0f64.to_le_bytes());
        }
        let err = FixedWidth::from_reference_bytes(data).unwrap_err().to_string();
        assert!(err.contains("backwards at record 2"), "{err}");
    }

    #[test]
    fn truncated_header_is_an_error() {
        let desc = Descriptor {
            schema: 1,
            name: None,
            endian: Endian::Little,
            header_size: 0,
            record_size: 4,
            time: TimeSpec::Index,
            fields: vec![field("v", DType::F32, 0)],
        };
        let mut data = Vec::new();
        reference::write_header(&mut data, &desc).unwrap();
        data.truncate(40);
        assert!(FixedWidth::from_reference_bytes(data).is_err());
    }
}
