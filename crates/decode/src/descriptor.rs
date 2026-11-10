//! Format descriptors: a TOML file that lays out fixed-width records, so a new binary layout is a config file,
//! not a recompile.
//!
//! Fixed-width records: byte order, header size, record size, the time source, and per field a name, type, byte
//! offset, unit, scale and value offset. An optional `[packets]` table frames records in packets instead: a sync word,
//! a length field, a checksum and a compressed payload holding whole records (see [`Packets`]). A field can take a run of bits (`bits = { shift = 4, width = 3 }`), and the time can come from a
//! wrapping tick counter (`source = "counter"`). Plugin fields come later, as new keys.
//!
//! ```toml
//! schema = 1
//! name = "Bench logger"
//! endian = "little"      # little | big
//! header_size = 0        # bytes before the first record (ignored when the file's own header says)
//! record_size = 16
//!
//! sequence = "seq"      # optional: an integer field counting frames, so a live source can count lost ones
//!
//! [time]
//! source = "field"       # rate | field | counter | index
//! field = "t"            # an integer field, not a channel
//! unit = "us"            # ns | us | ms | s
//! # source = "rate": rate_hz = 1000.0, t0_ns = 1793491200000000000
//! # source = "counter": field = "ticks", clock_hz = 1000000.0, wraps_at = 65536 (default: 2^bits of the field)
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
    /// The files this descriptor decodes, by name (`*` matches any text): `["esc_bench_*.bin"]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    #[serde(default)]
    pub endian: Endian,
    #[serde(default)]
    pub header_size: u64,
    /// Left out when `[[packet_types]]` give each type its own.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub record_size: u64,
    /// An integer field that counts records up by one each, for live sources: a jump is that many records lost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<String>,
    /// Left out (index) when `[[packet_types]]` give each type its own.
    #[serde(default, skip_serializing_if = "TimeSpec::is_index")]
    pub time: TimeSpec,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<Field>,
    /// Records framed in packets; field offsets then count from the packet's start (its header, then the record).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packets: Option<Packets>,
    /// Packets of several kinds (`[packets] id` says which): each opens as its own recording, with its own record
    /// layout.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packet_types: Vec<PacketType>,
}

/// One kind of packet in a file with an `id` field: `[[packet_types]]`, each with the keys a descriptor has for its
/// records.
///
/// ```toml
/// [[packet_types]]
/// id = 2
/// name = "gps"
/// record_size = 24
/// time = { source = "field", field = "t", unit = "us" }
/// fields = [ { name = "t", type = "u64", offset = 8 }, { name = "lat", type = "f64", offset = 16 } ]
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PacketType {
    pub id: i64,
    pub name: String,
    pub record_size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<String>,
    pub time: TimeSpec,
    pub fields: Vec<Field>,
}

/// The packet field that says what kind a packet is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdField {
    pub offset: u64,
    #[serde(rename = "type")]
    pub dtype: DType,
}

/// One packet type of a descriptor, as a recording opens it.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub id: i64,
    pub name: String,
    pub field: IdField,
    /// Every type's id and record size (to frame packets with no length field, and to know ids no type claims).
    pub known: Vec<(i64, u64)>,
    /// The first type: it counts the packets no type claims.
    pub first: bool,
}

/// How records are framed in packets:
///
/// ```toml
/// [packets]
/// sync = "EB90"          # hex bytes each packet starts with (optional, but without it a damaged packet ends the
///                        # file: there is nothing to find the next one by)
/// header_size = 8        # bytes before the payload, sync included
/// length = { offset = 2, type = "u16", counts = "payload" }   # or counts = "packet" (header + payload + checksum);
///                        # without a length, each packet holds one record (fixed-size frames, uncompressed)
/// id = { offset = 4, type = "u8" }   # with [[packet_types]]: each packet's type
/// checksum = "crc16-ccitt"   # none | sum8 | crc16-ccitt | crc32, after the payload, over header and payload
/// compression = "zstd"   # none | zstd | lz4 (frame) | gzip, of the payload
/// ```
///
/// The payload (decompressed) holds whole records of `record_size` bytes. Each record is read as the packet header
/// followed by the record, so fields at offsets below `header_size` read header bytes (a packet time or counter).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Packets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync: Option<String>,
    pub header_size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<LengthField>,
    /// The field holding a packet's type, matching a `[[packet_types]]` id (an integer inside the header).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<IdField>,
    #[serde(default)]
    pub checksum: Checksum,
    #[serde(default)]
    pub compression: Compression,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LengthField {
    pub offset: u64,
    #[serde(rename = "type")]
    pub dtype: DType,
    #[serde(default)]
    pub counts: Counts,
    /// Added to the field's value (a length that leaves out or counts extra bytes).
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub adjust: i64,
}

