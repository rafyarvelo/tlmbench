//! Format descriptors: a TOML file that lays out fixed-width records, so a new binary layout is a config file,
//! not a recompile.
//!
//! Phase 1 covers fixed-width records: byte order, header size, record size, the time source, and per field a
//! name, type, byte offset, unit, scale and value offset. Bitfields, packet framing (sync words, length fields,
//! checksums) and plugin fields come later, as new keys.
//!
//! ```toml
//! schema = 1
//! name = "Bench logger"
//! endian = "little"      # little | big
//! header_size = 0        # bytes before the first record (ignored when the file's own header says)
//! record_size = 16
//!
//! [time]
//! source = "field"       # rate | field | index
//! field = "t"            # an integer field, not a channel
//! unit = "us"            # ns | us | ms | s
//! # source = "rate": rate_hz = 1000.0, t0_ns = 1793491200000000000
//!
//! [[fields]]
//! name = "t"
//! type = "u64"           # i8 i16 i32 i64 u8 u16 u32 u64 f32 f64
//! offset = 0
//!
//! [[fields]]
//! name = "battery_v"
//! type = "i16"
//! offset = 8
//! unit = "V"
//! scale = 0.001          # value = raw * scale + value_offset
//! ```

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const SCHEMA: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    pub schema: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub endian: Endian,
    #[serde(default)]
    pub header_size: u64,
    pub record_size: u64,
    pub time: TimeSpec,
    pub fields: Vec<Field>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Endian {
    #[default]
    Little,
    Big,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "lowercase", deny_unknown_fields)]
pub enum TimeSpec {
    Rate {
        rate_hz: f64,
        #[serde(default)]
        t0_ns: i64,
    },
    Field {
        field: String,
        #[serde(default)]
        unit: TimeUnit,
        #[serde(default)]
        t0_ns: i64,
    },
    Index,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeUnit {
    #[default]
    Ns,
    Us,
    Ms,
    S,
}

impl TimeUnit {
    pub fn ns(self) -> i64 {
        match self {
            TimeUnit::Ns => 1,
            TimeUnit::Us => 1_000,
            TimeUnit::Ms => 1_000_000,
            TimeUnit::S => 1_000_000_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub dtype: DType,
    pub offset: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub scale: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub value_offset: f64,
}

fn one() -> f64 {
    1.0
}

fn is_one(v: &f64) -> bool {
    *v == 1.0
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DType {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
}

impl DType {
    pub fn size(self) -> u64 {
        match self {
            DType::I8 | DType::U8 => 1,
            DType::I16 | DType::U16 => 2,
            DType::I32 | DType::U32 | DType::F32 => 4,
            DType::I64 | DType::U64 | DType::F64 => 8,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            DType::I8 => "i8",
            DType::I16 => "i16",
            DType::I32 => "i32",
            DType::I64 => "i64",
            DType::U8 => "u8",
            DType::U16 => "u16",
            DType::U32 => "u32",
            DType::U64 => "u64",
            DType::F32 => "f32",
            DType::F64 => "f64",
        }
    }

    pub fn is_float(self) -> bool {
        matches!(self, DType::F32 | DType::F64)
    }
}

impl Descriptor {
    /// Parses and validates a descriptor.
    pub fn parse(text: &str) -> Result<Self> {
        let d: Descriptor = toml::from_str(text).map_err(|e| Error::Descriptor(e.message().to_owned()))?;
        d.validate()?;
        Ok(d)
    }

    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("descriptors always serialize")
    }

    pub fn validate(&self) -> Result<()> {
        let bad = |msg: String| Err(Error::Descriptor(msg));
        if self.schema != SCHEMA {
            return bad(format!("schema {} is not supported (this version reads schema {SCHEMA})", self.schema));
        }
        if self.record_size == 0 || self.record_size > 1 << 20 {
            return bad(format!("record_size {} must be between 1 and 1048576 bytes", self.record_size));
        }
        if self.fields.is_empty() {
            return bad("no fields".into());
        }
        let mut names = HashSet::new();
        for f in &self.fields {
            if f.name.trim().is_empty() {
                return bad("a field has an empty name".into());
            }
            if !names.insert(f.name.as_str()) {
                return bad(format!("field {:?} appears twice", f.name));
            }
            if f.offset.checked_add(f.dtype.size()).is_none_or(|end| end > self.record_size) {
                return bad(format!(
                    "field {:?} ({} at offset {}) runs past the {}-byte record",
                    f.name,
                    f.dtype.name(),
                    f.offset,
                    self.record_size
                ));
            }
            if !f.scale.is_finite() || f.scale == 0.0 || !f.value_offset.is_finite() {
                return bad(format!("field {:?}: scale must be finite and non-zero, value_offset finite", f.name));
            }
        }
        match &self.time {
            TimeSpec::Rate { rate_hz, .. } if !(rate_hz.is_finite() && *rate_hz > 0.0) => {
                bad(format!("time.rate_hz {rate_hz} must be a positive number"))
            }
            TimeSpec::Field { field, .. } => match self.fields.iter().find(|f| &f.name == field) {
                None => bad(format!("time.field {field:?} is not one of the fields")),
                Some(f) if f.dtype.is_float() => bad(format!("time.field {field:?} must be an integer type")),
                Some(_) if self.fields.len() == 1 => bad("the only field is the time field: no channels".into()),
                Some(_) => Ok(()),
            },
            _ => Ok(()),
        }
    }

    /// The fields that are channels (all but the time field).
    pub fn channels(&self) -> impl Iterator<Item = (usize, &Field)> {
        let time = match &self.time {
            TimeSpec::Field { field, .. } => Some(field.as_str()),
            _ => None,
        };
        self.fields.iter().enumerate().filter(move |(_, f)| Some(f.name.as_str()) != time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"
        schema = 1
        name = "Bench logger"
        endian = "little"
        record_size = 16
        [time]
        source = "field"
        field = "t"
        unit = "us"
        [[fields]]
        name = "t"
        type = "u64"
        offset = 0
        [[fields]]
        name = "battery_v"
        type = "i16"
        offset = 8
        unit = "V"
        scale = 0.001
    "#;

    #[test]
    fn parses_and_round_trips() {
        let d = Descriptor::parse(EXAMPLE).unwrap();
        assert_eq!(d.time, TimeSpec::Field { field: "t".into(), unit: TimeUnit::Us, t0_ns: 0 });
        assert_eq!(d.channels().map(|(_, f)| f.name.as_str()).collect::<Vec<_>>(), ["battery_v"]);
        assert_eq!(Descriptor::parse(&d.to_toml()).unwrap(), d);
    }

    #[test]
    fn rejects_bad_layouts() {
        let cases = [
            ("offset = 8", "offset = 15", "runs past"),
            ("record_size = 16", "record_size = 0", "record_size"),
            ("type = \"u64\"", "type = \"f64\"", "integer"),
            ("field = \"t\"", "field = \"nope\"", "not one of the fields"),
            ("schema = 1", "schema = 2", "schema 2"),
            ("scale = 0.001", "scale = 0.0", "scale"),
            ("unit = \"us\"", "unit = \"fortnights\"", "unknown variant"),
            ("endian = \"little\"", "endian = \"little\"\ncolour = 3", "unknown field"),
        ];
        for (from, to, expect) in cases {
            let err = Descriptor::parse(&EXAMPLE.replacen(from, to, 1)).unwrap_err().to_string();
            assert!(err.contains(expect), "{to}: {err}");
        }
    }
}
