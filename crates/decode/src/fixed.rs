//! Fixed-width records read in place: record `i` is at `header + i * record_size`, so the file itself is the store
//! and nothing is copied or cached.

use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread::JoinHandle;

use rayon::prelude::*;
use tlmbench_core::pyramid::{self, Summary};
use tlmbench_core::{ChannelInfo, ChannelOp, Channels, RateTimes, RecordingInfo, Shifted, TimeSource, Times, TimesOp};

use crate::bytes::{self, Bytes, read_exact_at};
use crate::descriptor::{DType, Descriptor, Endian, TimeSpec};
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
}

/// Records `start..` read on another thread.
#[derive(Debug)]
struct ReadAhead {
    start: usize,
    end: usize,
    bytes: JoinHandle<Option<Vec<u8>>>,
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
        let field = &self.desc.fields[self.channels[channel]];
        let (records, stride, offset) = (self.records_bytes(), self.desc.record_size as usize, field.offset as usize);
        let big = self.desc.endian == Endian::Big;

        struct WithTimes<'a, O> {
            op: O,
            dtype: DType,
            records: &'a [u8],
            offset: usize,
            stride: usize,
            big: bool,
        }
        impl<O: ChannelOp> TimesOp for WithTimes<'_, O> {
            type Out = O::Out;
            fn run<T: Times + ?Sized>(self, times: &T) -> O::Out {
                fn run<V: Pod, O: ChannelOp, T: Times + ?Sized>(w: WithTimes<O>, times: &T) -> O::Out {
                    // contiguous, native order and aligned: read the map as a slice (the fast path)
                    if w.stride == V::SIZE
                        && w.offset == 0
                        && !w.big
                        && cfg!(target_endian = "little")
                        && let Ok(values) = bytemuck::try_cast_slice::<u8, V>(w.records)
                    {
                        return w.op.run(times, values);
                    }
                    let (records, offset, stride, big_endian) = (w.records, w.offset, w.stride, w.big);
                    w.op.run(times, &Strided::<V> { records, offset, stride, big_endian, _v: PhantomData })
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
        self.dispatch_times(WithTimes { op, dtype: field.dtype, records, offset, stride, big })
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
        let stride = self.desc.record_size as usize;
        let read = move |path: PathBuf, header: usize, a: usize, b: usize| {
            let mut buf = vec![0u8; (b - a) * stride];
            let f = std::fs::File::open(path).ok()?;
            read_exact_at(&f, &mut buf, (header + a * stride) as u64).ok().map(|()| buf)
        };
        let mut ahead = self.read_ahead.lock().unwrap();
        let buf = match ahead.take() {
            Some(r) if (r.start, r.end) == (start, end) => r.bytes.join().ok().flatten(),
            _ if matches!(self.bytes, Bytes::Mapped(_)) => read(self.info.path.clone(), self.header_len, start, end),
            _ => None,
        }
        .unwrap_or_else(|| self.records_bytes()[start * stride..end * stride].to_vec());
        // read the next block of the same size while this one is summarized
        let (next, next_end) = (end, (end + (end - start)).min(self.records));
        if matches!(self.bytes, Bytes::Mapped(_)) && next < next_end {
            let (path, header) = (self.info.path.clone(), self.header_len);
            let bytes = std::thread::spawn(move || read(path, header, next, next_end));
            *ahead = Some(ReadAhead { start: next, end: next_end, bytes });
        }
        drop(ahead);

        struct Op<'a> {
            me: &'a FixedWidth,
            buf: &'a [u8],
            start: usize,
        }
        impl TimesOp for Op<'_> {
            type Out = Vec<Vec<Summary>>;
            fn run<T: Times + ?Sized>(self, times: &T) -> Vec<Vec<Summary>> {
                fn one<V: Pod, T: Times + ?Sized>(
                    op: &Op,
                    times: &T,
                    field: &crate::descriptor::Field,
                ) -> Vec<Summary> {
                    let (stride, offset) = (op.me.desc.record_size as usize, field.offset as usize);
                    let big = op.me.desc.endian == Endian::Big;
                    let data: Vec<V> = op
                        .buf
                        .chunks_exact(stride)
                        .map(|r| {
                            let b = &r[offset..offset + V::SIZE];
                            if big { V::from_be(b) } else { V::from_le(b) }
                        })
                        .collect();
                    let n = data.len();
                    pyramid::summarize(times, &Shifted { offset: op.start, data: &data }, op.start, op.start + n)
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
        self.dispatch_times(Op { me: self, buf: &buf, start })
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
        assert_eq!(f.view(1, vp, Some(&pyr), &[]), f.trace(1, vp));
        let st = f.stats(1, &pyr, 100, 40_000);
        // the largest stored value (2000 + something at 4242) becomes the smallest shown one
        let shown: Vec<f64> = (100..40_000).map(|i| raw(i) as f64 * -0.5 + 10.0).collect();
        let lo = shown.iter().copied().fold(f64::INFINITY, f64::min);
        assert_eq!(st.min.map(|m| m.0), Some(lo));
        assert_eq!(st.min.map(|m| m.1), Some(4_242));
        assert_eq!(f.value_at(1, 4_242), raw(4_242) as f64 * -0.5 + 10.0);
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