fn is_zero_i64(v: &i64) -> bool {
    *v == 0
}

fn is_zero_u64(v: &u64) -> bool {
    *v == 0
}

/// What a length field counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Counts {
    /// The payload's stored (compressed) bytes.
    #[default]
    Payload,
    /// The whole packet: header, payload and checksum.
    Packet,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Checksum {
    #[default]
    None,
    /// The low byte of the sum of the bytes.
    Sum8,
    /// CRC-16/CCITT-FALSE (polynomial 0x1021, initial 0xFFFF).
    Crc16Ccitt,
    /// CRC-32 (IEEE 802.3, as zlib's).
    Crc32,
}

impl Checksum {
    pub fn size(self) -> u64 {
        match self {
            Checksum::None => 0,
            Checksum::Sum8 => 1,
            Checksum::Crc16Ccitt => 2,
            Checksum::Crc32 => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Compression {
    #[default]
    None,
    Zstd,
    /// LZ4 frames.
    Lz4,
    Gzip,
}

impl Packets {
    /// The sync word's bytes.
    pub fn sync_bytes(&self) -> Result<Vec<u8>> {
        let Some(s) = &self.sync else { return Ok(Vec::new()) };
        let hex = s.trim().trim_start_matches("0x").trim_start_matches("0X").replace([' ', '_'], "");
        if hex.is_empty() || hex.len() % 2 != 0 || hex.len() > 16 {
            return Err(Error::Descriptor(format!("packets.sync {s:?} must be 1 to 8 bytes of hex")));
        }
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
            .collect::<std::result::Result<_, _>>()
            .map_err(|_| Error::Descriptor(format!("packets.sync {s:?} is not hex")))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Endian {
    #[default]
    Little,
    Big,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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
    /// A tick counter that wraps: `raw + wraps_at × (times it has wrapped)` ticks of `clock_hz`. A drop of more than
    /// half the range is a wrap; a smaller one is time going backwards.
    Counter {
        field: String,
        clock_hz: f64,
        /// The counter's range; left out it is 2 to the power of the field's bits.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        wraps_at: Option<u64>,
        #[serde(default)]
        t0_ns: i64,
    },
    #[default]
    Index,
}

impl TimeSpec {
    /// The field the time is read from, when it is one.
    pub fn field(&self) -> Option<&str> {
        match self {
            TimeSpec::Field { field, .. } | TimeSpec::Counter { field, .. } => Some(field),
            _ => None,
        }
    }

    fn is_index(&self) -> bool {
        *self == TimeSpec::Index
    }
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
    /// Only some bits of the stored integer: `bits = { shift = 4, width = 3 }` reads bits 4 to 6 (counting from the
    /// least significant), as an unsigned number, or sign-extended from `width` bits when the type is signed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bits: Option<Bits>,
}

/// A run of bits inside an integer field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bits {
    /// How many bits up from the least significant one the run starts.
    pub shift: u32,
    pub width: u32,
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
    pub fn bits(self) -> u32 {
        self.size() as u32 * 8
    }

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

    /// Reads and parses a descriptor file; errors name the file.
    pub fn load(path: &std::path::Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(crate::error::io_err(path))?;
        Self::parse(&text).map_err(|e| match e {
            Error::Descriptor(msg) => Error::Descriptor(format!("{}: {msg}", path.display())),
            other => other,
        })
    }

    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("descriptors always serialize")
    }

    pub fn validate(&self) -> Result<()> {
        let bad = |msg: String| Err(Error::Descriptor(msg));
        if self.schema != SCHEMA {
            return bad(format!("schema {} is not supported (this version reads schema {SCHEMA})", self.schema));
        }
        let header = match &self.packets {
            None => 0,
            Some(p) => {
                let sync = p.sync_bytes()?;
                if p.header_size > 1 << 16 || (sync.len() as u64) > p.header_size {
                    return bad(format!(
                        "packets.header_size {} must hold the sync word and be at most 65536",
                        p.header_size
                    ));
                }
                let inside = |offset: u64, dtype: DType| {
                    !dtype.is_float() && offset.checked_add(dtype.size()).is_some_and(|e| e <= p.header_size)
                };
                match &p.length {
                    Some(l) if !inside(l.offset, l.dtype) => {
                        return bad(format!(
                            "packets.length must be an integer field inside the {}-byte header",
                            p.header_size
                        ));
                    }
                    None if p.compression != Compression::None => {
                        return bad("packets.compression needs packets.length: compressed payloads vary in size".into());
                    }
                    _ => {}
                }
                if let Some(id) = &p.id
                    && !inside(id.offset, id.dtype)
                {
                    return bad(format!(
                        "packets.id must be an integer field inside the {}-byte header",
                        p.header_size
                    ));
                }
                p.header_size
            }
        };
        if self.packet_types.is_empty() {
            if self.packets.as_ref().is_some_and(|p| p.id.is_some()) {
                return bad("packets.id needs [[packet_types]]".into());
            }
            return validate_layout(self.record_size, header, &self.fields, &self.time, self.sequence.as_deref(), "");
        }
        if self.packets.as_ref().is_none_or(|p| p.id.is_none()) {
            return bad("[[packet_types]] need [packets] with an id field".into());
        }
        if self.record_size != 0 || !self.fields.is_empty() || self.sequence.is_some() || !self.time.is_index() {
            return bad("with [[packet_types]], record_size, time, sequence and fields go in each type".into());
        }
        let (mut ids, mut names) = (HashSet::new(), HashSet::new());
        for t in &self.packet_types {
            if t.name.trim().is_empty() {
                return bad(format!("the packet type with id {} has no name", t.id));
            }
            if !ids.insert(t.id) {
                return bad(format!("packet id {} appears twice", t.id));
            }
            if !names.insert(t.name.as_str()) {
                return bad(format!("packet type {:?} appears twice", t.name));
            }
            validate_layout(
                t.record_size,
                header,
                &t.fields,
                &t.time,
                t.sequence.as_deref(),
                &format!("{:?}: ", t.name),
            )?;
        }
        Ok(())
    }

    /// One descriptor per packet type, each with the type's own record layout and the part that picks its packets
    /// (a descriptor without `[[packet_types]]` is its own single part).
    pub fn parts(&self) -> Vec<(Descriptor, Option<Part>)> {
        let Some(field) = self.packets.as_ref().and_then(|p| p.id) else { return vec![(self.clone(), None)] };
        let known: Vec<(i64, u64)> = self.packet_types.iter().map(|t| (t.id, t.record_size)).collect();
        self.packet_types
            .iter()
            .enumerate()
            .map(|(k, t)| {
                let mut d = self.clone();
                d.packet_types.clear();
                (d.record_size, d.time, d.sequence, d.fields) =
                    (t.record_size, t.time.clone(), t.sequence.clone(), t.fields.clone());
                let part = Part { id: t.id, name: t.name.clone(), field, known: known.clone(), first: k == 0 };
                (d, Some(part))
            })
            .collect()
    }

    /// The fields that are channels (all but the time field).
    pub fn channels(&self) -> impl Iterator<Item = (usize, &Field)> {
        let time = self.time.field();
        self.fields.iter().enumerate().filter(move |(_, f)| Some(f.name.as_str()) != time)
    }
}

/// Checks one record layout: `header` bytes (a packet's) then `record_size` bytes holding `fields`.
fn validate_layout(
    record_size: u64,
    header: u64,
    fields: &[Field],
    time: &TimeSpec,
    sequence: Option<&str>,
    who: &str,
) -> Result<()> {
    let bad = |msg: String| Err(Error::Descriptor(format!("{who}{msg}")));
    if record_size == 0 || record_size > 1 << 20 {
        return bad(format!("record_size {record_size} must be between 1 and 1048576 bytes"));
    }
    // in a packet file a record is read as the packet header, then the record
    let extent = record_size + header;
    if fields.is_empty() {
        return bad("no fields".into());
    }
    let mut names = HashSet::new();
    for f in fields {
        if f.name.trim().is_empty() {
            return bad("a field has an empty name".into());
        }
        if !names.insert(f.name.as_str()) {
            return bad(format!("field {:?} appears twice", f.name));
        }
        if f.offset.checked_add(f.dtype.size()).is_none_or(|end| end > extent) {
            let within = if header > 0 { "packet header and record" } else { "record" };
            return bad(format!(
                "field {:?} ({} at offset {}) runs past the {extent}-byte {within}",
                f.name,
                f.dtype.name(),
                f.offset,
            ));
        }
        if !f.scale.is_finite() || f.scale == 0.0 || !f.value_offset.is_finite() {
            return bad(format!("field {:?}: scale must be finite and non-zero, value_offset finite", f.name));
        }
        if let Some(b) = f.bits {
            if f.dtype.is_float() {
                return bad(format!("field {:?}: bits need an integer type", f.name));
            }
            if b.width == 0 || b.shift.checked_add(b.width).is_none_or(|end| end > f.dtype.bits()) {
                return bad(format!(
                    "field {:?}: bits shift {} and width {} don't fit in {} bits",
                    f.name,
                    b.shift,
                    b.width,
                    f.dtype.bits()
                ));
            }
            if sequence == Some(f.name.as_str()) {
                return bad(format!("sequence {:?} can't be a bitfield", f.name));
            }
        }
    }
    if let Some(seq) = sequence {
        match fields.iter().find(|f| f.name == seq) {
            None => return bad(format!("sequence {seq:?} is not one of the fields")),
            Some(f) if f.dtype.is_float() => return bad(format!("sequence {seq:?} must be an integer type")),
            Some(_) => {}
        }
    }
    match time {
        TimeSpec::Rate { rate_hz, .. } if !(rate_hz.is_finite() && *rate_hz > 0.0) => {
            bad(format!("time.rate_hz {rate_hz} must be a positive number"))
        }
        TimeSpec::Field { field, .. } | TimeSpec::Counter { field, .. } => {
            match fields.iter().find(|f| &f.name == field) {
                None => bad(format!("time.field {field:?} is not one of the fields")),
                Some(f) if f.dtype.is_float() => bad(format!("time.field {field:?} must be an integer type")),
                Some(_) if fields.len() == 1 => bad("the only field is the time field: no channels".into()),
                Some(f) => match time {
                    TimeSpec::Counter { clock_hz, .. } if !(clock_hz.is_finite() && *clock_hz > 0.0) => {
                        bad(format!("time.clock_hz {clock_hz} must be a positive number"))
                    }
                    TimeSpec::Counter { wraps_at, .. } => match counter_range(f, *wraps_at) {
                        Some(_) => Ok(()),
                        None => bad(format!(
                            "time.wraps_at for {field:?} must be between 2 and 2^63 (a 64-bit counter needs it)"
                        )),
                    },
                    _ => Ok(()),
                },
            }
        }
        _ => Ok(()),
    }
}

/// A counter field's range: `wraps_at`, else 2 to the power of its bits; `None` when that is under 2 or over 2^63.
pub fn counter_range(f: &Field, wraps_at: Option<u64>) -> Option<i64> {
    let range = match wraps_at {
        Some(r) => r,
        None => {
            let bits = f.bits.map_or(f.dtype.bits(), |b| b.width);
            if bits >= 63 {
                return None;
            }
            1u64 << bits
        }
    };
    (2..=1u64 << 63).contains(&range).then(|| i64::try_from(range).unwrap_or(i64::MAX))
}

/// A sync word that repeats at a fixed distance: what "Find frames" in the descriptor form proposes.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Frames {
    /// The sync word, as upper-case hex (`A55A`).
    pub sync: String,
    /// Bytes from one sync word to the next: the packet's whole length.
    pub stride: usize,
    /// Where the first one starts (bytes before it belong to a file header).
    pub start: usize,
}

/// Looks in the first bytes of a file for a 2- or 4-byte word that comes back at the same distance at least six
/// times in a row (the first such word, the one starting earliest). `None` when there is none.
pub fn find_frames(head: &[u8]) -> Option<Frames> {
    const REPEATS: usize = 6;
    let head = &head[..head.len().min(64 << 10)];
    for start in 0..head.len().min(512) {
        for len in [4usize, 2] {
            let Some(word) = head.get(start..start + len) else { continue };
            // a word of one repeated byte (zeros, 0xFF) is padding, not a sync word
            if word.iter().all(|&b| b == word[0]) {
                continue;
            }
            let Some(next) = memchr::memmem::find(&head[start + len..], word) else { continue };
            let stride = len + next;
            if stride < len + 2 {
                continue;
            }
            if (1..REPEATS).all(|k| head.get(start + k * stride..start + k * stride + len) == Some(word)) {
                return Some(Frames { sync: word.iter().map(|b| format!("{b:02X}")).collect(), stride, start });
            }
        }
    }
    None
}

/// True when `name` matches `pattern`, where `*` stands for any text (including none). Names are compared
/// as written (case matters).
pub fn name_matches(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    let (first, rest) = parts.split_first().expect("split yields at least one part");
    let Some(mut tail) = name.strip_prefix(first) else { return false };
    let Some((last, middle)) = rest.split_last() else { return tail.is_empty() };
    for m in middle {
        match tail.find(m) {
            Some(i) => tail = &tail[i + m.len()..],
            None => return false,
        }
    }
    tail.len() >= last.len() && tail.ends_with(last)
}

/// A descriptor file the app knows: one in a folder's `.tlmbench/decoders/` (shared with the folder) or in the app
/// cache's `decoders/` (this machine's).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Known {
    /// The file's name without `.toml`.
    pub id: String,
    pub path: std::path::PathBuf,
    /// In the data's folder rather than the app cache.
    pub shared: bool,
    pub descriptor: Descriptor,
}

static CONFIG_DIRS: std::sync::RwLock<Vec<std::path::PathBuf>> = std::sync::RwLock::new(Vec::new());

/// Sets the `.tlmbench/` folders decoders are read from instead of the nearest one (`--config`; later folders win).
/// An empty list goes back to the nearest `.tlmbench/` above the file.
pub fn set_config_dirs(dirs: Vec<std::path::PathBuf>) {
    *CONFIG_DIRS.write().unwrap() = dirs;
}

/// The `decoders/` folders to look in for a file in `dir`, best first: the `--config` ones (last given first), or the
/// nearest `.tlmbench/` at or above `dir`.
fn shared_decoder_dirs(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let config = CONFIG_DIRS.read().unwrap();
    if !config.is_empty() {
        return config.iter().rev().map(|d| d.join("decoders")).collect();
    }
    // paths are kept as given (a canonical one would differ on macOS: /var is /private/var)
    let abs =
        if dir.is_absolute() { dir.to_owned() } else { std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_owned()) };
    abs.ancestors().map(|d| d.join(".tlmbench")).find(|d| d.is_dir()).map(|d| d.join("decoders")).into_iter().collect()
}

