//! The Parquet and Arrow IPC writers: one record batch per block (a Parquet row group each, zstd), each channel in
//! its own type with its unit in the field's `unit` metadata.

use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow_array::{
    ArrayRef, Float32Array, Float64Array, Int8Array, Int16Array, Int32Array, Int64Array, RecordBatch, StringArray,
    TimestampNanosecondArray, UInt8Array, UInt16Array, UInt32Array, UInt64Array,
};
use arrow_schema::{DataType, Field, Schema, SchemaRef, TimeUnit};
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;
use tlmbench_core::Column;
use tlmbench_core::chunks::CHUNK_SAMPLES;

use crate::{Block, Error, Format, Layout, Plan, Result, Sink, TimeFormat, io_err};

enum Writer {
    Parquet(ArrowWriter<File>),
    Arrow(arrow_ipc::writer::FileWriter<File>),
}

pub(crate) struct ColumnarSink<'a> {
    writer: Writer,
    schema: SchemaRef,
    plan: &'a Plan,
    path: &'a Path,
}

fn err(e: impl std::fmt::Display) -> Error {
    Error::Write(e.to_string())
}

fn dtype(name: &str) -> DataType {
    match name {
        "i8" => DataType::Int8,
        "i16" => DataType::Int16,
        "i32" => DataType::Int32,
        "i64" => DataType::Int64,
        "u8" => DataType::UInt8,
        "u16" => DataType::UInt16,
        "u32" => DataType::UInt32,
        "u64" => DataType::UInt64,
        "f32" => DataType::Float32,
        _ => DataType::Float64,
    }
}

/// The time field: a UTC timestamp for dates (or when asked for ISO), nanoseconds for relative times, f64 seconds
/// when asked.
fn time_field(plan: &Plan) -> Option<Field> {
    let ts = || Field::new("time", DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())), false);
    Some(match plan.time? {
        TimeFormat::Iso => ts(),
        TimeFormat::Auto if plan.absolute => ts(),
        TimeFormat::Auto | TimeFormat::Ns => Field::new("time_ns", DataType::Int64, false),
        TimeFormat::Seconds => Field::new("time_s", DataType::Float64, false),
    })
}

fn array(c: &Column) -> ArrayRef {
    match c {
        Column::I8(v) => Arc::new(Int8Array::from(v.clone())),
        Column::I16(v) => Arc::new(Int16Array::from(v.clone())),
        Column::I32(v) => Arc::new(Int32Array::from(v.clone())),
        Column::I64(v) => Arc::new(Int64Array::from(v.clone())),
        Column::U8(v) => Arc::new(UInt8Array::from(v.clone())),
        Column::U16(v) => Arc::new(UInt16Array::from(v.clone())),
        Column::U32(v) => Arc::new(UInt32Array::from(v.clone())),
        Column::U64(v) => Arc::new(UInt64Array::from(v.clone())),
        Column::F32(v) => Arc::new(Float32Array::from(v.clone())),
        Column::F64(v) => Arc::new(Float64Array::from(v.clone())),
    }
}

impl<'a> ColumnarSink<'a> {
    pub(crate) fn create(path: &'a Path, plan: &'a Plan, format: Format) -> Result<Self> {
        let mut fields: Vec<Field> = time_field(plan).into_iter().collect();
        match plan.layout {
            Layout::Wide => fields.extend(plan.channels.iter().map(|c| {
                let f = Field::new(&c.name, dtype(c.dtype), true);
                match &c.unit {
                    Some(u) => f.with_metadata(HashMap::from([("unit".to_owned(), u.clone())])),
                    None => f,
                }
            })),
            Layout::Long => fields
                .extend([Field::new("channel", DataType::Utf8, false), Field::new("value", DataType::Float64, true)]),
        }
        let mut meta = HashMap::new();
        if let Some(note) = &plan.note {
            meta.insert("tlmbench".to_owned(), note.clone());
        }
        let schema = Arc::new(Schema::new_with_metadata(fields, meta));
        let file = File::create(path).map_err(io_err(path))?;
        let writer = match format {
            Format::Parquet => {
                let props = WriterProperties::builder()
                    .set_compression(Compression::ZSTD(ZstdLevel::default()))
                    .set_max_row_group_row_count(Some(CHUNK_SAMPLES))
                    .build();
                Writer::Parquet(ArrowWriter::try_new(file, schema.clone(), Some(props)).map_err(err)?)
            }
            Format::Arrow => Writer::Arrow(arrow_ipc::writer::FileWriter::try_new(file, &schema).map_err(err)?),
            Format::Csv => unreachable!("CSV has its own writer"),
        };
        Ok(Self { writer, schema, plan, path })
    }

    fn time_array(&self, t: &[i64]) -> Option<ArrayRef> {
        let f = time_field(self.plan)?;
        Some(match f.data_type() {
            DataType::Timestamp(..) => Arc::new(TimestampNanosecondArray::from(t.to_vec()).with_timezone("UTC")),
            DataType::Int64 => Arc::new(Int64Array::from(t.to_vec())),
            _ => Arc::new(Float64Array::from_iter_values(t.iter().map(|&t| t as f64 / 1e9))),
        })
    }
}

impl Sink for ColumnarSink<'_> {
    fn write(&mut self, block: &Block) -> Result<()> {
        let n = block.columns.first().map_or(0, Column::len);
        let mut arrays: Vec<ArrayRef> = Vec::new();
        match self.plan.layout {
            Layout::Wide => {
                if let Some(t) = &block.times {
                    arrays.extend(self.time_array(t));
                }
                arrays.extend(block.columns.iter().map(array));
            }
            Layout::Long => {
                let k = block.columns.len();
                if let Some(t) = &block.times {
                    let rep: Vec<i64> = t.iter().flat_map(|&t| std::iter::repeat_n(t, k)).collect();
                    arrays.extend(self.time_array(&rep));
                }
                let names = (0..n).flat_map(|_| self.plan.channels.iter().map(|c| c.name.as_str()));
                arrays.push(Arc::new(StringArray::from_iter_values(names)));
                let values = (0..n).flat_map(|i| block.columns.iter().map(move |c| c.get_f64(i)));
                arrays.push(Arc::new(Float64Array::from_iter_values(values)));
            }
        }
        let batch = RecordBatch::try_new(self.schema.clone(), arrays).map_err(err)?;
        match &mut self.writer {
            Writer::Parquet(w) => w.write(&batch).map_err(err),
            Writer::Arrow(w) => w.write(&batch).map_err(err),
        }
    }

    fn finish(self: Box<Self>) -> Result<()> {
        let path = self.path;
        match self.writer {
            Writer::Parquet(w) => w.close().map(drop).map_err(err),
            Writer::Arrow(mut w) => w.finish().map_err(err),
        }
        .map_err(|e| match e {
            Error::Write(m) => Error::Write(format!("{}: {m}", path.display())),
            e => e,
        })
    }
}
