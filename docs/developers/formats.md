# File formats

What the code reads and writes, for people who write other tools against it. The code's own doc comments are the
reference where a table here is short: `crates/decode/src/reference.rs`, `descriptor.rs`, `crates/core/src/wire.rs`,
`crates/store/src/lib.rs`.

## The `.tlm` reference format

Fixed-width records behind a small header that embeds the file's own descriptor, so a file opens with no extra
configuration. Little-endian.

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 8 | magic `TLMBENCH` |
| 8 | 2 | format version (1) |
| 10 | 2 | flags (0) |
| 12 | 4 | header length: bytes before the first record, a multiple of 64 |
| 16 | 4 | descriptor length in bytes |
| 20 | 4 | reserved (0) |
| 24 | n | the descriptor (UTF-8 TOML), zero-padded to the header length |

Records follow back to back; a trailing partial record is ignored. The descriptor's `header_size` is ignored: the header
length wins. `tools/gen` (`tlmbench-gen`) writes it; `tlmbench convert` writes CSV, Parquet, Arrow IPC, HDF5 and MAT.

## Descriptors

A descriptor is a TOML file that lays out fixed-width records (or packets), so a new binary layout is a config file, not
a recompile. `schema = 1`.

```toml
schema = 1
name = "Bench records"
files = ["*.bin"]       # the files this descriptor decodes, by name (`*` matches any text)
endian = "little"       # little | big
header_size = 0         # bytes before the first record
record_size = 12

[time]
source = "field"        # rate | field | counter | index
field = "t"             # an integer field, not a channel
unit = "us"             # ns | us | ms | s
# source = "rate": rate_hz = 1000.0, t0_ns = ...
# source = "counter": field = "ticks", clock_hz = 1000000.0, wraps_at = 65536

[[fields]]
name = "battery"
type = "i16"            # i8 i16 i32 i64 u8 u16 u32 u64 f32 f64
offset = 8
unit = "V"
scale = 0.001           # value = raw * scale + value_offset (`offset` above is the byte offset)
```

Also: `bits = { shift = 4, width = 3 }` on a field takes a run of bits; `sequence = "seq"` names an integer field counting
frames (a live source counts the lost ones); `[packets]` frames records in packets (`sync`, `header_size`, a length field,
`checksum`, `compression`), and `[[packet_types]]` give each type its own layout; `plugin = "name"` hands a field or a
packet type to a payload plugin. `crates/decode/src/descriptor.rs` has the full schema (unknown keys are rejected).
A folder's `.tlmbench/decoders/` holds the descriptors it shares; the ones you save yourself stay with the app.

## The cache

Under the cache root (`--cache`, `$TLMBENCH_CACHE`, or the OS cache folder), never beside the data:

- `csv/<key>/`: a decoded CSV (or Parquet): `manifest.json`, one plain little-endian file per column (`t.i64` for times,
  `c<k>.f64` per numeric column), memory-mapped on reopen. Written to a temporary folder and renamed into place, so a
  failed build leaves nothing.
- `pyr/v2/<key>/`: the pyramid: `manifest.json`, one `c<k>.pyr` per channel (levels back to back, level 4 first, 88-byte
  little-endian summaries: first and last times, min and max with their sample indexes, first and last values, sum, sum of
  squares, count), `gaps.bin` and `times.bin`. The key hashes the source's fingerprint with how it is decoded (descriptor,
  scale and offset, profile), so a changed file never reads stale summaries. A build resumes after an interruption
  (`progress.json`), and `manifest.json` is written last. The cache is capped at 20 GB or 10 GB of free disk: the least
  recently opened entries go first, and entries in use are kept.
- `derived/<key>/`, `events/<fingerprint>.json`: derived channels and the events you added.

## `.tlmbench/`: shared configuration

A folder's `.tlmbench/` holds what a team shares, one small file per object: TOML for files people edit, JSON for layouts.
Only **Share** (and `tlmbench config`) writes it. [`examples/recordings/.tlmbench/`](../../examples/recordings/.tlmbench)
is a worked example.

| Path | What |
| --- | --- |
| `checks/*.toml` | rule-and-check sets: `[[check]]` tables (`kind` = `check` or `rule`) |
| `categories.toml` | event categories: id, name, colour, whether shown on charts |
| `recipes/*.toml` | derived-channel recipes |
| `views/*.tlmview.json` | shared layouts |
| `widgets/<id>/` | a widget: `widget.toml` (manifest) and its SVG or `widget.js` (code; runs only after the trust prompt) |
| `decoders/*.toml` | format descriptors |
| `events/*.toml` | events added to a file, by file |
| `plugins/*.wasm` | sandboxed plugins (never native ones) |

`tlmbench validate` lints a `.tlmbench/` folder.

## The view reply

The binary reply to a viewport query: little-endian, every array 8-byte aligned so the UI's `Float32Array`s view it with no
copy. A batch (one `view` request: every pane and the overview under one generation) frames one message per entry.

```text
message header (40 B):  magic "TLMV" · u16 version (3) · u16 flags · u32 generation · u32 trace count
                        · i64 origin_ns · f64 x_scale_ns · i64 built_until_ns
per trace (32 B):       u32 channel · u32 n · u32 kind · u32 reduce_us · f32 y_min · f32 y_max · u32 d · u32 0
                        · f32 x[n] · f32 y[n] · u32 dashed[d] (padded to 8 bytes)
batch header (16 B):    magic "TLMB" · u16 version · u16 0 · u32 generation · u32 entry count
per entry (16 B):       u32 id · u32 0 · u32 offset · u32 length   (then the messages, back to back)
```

Flag bit 0: the pyramid is still being built, only samples up to `built_until_ns` are summarised. Trace kind bits:
M4 (else raw), coarse (drawn from whole summaries, to be replaced by an exact reply), from the pyramid. A sample's time is
`origin_ns + x * x_scale_ns`; a NaN in `y` is a gap. `crates/core/src/wire.rs` is the reference, the UI's `wire.ts` decodes
it, and `t_wire` checks the two against a golden batch.

## The daemon's frames

`tlmbench-app --daemon` serves the app's commands over a loopback WebSocket guarded by a token. A request is a command
name and its arguments as JSON; replies, channel messages (build progress) and events come back as binary frames
`[kind u8][id u32 LE][payload]`: kind 0 a JSON reply, 1 a raw reply (the view reply above), 2 an error, 3 and 4 a JSON or
raw channel message (payload `[index u32 LE]` then the message), 5 an event. `app/src-tauri/src/daemon.rs` is the reference.

## HTML exports

A folder: `index.html`, the viewer's script, the core as WebAssembly, `export.json` (the manifest) and the data in chunks
with a BLAKE3 checksum each (`c<n>.<j>.bin` raw samples as f64 little-endian, pyramid levels per channel). A single-file
export holds the same in one page. [`docs/exports.md`](../exports.md) says what a user sees,
[`docs/viewer-store.md`](../viewer-store.md) what the manifest lists (pyramid levels, raw windows, checksums) and the rule
the viewer keeps: every value shown is a raw sample or an exact aggregate, and a view without the samples it needs says
"finer detail not in this export".