/// The descriptors in the shared `.tlmbench/decoders/` for files in `dir` (the nearest one above, or `--config`) and
/// then in the app cache's `decoders/` (`cache_root`), each in name order. A file that doesn't parse is skipped.
pub fn known(dir: &std::path::Path, cache_root: &std::path::Path) -> Vec<Known> {
    let mut out = Vec::new();
    let mut folders: Vec<(std::path::PathBuf, bool)> =
        shared_decoder_dirs(dir).into_iter().map(|d| (d, true)).collect();
    folders.push((cache_root.join("decoders"), false));
    for (folder, shared) in folders {
        let Ok(rd) = std::fs::read_dir(&folder) else { continue };
        let mut files: Vec<_> =
            rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "toml")).collect();
        files.sort_by_key(|p| p.file_stem().map(std::ffi::OsStr::to_owned));
        for path in files {
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let Ok(descriptor) = Descriptor::parse(&text) else { continue };
            let id = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            out.push(Known { id, path, shared, descriptor });
        }
    }
    out
}

/// The descriptor for a file, found by the `files = [...]` patterns of the descriptors in the folder's
/// `.tlmbench/decoders/` and then in the app cache's `decoders/` (`cache_root`): the first match in name order,
/// the folder's before the cache's. A descriptor that doesn't parse is skipped.
pub fn discover(path: &std::path::Path, cache_root: &std::path::Path) -> Option<std::path::PathBuf> {
    let name = path.file_name()?.to_str()?;
    known(path.parent()?, cache_root)
        .into_iter()
        .find(|k| k.descriptor.files.iter().any(|p| name_matches(p, name)))
        .map(|k| k.path)
}

