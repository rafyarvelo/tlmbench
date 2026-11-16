//! A payload plugin template. A descriptor names it for a packet type or a field; it runs as it is on the made-up
//! layouts below: replace `packet_schema`, `decode_packets` and `decode_field` with yours.
//!
//! - **A packet type** (`[[packet_types]]` with `plugin = "demo-payload"`): each payload is a `u64` time in µs, a `u16`
//!   status word (bits 0 to 3: the mode, bit 15: a fault) and an `i16` temperature in tenths of a degree (`params =
//!   { temp_scale = 0.1 }` changes the scale). One row per packet.
//! - **A field** (`plugin = "demo-payload"`, `size = 3`, `type = "f64"`): the field's bytes are a big-endian signed
//!   integer of `size` bytes, times `params.scale` (default 1).
//!
//! What the host gives you: bytes, nothing else (no files, network or clock). The host frames the packets, checks their
//! checksums and decompresses them first. Columns are little-endian arrays in each channel's type; times are
//! nanoseconds and must not decrease.

wit_bindgen::generate!({ path: "wit", world: "payload" });

/// A number in the params JSON (`{"temp_scale":0.1}`), if it has one by that name. Enough for flat params; a real
/// plugin may use serde_json.
fn param(params: &str, key: &str) -> Option<f64> {
    let at = params.find(&format!("\"{key}\""))? + key.len() + 2;
    let rest = params[at..].trim_start().strip_prefix(':')?.trim_start();
    let end = rest.find(|c: char| !(c.is_ascii_digit() || "+-.eE".contains(c))).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

struct Demo;

impl Guest for Demo {
    fn describe() -> About {
        About {
            name: "demo-payload".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            description: Some("The template's made-up status packets and packed integer fields".into()),
            // a payload plugin is found by its name in a descriptor, not by file names
            files: Vec::new(),
        }
    }

    fn packet_schema(_type_name: String, _params: String) -> Result<Schema, String> {
        Ok(Schema {
            channels: vec![
                Channel { name: "mode".into(), unit: None, dtype: Dtype::U8 },
                Channel { name: "fault".into(), unit: None, dtype: Dtype::U8 },
                Channel { name: "temp".into(), unit: Some("°C".into()), dtype: Dtype::F32 },
            ],
            timed: true,
        })
    }

    fn decode_packets(_type_name: String, params: String, packets: Vec<Packet>) -> Result<Rows, String> {
        let scale = param(&params, "temp_scale").unwrap_or(0.1);
        let (mut time, mut mode, mut fault, mut temp) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for p in &packets {
            let b = &p.payload;
            // a payload too short to hold a reading gives no row
            if b.len() < 12 {
                continue;
            }
            let us = u64::from_le_bytes(b[0..8].try_into().unwrap());
            let word = u16::from_le_bytes([b[8], b[9]]);
            let raw = i16::from_le_bytes([b[10], b[11]]);
            time.push(us as i64 * 1000);
            mode.push((word & 0x0f) as u8);
            fault.push((word >> 15) as u8);
            temp.extend_from_slice(&((raw as f64 * scale) as f32).to_le_bytes());
        }
        Ok(Rows { rows: time.len() as u32, time, columns: vec![mode, fault, temp] })
    }

    fn decode_field(
        _field: String,
        params: String,
        dtype: Dtype,
        size: u32,
        count: u32,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        let size = size as usize;
        if size == 0 || size > 8 {
            return Err(format!("a packed integer is 1 to 8 bytes, not {size}"));
        }
        let scale = param(&params, "scale").unwrap_or(1.0);
        let mut out = Vec::with_capacity(count as usize * 8);
        for r in data.chunks_exact(size).take(count as usize) {
            // big-endian, sign-extended from its top bit
            let mut v: i64 = if r[0] & 0x80 != 0 { -1 } else { 0 };
            for &byte in r {
                v = (v << 8) | byte as i64;
            }
            let x = v as f64 * scale;
            match dtype {
                Dtype::F64 => out.extend_from_slice(&x.to_le_bytes()),
                Dtype::F32 => out.extend_from_slice(&(x as f32).to_le_bytes()),
                Dtype::I64 => out.extend_from_slice(&(x.round() as i64).to_le_bytes()),
                Dtype::I32 => out.extend_from_slice(&(x.round() as i32).to_le_bytes()),
                _ => return Err("this field gives f64, f32, i64 or i32 values".into()),
            }
        }
        Ok(out)
    }
}

export!(Demo);

// The test harness: `cargo test` runs your code on this machine (no WASM needed; the host's imports are stubs here).
// Check the built component the way the app runs it with `tlmbench plugins --check`.
#[cfg(test)]
mod tests {
    use super::*;

    fn packet(us: u64, word: u16, temp: i16) -> Packet {
        let mut payload = us.to_le_bytes().to_vec();
        payload.extend_from_slice(&word.to_le_bytes());
        payload.extend_from_slice(&temp.to_le_bytes());
        Packet { header: vec![0; 6], payload }
    }

    #[test]
    fn a_status_packet_is_one_row() {
        let rows = Demo::decode_packets("status".into(), "{}".into(), vec![packet(1500, 0x8003, 215), packet(2500, 2, -10)])
            .unwrap();
        assert_eq!(rows.rows, 2);
        assert_eq!(rows.time, [1_500_000, 2_500_000]);
        assert_eq!(rows.columns[0], [3, 2]);
        assert_eq!(rows.columns[1], [1, 0]);
        assert_eq!(f32::from_le_bytes(rows.columns[2][0..4].try_into().unwrap()), 21.5);
        // every channel has one value per row, in its type
        let schema = Demo::packet_schema("status".into(), "{}".into()).unwrap();
        assert_eq!(schema.channels.len(), rows.columns.len());
    }

    #[test]
    fn params_reach_the_code() {
        let rows = Demo::decode_packets("status".into(), r#"{"temp_scale":0.5}"#.into(), vec![packet(0, 0, 10)]).unwrap();
        assert_eq!(f32::from_le_bytes(rows.columns[2][0..4].try_into().unwrap()), 5.0);
        let v = Demo::decode_field("pos".into(), r#"{"scale":2}"#.into(), Dtype::F64, 3, 2, vec![0xff, 0xff, 0xfe, 0, 1, 0])
            .unwrap();
        let x: Vec<f64> = v.chunks_exact(8).map(|b| f64::from_le_bytes(b.try_into().unwrap())).collect();
        assert_eq!(x, [-4.0, 512.0]);
    }
}
