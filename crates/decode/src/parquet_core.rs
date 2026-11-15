//! The pure part of the Parquet reader: which column is the time, which are channels, and how a record batch becomes
//! times and values. No files, threads or cache here, so the app's reader (`parquet.rs`) and the browser-only page's
//! (`crates/web-parquet`, which includes this file as source) read a Parquet file by the same rules.

use arrow_array::cast::AsArray;
use arrow_array::types::*;
use arrow_array::{Array, ArrayRef, ArrowPrimitiveType};
use arrow_schema::{DataType, Field, Schema, TimeUnit};

use crate::csv;
use crate::descriptor::DType;
use crate::error::Result;

/// How a column's values are read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Read {
    /// Stored as this type, nulls (floats only) as NaN.
    Native(DType),
    /// Integers with nulls: f64, nulls as NaN.
    Widened,
}

impl Read {
    pub(crate) fn dtype(self) -> DType {
        match self {
            Read::Native(d) => d,
            Read::Widened => DType::F64,
        }
    }
}

/// The time column and nanoseconds per unit of its values.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Time {
    pub index: usize,
    pub unit_ns: i64,
}

pub(crate) fn native(dt: &DataType) -> Option<DType> {
    Some(match dt {
        DataType::Int8 => DType::I8,
        DataType::Int16 => DType::I16,
        DataType::Int32 => DType::I32,
        DataType::Int64 => DType::I64,
        DataType::UInt8 | DataType::Boolean => DType::U8,
        DataType::UInt16 => DType::U16,
        DataType::UInt32 => DType::U32,
        DataType::UInt64 => DType::U64,
        DataType::Float16 | DataType::Float32 => DType::F32,
        DataType::Float64 => DType::F64,
        _ => return None,
    })
}

pub(crate) fn time_type_ns(dt: &DataType) -> Option<i64> {
    match dt {
        DataType::Timestamp(u, _) => Some(match u {
            TimeUnit::Second => 1_000_000_000,
            TimeUnit::Millisecond => 1_000_000,
            TimeUnit::Microsecond => 1_000,
            TimeUnit::Nanosecond => 1,
        }),
        DataType::Date32 => Some(86_400_000_000_000),
        DataType::Date64 => Some(1_000_000),
        _ => None,
    }
}

pub(crate) fn name_and_unit(f: &Field) -> (String, Option<String>) {
    let meta = f.metadata().get("unit").or_else(|| f.metadata().get("units")).filter(|u| !u.is_empty());
    match meta {
        Some(u) => (f.name().clone(), Some(u.clone())),
        None => {
            let (n, u) = csv::split_unit(f.name());
            (n.to_owned(), u)
        }
    }
}

/// What a file's columns become: the time column, the channels (and how each is read) and the columns left out.
#[derive(Debug)]
pub(crate) struct Plan {
    pub time: Option<Time>,
    pub channels: Vec<(usize, Read)>,
    pub skipped: Vec<String>,
}

/// Plans the read of `schema`. `has_nulls(i)` says whether column `i` may hold nulls (an integer one then widens to f64).
pub(crate) fn plan(schema: &Schema, has_nulls: impl Fn(usize) -> bool) -> Plan {
    let time = schema
        .fields()
        .iter()
        .position(|f| time_type_ns(f.data_type()).is_some())
        .map(|index| Time { index, unit_ns: time_type_ns(schema.field(index).data_type()).unwrap() })
        .or_else(|| {
            schema.fields().iter().enumerate().find_map(|(index, f)| {
                native(f.data_type())?;
                let (name, unit) = name_and_unit(f);
                csv::time_unit(&name, unit.as_deref()).map(|unit_ns| Time { index, unit_ns })
            })
        });
    let mut channels: Vec<(usize, Read)> = Vec::new();
    let mut skipped = Vec::new();
    for (i, f) in schema.fields().iter().enumerate() {
        if time.is_some_and(|t| t.index == i) {
            continue;
        }
        match native(f.data_type()) {
            Some(d) if !d.is_float() && has_nulls(i) => channels.push((i, Read::Widened)),
            Some(d) => channels.push((i, Read::Native(d))),
            None => skipped.push(f.name().clone()),
        }
    }
    Plan { time, channels, skipped }
}

/// One row group, decoded.
#[derive(Debug)]
pub(crate) struct Decoded {
    pub rows: usize,
    pub skipped: usize,
    pub t: Vec<i64>,
    /// Per channel, its values' little-endian bytes.
    pub columns: Vec<Vec<u8>>,
}

impl Decoded {
    pub(crate) fn new(n: usize) -> Self {
        Self { rows: 0, skipped: 0, t: Vec::new(), columns: vec![Vec::new(); n] }
    }
}