/// A file name for a descriptor called `name`: lower case, letters and digits joined by dashes (`esc-telemetry`).
pub fn slug(name: &str) -> String {
    let s: String = name
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if s.is_empty() { "decoder".into() } else { s }
}

/// Writes `d` to the app cache's `decoders/` (never beside the data) and returns where it went. `replace` is the id
/// of a descriptor being edited: it is written over (and renamed when the name changed, the old file removed);
/// otherwise a name that is taken gets a number.
pub fn save(d: &Descriptor, replace: Option<&str>, cache_root: &std::path::Path) -> Result<Known> {
    d.validate()?;
    let dir = cache_root.join("decoders");
    std::fs::create_dir_all(&dir).map_err(crate::error::io_err(&dir))?;
    let base = slug(d.name.as_deref().unwrap_or(""));
    let bare = |id: &str| !id.is_empty() && !id.contains(['/', '\\']) && !id.starts_with('.');
    let id = match replace.filter(|r| bare(r)) {
        // an edited descriptor keeps its file unless it was renamed
        Some(r)
            if dir.join(format!("{r}.toml")).exists() && slug_of_file(&dir, r).as_deref() == Some(base.as_str()) =>
        {
            r.to_owned()
        }
        other => {
            let mut id = base.clone();
            let mut n = 2;
            while dir.join(format!("{id}.toml")).exists() && other != Some(id.as_str()) {
                id = format!("{base}-{n}");
                n += 1;
            }
            id
        }
    };
    let path = dir.join(format!("{id}.toml"));
    std::fs::write(&path, d.to_toml()).map_err(crate::error::io_err(&path))?;
    if let Some(old) = replace.filter(|r| bare(r) && *r != id) {
        let _ = std::fs::remove_file(dir.join(format!("{old}.toml")));
    }
    Ok(Known { id, path, shared: false, descriptor: d.clone() })
}

