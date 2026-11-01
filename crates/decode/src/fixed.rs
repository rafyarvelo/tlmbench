//! Fixed-width records read in place: record `i` is at `header + i * record_size`, so the file itself is the store
//! and nothing is copied or cached.

use std::marker::PhantomData;
use std::path::Path;

use tlmbench_core::{ChannelInfo, RateTimes, Recording, RecordingInfo, TimeSource, Times, Trace, Viewport, m4};

use crate::bytes::{self, Bytes};
use crate::descriptor::{DType, Descriptor, Endian, TimeSpec};
use crate::error::{Result, format_err, io_err};
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
        let desc = Descriptor::parse(&text).map_err(|e| format_err(descriptor, e.to_string()))?;
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
        let (first, last) = me
            .with_times(|t| {
                if let Some(i) = (1..t.len()).find(|&i| t.at(i) < t.at(i - 1)) {
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

impl Recording for FixedWidth {
    fn info(&self) -> &RecordingInfo {
        &self.info
    }

    fn trace(&self, channel: usize, vp: Viewport) -> Trace {
        let field = &self.desc.fields[self.channels[channel]];
        let records = self.records_bytes();
        let stride = self.desc.record_size as usize;
        let offset = field.offset as usize;
        let big_endian = self.desc.endian == Endian::Big;

        fn run<V: Pod>(
            times: &impl Times,
            records: &[u8],
            offset: usize,
            stride: usize,
            big: bool,
            vp: Viewport,
        ) -> Trace {
            // contiguous, native order and aligned: read the map as a slice (the fast path)
            if stride == V::SIZE
                && offset == 0
                && !big
                && cfg!(target_endian = "little")
                && let Ok(values) = bytemuck::try_cast_slice::<u8, V>(records)
            {
                return m4(times, values, vp);
            }
            m4(times, &Strided::<V> { records, offset, stride, big_endian: big, _v: PhantomData }, vp)
        }

        fn by_type(dtype: DType, times: &impl Times, r: &[u8], o: usize, s: usize, b: bool, vp: Viewport) -> Trace {
            match dtype {
                DType::I8 => run::<i8>(times, r, o, s, b, vp),
                DType::I16 => run::<i16>(times, r, o, s, b, vp),
                DType::I32 => run::<i32>(times, r, o, s, b, vp),
                DType::I64 => run::<i64>(times, r, o, s, b, vp),
                DType::U8 => run::<u8>(times, r, o, s, b, vp),
                DType::U16 => run::<u16>(times, r, o, s, b, vp),
                DType::U32 => run::<u32>(times, r, o, s, b, vp),
                DType::U64 => run::<u64>(times, r, o, s, b, vp),
                DType::F32 => run::<f32>(times, r, o, s, b, vp),
                DType::F64 => run::<f64>(times, r, o, s, b, vp),
            }
        }

        let mut trace = match &self.desc.time {
            TimeSpec::Rate { rate_hz, t0_ns } => {
                let t = RateTimes::from_rate(*t0_ns, *rate_hz, self.records);
                by_type(field.dtype, &t, records, offset, stride, big_endian, vp)
            }
            TimeSpec::Index => {
                let t = RateTimes::from_rate(0, 1.0, self.records);
                by_type(field.dtype, &t, records, offset, stride, big_endian, vp)
            }
            TimeSpec::Field { field: tname, unit, t0_ns } => {
                let tf = self.desc.fields.iter().find(|x| &x.name == tname).expect("validated");
                let t = FieldTimes {
                    records,
                    offset: tf.offset as usize,
                    stride,
                    big_endian,
                    dtype: tf.dtype,
                    scale_ns: unit.ns(),
                    t0_ns: *t0_ns,
                };
                by_type(field.dtype, &t, records, offset, stride, big_endian, vp)
            }
        };
        if field.scale != 1.0 || field.value_offset != 0.0 {
            for y in &mut trace.y {
                *y = (*y as f64 * field.scale + field.value_offset) as f32;
            }
        }
        trace
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::{Field, TimeUnit};
    use tlmbench_core::TraceKind;

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