/// Appends the batch's times (nanoseconds) to `out.t` and returns which rows have one.
pub(crate) fn times(a: &ArrayRef, unit_ns: i64, out: &mut Decoded) -> Result<Vec<bool>> {
    let n = a.len();
    let keep: Vec<bool> = (0..n).map(|i| a.is_valid(i)).collect();
    macro_rules! ints {
        ($t:ty) => {{
            let v = a.as_primitive::<$t>().values();
            out.t.extend((0..n).filter(|&i| keep[i]).map(|i| (v[i] as i64).saturating_mul(unit_ns)));
        }};
    }
    macro_rules! floats {
        ($t:ty) => {{
            let v = a.as_primitive::<$t>().values();
            out.t.extend((0..n).filter(|&i| keep[i]).map(|i| (f64::from(v[i]) * unit_ns as f64).round() as i64));
        }};
    }
    match a.data_type() {
        DataType::Timestamp(TimeUnit::Second, _) => ints!(TimestampSecondType),
        DataType::Timestamp(TimeUnit::Millisecond, _) => ints!(TimestampMillisecondType),
        DataType::Timestamp(TimeUnit::Microsecond, _) => ints!(TimestampMicrosecondType),
        DataType::Timestamp(TimeUnit::Nanosecond, _) => ints!(TimestampNanosecondType),
        DataType::Date32 => ints!(Date32Type),
        DataType::Date64 => ints!(Date64Type),
        DataType::Int8 => ints!(Int8Type),
        DataType::Int16 => ints!(Int16Type),
        DataType::Int32 => ints!(Int32Type),
        DataType::Int64 => ints!(Int64Type),
        DataType::UInt8 => ints!(UInt8Type),
        DataType::UInt16 => ints!(UInt16Type),
        DataType::UInt32 => ints!(UInt32Type),
        DataType::UInt64 => ints!(UInt64Type),
        DataType::Float16 => {
            let v = a.as_primitive::<Float16Type>().values();
            out.t.extend((0..n).filter(|&i| keep[i]).map(|i| (v[i].to_f64() * unit_ns as f64).round() as i64));
        }
        DataType::Float32 => floats!(Float32Type),
        DataType::Float64 => floats!(Float64Type),
        DataType::Boolean => {
            let v = a.as_boolean();
            out.t.extend((0..n).filter(|&i| keep[i]).map(|i| i64::from(v.value(i)) * unit_ns));
        }
        _ => unreachable!("time columns are numeric or temporal"),
    }
    out.skipped += n - keep.iter().filter(|&&k| k).count();
    Ok(keep)
}

/// Appends the batch's values of one column (rows in `keep` only) to `out`, as its stored type's bytes.
pub(crate) fn values(a: &ArrayRef, read: Read, keep: Option<&Vec<bool>>, out: &mut Vec<u8>) {
    let n = a.len();
    let rows = || (0..n).filter(move |&i| keep.is_none_or(|k| k[i]));
    macro_rules! put {
        ($t:ty, $conv:expr) => {{
            let p = a.as_primitive::<$t>();
            let v = p.values();
            for i in rows() {
                let x = if p.is_null(i) { $conv(None) } else { $conv(Some(v[i])) };
                out.extend_from_slice(bytemuck::bytes_of(&x));
            }
        }};
    }
    macro_rules! native_or_widened {
        ($t:ty, $n:ty) => {
            match read {
                Read::Widened => put!($t, |x: Option<$n>| x.map_or(f64::NAN, |x| x as f64)),
                Read::Native(_) => put!($t, |x: Option<$n>| x.unwrap_or_default()),
            }
        };
    }
    match a.data_type() {
        DataType::Int8 => native_or_widened!(Int8Type, i8),
        DataType::Int16 => native_or_widened!(Int16Type, i16),
        DataType::Int32 => native_or_widened!(Int32Type, i32),
        DataType::Int64 => native_or_widened!(Int64Type, i64),
        DataType::UInt8 => native_or_widened!(UInt8Type, u8),
        DataType::UInt16 => native_or_widened!(UInt16Type, u16),
        DataType::UInt32 => native_or_widened!(UInt32Type, u32),
        DataType::UInt64 => native_or_widened!(UInt64Type, u64),
        DataType::Float16 => put!(Float16Type, |x: Option<<Float16Type as ArrowPrimitiveType>::Native>| x
            .map_or(f32::NAN, |x| x.to_f32())),
        DataType::Float32 => put!(Float32Type, |x: Option<f32>| x.unwrap_or(f32::NAN)),
        DataType::Float64 => put!(Float64Type, |x: Option<f64>| x.unwrap_or(f64::NAN)),
        DataType::Boolean => {
            let b = a.as_boolean();
            for i in rows() {
                let x: f64 = if b.is_null(i) { f64::NAN } else { f64::from(u8::from(b.value(i))) };
                match read {
                    Read::Widened => out.extend_from_slice(&x.to_le_bytes()),
                    Read::Native(_) => out.push(if b.is_null(i) { 0 } else { u8::from(b.value(i)) }),
                }
            }
        }
        _ => unreachable!("channels are numeric"),
    }
}