/// The slug of the name inside descriptor file `id` in `dir`.
fn slug_of_file(dir: &std::path::Path, id: &str) -> Option<String> {
    let d = Descriptor::load(&dir.join(format!("{id}.toml"))).ok()?;
    Some(slug(d.name.as_deref().unwrap_or("")))
}

/// Share…: writes `d` to `folder/.tlmbench/decoders/<name>.toml` so everyone with the folder decodes its files the
/// same way. Only ever called for a user's action; returns the file written.
pub fn share(d: &Descriptor, folder: &std::path::Path) -> Result<std::path::PathBuf> {
    d.validate()?;
    let dir = folder.join(".tlmbench").join("decoders");
    std::fs::create_dir_all(&dir).map_err(crate::error::io_err(&dir))?;
    let path = dir.join(format!("{}.toml", slug(d.name.as_deref().unwrap_or(""))));
    std::fs::write(&path, d.to_toml()).map_err(crate::error::io_err(&path))?;
    Ok(path)
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
    fn frames_are_found_by_their_repeating_sync_word() {
        let mut file = vec![0u8; 8]; // a file header
        for k in 0..12u8 {
            file.extend([0xA5, 0x5A]);
            file.extend((0..22).map(|i| k.wrapping_mul(7).wrapping_add(i * 3 + 1)));
        }
        let f = find_frames(&file).unwrap();
        assert_eq!((f.sync.as_str(), f.stride, f.start), ("A55A", 24, 8));
        // too few repeats, or only padding: nothing
        assert_eq!(find_frames(&file[..8 + 24 * 4]), None);
        assert_eq!(find_frames(&[0u8; 4096]), None);
        assert_eq!(find_frames(&[]), None);
    }

    #[test]
    fn name_patterns() {
        assert!(name_matches("esc_*.bin", "esc_bench_01.bin"));
        assert!(name_matches("*.bin", ".bin"));
        assert!(name_matches("a*b*c", "a--b--c"));
        assert!(name_matches("flight.bin", "flight.bin"));
        assert!(!name_matches("esc_*.bin", "esc_bench_01.bin.gz"));
        assert!(!name_matches("esc_*.bin", "my_esc_1.bin"));
        assert!(!name_matches("a*b*c", "a--c"));
        assert!(!name_matches("ab*ba", "aba"));
    }

    #[test]
    fn descriptors_are_found_by_pattern_in_the_folder_then_the_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("rec");
        let (local, cache) = (data.join(".tlmbench/decoders"), tmp.path().join("cache"));
        std::fs::create_dir_all(&local).unwrap();
        std::fs::create_dir_all(cache.join("decoders")).unwrap();
        let toml = |pat: &str| EXAMPLE.replacen("endian", &format!("files = [\"{pat}\"]\nendian"), 1);
        std::fs::write(cache.join("decoders/a.toml"), toml("*.bin")).unwrap();
        std::fs::write(local.join("z.toml"), toml("esc_*.bin")).unwrap();
        std::fs::write(local.join("bad.toml"), "not toml [").unwrap();
        let file = |n: &str| data.join(n);
        assert_eq!(discover(&file("esc_1.bin"), &cache), Some(local.join("z.toml")));
        assert_eq!(discover(&file("other.bin"), &cache), Some(cache.join("decoders/a.toml")));
        assert_eq!(discover(&file("other.dat"), &cache), None);
    }

    #[test]
    fn descriptors_are_saved_to_the_cache_and_shared_to_the_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let (data, cache) = (tmp.path().join("rec"), tmp.path().join("cache"));
        std::fs::create_dir_all(&data).unwrap();
        assert_eq!(slug("ESC telemetry (v2)"), "esc-telemetry-v2");
        assert_eq!(slug("  "), "decoder");
        let d = Descriptor::parse(EXAMPLE).unwrap();
        let a = save(&d, None, &cache).unwrap();
        assert_eq!((a.id.as_str(), a.shared), ("bench-logger", false));
        assert!(a.path.starts_with(cache.join("decoders")));
        // the same name again is a different descriptor; editing one writes over it
        assert_eq!(save(&d, None, &cache).unwrap().id, "bench-logger-2");
        let mut edited = d.clone();
        edited.files = vec!["*.bin".into()];
        assert_eq!(save(&edited, Some("bench-logger"), &cache).unwrap().id, "bench-logger");
        assert_eq!(
            known(&data, &cache).iter().map(|k| k.id.as_str()).collect::<Vec<_>>(),
            ["bench-logger", "bench-logger-2"]
        );
        // a rename moves the file
        edited.name = Some("Renamed".into());
        assert_eq!(save(&edited, Some("bench-logger"), &cache).unwrap().id, "renamed");
        assert!(!cache.join("decoders/bench-logger.toml").exists());
        // only share() writes into the folder, and it's found there before the cache's
        assert!(!data.join(".tlmbench").exists());
        let shared = share(&edited, &data).unwrap();
        assert_eq!(shared, data.join(".tlmbench/decoders/renamed.toml"));
        let k = known(&data, &cache);
        assert_eq!((k[0].shared, k[0].id.as_str()), (true, "renamed"));
        assert_eq!(discover(&data.join("x.bin"), &cache), Some(shared));
        // an invalid descriptor is refused and writes nothing
        let mut bad = d.clone();
        bad.fields[1].offset = 15;
        assert!(save(&bad, None, &cache).is_err() && share(&bad, &data).is_err());
    }

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
            ("record_size = 16", "record_size = 16\nsequence = \"nope\"", "sequence \"nope\""),
        ];
        for (from, to, expect) in cases {
            let err = Descriptor::parse(&EXAMPLE.replacen(from, to, 1)).unwrap_err().to_string();
            assert!(err.contains(expect), "{to}: {err}");
        }
    }

    const PACKETS: &str = "\n[packets]\nsync = \"EB90\"\nheader_size = 12\nlength = { offset = 2, type = \"u16\" }\n\
                           checksum = \"crc16-ccitt\"\ncompression = \"zstd\"\n";

    #[test]
    fn packets_parse_and_widen_field_offsets() {
        let d = Descriptor::parse(&(EXAMPLE.replacen("offset = 8", "offset = 20", 1) + PACKETS)).unwrap();
        let p = d.packets.as_ref().unwrap();
        assert_eq!(p.sync_bytes().unwrap(), [0xeb, 0x90]);
        assert_eq!(
            (p.checksum, p.compression, p.length.as_ref().unwrap().counts),
            (Checksum::Crc16Ccitt, Compression::Zstd, Counts::Payload)
        );
        assert_eq!(Descriptor::parse(&d.to_toml()).unwrap(), d);
        let with_files = Descriptor::parse(&EXAMPLE.replacen("endian", "files = [\"esc_*.bin\"]\nendian", 1)).unwrap();
        assert_eq!(with_files.files, ["esc_*.bin"]);
        // 12 + 16 bytes: offset 26 + 2 fits, 27 doesn't
        assert!(Descriptor::parse(&(EXAMPLE.replacen("offset = 8", "offset = 26", 1) + PACKETS)).is_ok());
        let err = Descriptor::parse(&(EXAMPLE.replacen("offset = 8", "offset = 27", 1) + PACKETS)).unwrap_err();
        assert!(err.to_string().contains("packet header and record"), "{err}");
        for (from, to, expect) in [
            ("sync = \"EB90\"", "sync = \"EB9\"", "hex"),
            ("offset = 2, type = \"u16\"", "offset = 11, type = \"u16\"", "inside the 12-byte header"),
            ("\"zstd\"", "\"brotli\"", "unknown variant"),
            ("length = { offset = 2, type = \"u16\" }\n", "", "needs packets.length"),
        ] {
            let err =
                Descriptor::parse(&(EXAMPLE.to_owned() + &PACKETS.replacen(from, to, 1))).unwrap_err().to_string();
            assert!(err.contains(expect), "{to}: {err}");
        }
    }

    #[test]
    fn packet_types_validate_and_split() {
        const TYPES: &str = "schema = 1\n[packets]\nsync = \"EB90\"\nheader_size = 8\nid = { offset = 4, type = \"u8\" }\n\
            [[packet_types]]\nid = 1\nname = \"a\"\nrecord_size = 4\ntime = { source = \"index\" }\n\
            fields = [ { name = \"x\", type = \"u32\", offset = 8 } ]\n\
            [[packet_types]]\nid = 2\nname = \"b\"\nrecord_size = 2\n\
            time = { source = \"rate\", rate_hz = 10.0 }\nfields = [ { name = \"y\", type = \"u16\", offset = 8 } ]\n";
        let d = Descriptor::parse(TYPES).unwrap();
        assert_eq!(Descriptor::parse(&d.to_toml()).unwrap(), d);
        let parts = d.parts();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[1].0.record_size, 2);
        assert_eq!(parts[1].1.as_ref().unwrap().known, [(1, 4), (2, 2)]);
        assert!(parts[0].1.as_ref().unwrap().first && !parts[1].1.as_ref().unwrap().first);
        for (from, to, expect) in [
            ("id = 2", "id = 1", "appears twice"),
            ("name = \"b\"", "name = \"a\"", "\"a\" appears twice"),
            ("offset = 8 } ]\n[[", "offset = 10 } ]\n[[", "\"a\": field \"x\""),
            ("id = { offset = 4, type = \"u8\" }", "id = { offset = 8, type = \"u8\" }", "packets.id must be"),
            ("schema = 1\n", "schema = 1\nrecord_size = 4\n", "go in each type"),
            ("id = { offset = 4, type = \"u8\" }\n", "", "need [packets] with an id"),
        ] {
            let err = Descriptor::parse(&TYPES.replacen(from, to, 1)).unwrap_err().to_string();
            assert!(err.contains(expect), "{to}: {err}");
        }
        // an id without types
        let err =
            Descriptor::parse(&(EXAMPLE.to_owned() + PACKETS + "id = { offset = 4, type = \"u8\" }\n")).unwrap_err();
        assert!(err.to_string().contains("needs [[packet_types]]"), "{err}");
    }
}
