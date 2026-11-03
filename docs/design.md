# TelemetryBench — Design

Oct 25, 2026 · @Rafy

## Goals and non-goals

TelemetryBench (short name tlmbench) is a desktop workbench for plotting, inspecting and investigating telemetry, live or recorded. It opens multi-GB recordings and live feeds and lets you pan and zoom over billions of samples at 60 fps, because the screen only ever draws a few thousand points per trace.

**How to read this doc.** Each section ends with its decisions and open questions; suggestions sit under the question they answer. The chip is the status: pick Accepted, Modified (then edit the words), or Rejected. Rejected suggestions stay on the page as the record of what was considered.

### **Goals**

- Open massive CSVs (first-class), Parquet and custom binary logs (fixed-width records and packetized/compressed streams) without loading them fully into RAM.
- Plot live sources (serial, UDP, TCP, files being appended) alongside recorded data, on the same time axis.
- Smooth interaction at any zoom: first paint of a 10 GB file in under 2 s, pan/zoom frames under 16 ms.
- Visually lossless downsampling: spikes and dropouts never disappear when zoomed out.
- A format descriptor system so a new binary layout is a config file, not a recompile.
- Derived channels from basic expressions: formulas that reference any number of channels and constants (e.g. `(a - b) * 0.5`) plus `delta(x)`, the sample-to-sample difference. The expression editor offers autocomplete for channel names (derived ones too), functions and value names. The v1 function set covers maths, sample-to-sample, rolling windows, whole-file summaries and conditions over time (see Derived channels), with numpy-style breadth as a stretch goal.
- Cross-platform via Tauri: Windows, macOS, Linux.

### **Non-goals (v1)**

- A statistical analysis suite or a built-in scripting language. Analysis beyond expressions uses the v1 Python bindings outside the app, or exports to CSV and Parquet.
- Publication-quality static figure styling.
- Multi-user collaboration or cloud storage.
- Non-time-series charts beyond XY panes and the XY dot (no scatter matrices, map tiles or 3D).

## Architecture

A Rust core owns all data work — parsing, storage, the pyramid and downsampling — and runs as a daemon that any number of frontends attach to; each frontend only draws pre-reduced buffers, so its cost is independent of file size.

&#91;embedded content: architecture · webview frontend, Rust core, disk\]

The chunk store is the center: every source writes into it and every view reads from it. Threads: one per live source, a `rayon` pool for parsing and pyramid builds, a `tokio` runtime for IPC and sockets, and the webview's main thread for drawing.

**Daemon and clients.** The core is a headless daemon that owns the chunk store; every frontend is a client of it. The desktop app embeds the daemon in-process, so a single desktop window keeps the zero-copy Tauri IPC path and stays the fastest setup. Extra desktop windows and browser tabs attach over a local socket or WebSocket, speaking the same binary request/reply protocol, so one frontend bundle runs everywhere.

- **Sessions**: each client has its own views, generation counters and live subscriptions; files, cursors and a linked time axis are shared with the clients that opt in.
- **Fan-out**: live data goes to each subscriber separately; a slow client drops frames instead of slowing the others.
- **Browser access**: binds to localhost with a per-session token by default.
- **Lifetime**: the daemon exits with the last window, unless started standalone (for example to keep logging live data with no window open).

### Multiple windows (in v1)

v1 ships with multiple windows. Each window is a client of the one core: in the desktop app that is the same process (Tauri windows), and in a browser it is the daemon above.

| Shared, owned by the core | Owned by each window |
| --- | --- |
| File caches, chunks and summaries (a file open in two windows is read and held once) | Tabs, panes, time view, cursors, Measure |
| Live sessions: ingest and recording happen once, any window can show them | Open files and which one is file 1 |
| Settings, recipes, import profiles, layouts, limits, value labels, your events | Undo history, sidebar layout and widths, Explorer state |

What the code does:

- Keep tabs, panes, open files and the view in a per-window state object; nothing UI-side is a global.
- Send every change to shared state (settings, recipes, profiles, layouts, events) through the core, which broadcasts it to all clients. A window that changes a setting sees it at the same moment as the others.
- Count users of a cached file: closing it in a window releases that window's hold; the cache drops it when no window uses it.
- Save the session as a list of windows, each with its tabs, size, position and monitor; reopening restores every window.

Moving tabs: drag a tab out of the tab bar to make a new window, drag it onto another window's tab bar to move it, and "Move to new window" in the tab menu. Tabs reorder within a window but don't dock or split inside it; panes already do side by side.

#### Decisions

- Modified Multiple windows ship in v1 (window-owned state, a shared core). Changed from "designed in v1, shipped later".
- Accepted Tabs reorder by drag and move between windows; no docking or splitting tabs inside a window.
- Accepted Closing a file in one window closes it only there; the core keeps the cache while any window uses it.
- Accepted A live session can show in several windows from one ingest; stopping it in one stops it everywhere, after a prompt.

#### Open questions

- **Time and cursors across windows.**
  - Accepted Independent by default, with "Link time view" (and optionally cursors) between chosen windows, for one flight spread over two monitors.
  - Rejected Always linked when two windows show the same file. It blocks two windows zoomed into different moments of one flight.
- **Quitting with several windows.**
  - Accepted Closing the last window quits (the daemon keeps running only if started standalone); reopening restores every window.
  - Rejected Each window closes on its own and the app quits only from the menu.
- **Where settings changes apply.**
  - Modified All settings are app-wide and apply to every window at once. The exception is which sidebars are shown and their widths, which are per window, since that's layout rather than a setting.
  - Rejected A few view settings (theme, sidebar, time format) can be set per window.
- **App identifier.** The reverse-domain bundle ID the OS knows the app by (settings and cache folders, file associations, permissions, signing, updates). It can't change after the first public build without the OS treating the app as new.
  - Accepted `io.github.rafyarvelo.tlmbench`, after the project's GitHub account.
  - Rejected `dev.tlmbench.desktop` was the placeholder in the Phase 1 build.

## Data ingestion

Every source — file or live — goes through one `Source` trait that yields typed column chunks, so storage, downsampling and rendering never know where data came from.

```rust
trait Source {
    fn schema(&self) -> Schema;            // channels, dtypes, units, time base
    fn next_chunk(&mut self) -> Option<Result<ColumnChunk>>; // ~64k rows
    fn seek(&mut self, t: Timestamp) -> Result<()>; // files only
}
```

| Source | Strategy | Key crates |
| --- | --- | --- |
| CSV (multi-GB) | Memory-map the file, split at newline boundaries, parse chunks in parallel; infer schema from the first 10k rows; numeric fast path | `memmap2`, `csv-core`, `lexical-core`, `rayon` |
| Parquet | Read the footer for schema and row-group stats; load only the columns plotted, skip row groups outside the time range by their min/max; decode straight into native chunks | `parquet, arrow-array` |
| Fixed-width binary | Record size known up front, so record N is at `header + N × size`; decode columns directly from the mmap with zero copies | `memmap2`, `bytemuck` |
| Packetized / compressed | Scan once to build a packet index (offset, timestamp, length) kept in the app cache, never beside the file; decompress packets on demand in parallel | `zstd`, `lz4_flex`, `flate2`, `nom` |
| Serial port | Reader thread → frame decoder → ring buffer | `serialport` |
| UDP / TCP | Async socket task → same frame decoder | `tokio` |
| Tail-a-file | Watch for appends, parse new bytes only | `notify` |

**Format descriptors.** Binary layouts are described in a TOML file the user can write or edit in a UI form: endianness, header size, record or packet framing (sync word, length field, checksum), and per-field name, type, byte offset, scale factor, value offset, unit and bitfield masks. The same descriptor drives file and live decoding, so a logger's recorded files and its live stream share one definition. A Rust `Decoder` trait sits underneath every tier of decoder plugin below.

**Decoder plugins.** Users extend decoding in three tiers, from no code to full native speed. Every tier yields the same typed column chunks, so the store, pyramid and views never know which one decoded a file. Plugins load in the daemon, are found in a plugins folder, and are listed with their tier and version in Settings. There is no external-process tier: the app never runs other programs. Existing parsers (in Python, say) convert to Parquet or CSV, which open directly.

| Tier | Written in | Runs as | Speed | Isolation | Best for |
| --- | --- | --- | --- | --- | --- |
| 1. Descriptor | TOML or the UI form | Built-in interpreter | Native | Full: no code | Fixed-width records, simple packets |
| 2. WASM plugin | Rust, C, Zig, Go, AssemblyScript | `wasmtime` sandbox, one `.wasm` file | Near native | Sandboxed; no file or network access | Odd framing, custom compression, checksums; shareable and hot-reloadable |
| 3. Native plugin | Rust (or C ABI) | Shared library implementing `Decoder` | Native, zero-copy from mmap | None: trusted code | Heaviest formats where every cycle counts |

The WASM interface is small: the host passes byte ranges, and the plugin returns a schema and column buffers. It is defined once in a WIT file, with a template crate and test harness, so a new plugin starts from `cargo generate`. A tier-1 descriptor can name a tier-2 or tier-3 plugin for one field or packet type, so most of a format stays declarative.

**Time base.** Each channel resolves to `i64` nanoseconds since epoch (or since start, for relative logs). Channels at different rates are not resampled at ingest; they are aligned only when drawn or exported. The schema names each channel's time source, one of five:

- **Column**: a timestamp field (epoch, ISO text or seconds from start), parsed to nanoseconds.
- **Rate**: no time field, but a known sample rate and start, so `t = t0 + i / rate`. Nothing is stored per sample; the time is computed on demand.
- **Counter**: a tick or packet counter plus a clock rate, with wraparound unwrapped.
- **Arrival**: live sources with no timestamps are stamped by the daemon's monotonic clock as bytes arrive.
- **Index**: nothing gives time. The x-axis becomes sample number, and the user can assign a rate and start later to turn it into time.

Index channels sit in their own linked group, since they can't share an axis with timed data until given a rate. Timestamps that go backwards are flagged in the index rather than silently reordered.

### Decisions

- Accepted v1 formats are CSV (first-class), Parquet, and one custom binary format we define as the reference for the descriptor schema.
- Accepted The reference binary format is `.tlm`: fixed-width records after a header that embeds the file's own descriptor, so a file opens with no extra config and every open goes through the descriptor parser. The header is the magic `TLMBENCH`, a version, the header length (a multiple of 64 bytes) and the descriptor length, then the descriptor as TOML. Records follow back to back; a trailing partial record is ignored. Other fixed-width files take a separate descriptor.
- Accepted The Phase 1 descriptor covers fixed-width records: schema version, byte order, header and record size, time from a rate, an integer field (ns, µs, ms or s) or the sample index, and per field a name, type (i8 to u64, f32, f64), byte offset, unit, scale and value offset. Bitfields, packet framing and plugin fields come later as new keys.
- Accepted CSV schema detection: comma, semicolon or tab, whichever the header row has most of. A unit after a space in brackets or parentheses (`Speed [m/s]`, `Temp (°C)`) becomes the channel's unit, while `accel[0]` stays a name; repeated names get `  (2) `. The time column is the first one named like a time (`time`, `t`, `timestamp`, `date`, `time_s`, `t_ms`, …) with at least 90% of its sampled values parsing, else the first column of ISO dates, else row numbers. A column with any non-numeric value in the first 10,000 rows is text and left out. Rows whose time doesn't parse are skipped and counted.
- Accepted Time that goes backwards is an error naming the row or record, until the index can flag it as the design intends.
- Accepted Packets and compression are new descriptor keys, not a separate file. An optional `[packets]` table gives the sync word, the length field (offset, type and what it counts), the checksum (none, sum8, crc16-ccitt or crc32), the payload's compression (none, zstd, lz4 or gzip), and an id field that picks one of several `[[packet_types]]`, each with its own fields. A first open scans the file once and keeps a packet index (offset, time, length) in the app cache, never beside the file; packets are decompressed on demand. The same keys decode a live stream.
- Accepted A packet file with several packet types opens one recording per type, as if each were its own file (`flight.bin · gps`, `flight.bin · imu`) in the same tab, on the same clock. Each type keeps one time base, so the core is unchanged, and `convert` writes a file per type, named \<name>.\<type>.\<ext> (the rate groups the CLI design already names). In the descriptor, \[packets\] id = { offset, type } names the field that says the type, and each \[\[packet\_types\]\] has an id, a name and its own record\_size, time, sequence and fields (the top level then has none of these). Without a length field, the type's record size frames its packets. A packet whose id no type claims is skipped and counted once. On the CLI, --type NAME picks one type: info and convert default to all of them, and a query asks which.
- Accepted In a packet file each record is read as the packet header followed by that record's bytes (after decompression): field offsets below the header size reach header fields such as a packet timestamp or counter, and the rest reach the record. With several records in a packet, the header fields repeat for each.
- Modified Parquet opens through the columnar cache, like CSV: the first open decodes every numeric column, row groups in parallel, keeping each column's type (f32 stays 4 bytes), and later opens read the cache through maps. Changed from reading plotted columns straight from the file: the pyramid build reads every column on open anyway. Packet files still decompress on demand, since a large compressed file would expand past the disk.
- Proposed CSV files may start with `#` comment lines before the header; they are skipped. `convert --resample` writes its note there, so the app reads its own exports.

## Storage and indexing

Data is stored once as raw columns plus a precomputed min/max pyramid, so any zoom level is answered by reading at most a few thousand summary entries.

**Raw columns.** Each channel is a sequence of fixed-size chunks (64k samples): a timestamp column and a value column, typed (`f32`, `f64`, `i16`, …). For fixed-width binary the original file is the store, read through mmap. CSV and compressed formats are converted once into a cache file (Arrow IPC or a simple custom columnar layout) in the app cache dir (Tauri's app\_cache\_dir), never beside the source, keyed by file hash + mtime, so the second open is instant. Settings shows the cache path and total size with Open folder and Clear cache buttons; a size cap (default 20 GB, and eviction also starts when free disk drops below 10 GB) evicts the least recently opened files first, and a deleted cache is simply rebuilt on the next open.

**Pyramid.** For each channel, level 0 is the raw data; level k holds one summary per 4^k raw samples (bucket factor 4 to 16 is tunable). Each summary stores `t_first`, `t_last`, `min` and `max` (each with its sample index), `first`, `last`, `sum`, `sum of squares` and `count`: enough for M4 rendering and for exact min, max, mean and standard deviation over any range, with ragged edges read from raw samples. Levels finer than 256 samples per summary (k < 4) aren't stored: a summary is about 88 bytes, so they would cost several times the raw data, and views that fine are reduced straight from raw samples. The stored levels add about 11% to raw f32 data.

**Lookup.** A chunk index maps time ranges to chunk numbers via binary search on each chunk's first/last timestamp. For a viewport \[t0, t1\] at width W pixels, pick the coarsest level whose bucket count in range is still ≥ 2W, read just those buckets, then refine to exact pixel buckets.

**Memory.** Hot chunks live in an LRU cache with a byte budget (default 25% of RAM); everything else stays on disk via mmap and the OS page cache. Gaps (time jumps larger than N × the median sample interval; N is 4 by default and set per channel) are recorded in the index so lines break instead of drawing across dropouts.

### Decisions

- Accepted Hard requirement: full capability from the app cache alone, so a read-only folder works. Source folders stay untouched; caches and sessions live only in the app cache dir. Only actions you start write to a folder: Share writes `.tlmbench/` at the folder root, Export to HTML writes where you choose.
- Accepted The CSV cache is a simple custom columnar layout, not Arrow IPC: one plain little-endian file per column (`t.i64` for times, `c<k>.f64` per numeric column) plus `manifest.json`, memory-mapped on reopen. It is written to a temporary folder and renamed into place, so a failed or interrupted build leaves nothing. Arrow stays an export format.
- Accepted The cache key is a fingerprint, not a whole-file hash: size, modification time, and a BLAKE3 hash of the first and last MiB plus 63 blocks spread through the file. A whole-file hash would cost about a second on every open of a multi-GB file; reopening a 2.4 GB CSV takes 0.01 s.
- Accepted One cache root for the CLI and the app: `--cache`, else `$TLMBENCH_CACHE`, else the OS cache folder's `tlmbench` (`~/Library/Caches/tlmbench` on macOS), rather than Tauri's per-app cache folder, so a file cached by one is instant in the other.
- Accepted CSV is read with positioned reads, each worker reading its own 8 MB chunk, not through one memory map: parallel page faults on one mapping serialize on macOS. Unquoted data takes a fast path (lines split with memchr); a quote found anywhere restarts the file with the general CSV reader, since a quoted field can hold a newline. On a MacBook Air M1 a 2.4 GB, 100M-row CSV ingests in 3.0 s (about 800 MB/s) with 430 MB peak memory.
- Accepted The pyramid stores levels from 256 samples per summary (k = 4 on the 4^k grid), ×4 above that, about 11% of raw f32 data. Storing every level from 4 samples would cost about 4.7× the raw data, since a summary is about 88 bytes.
- Accepted Gaps are time jumps over 4 × the median sample interval by default; the factor is a per-channel setting. In the app, right-click a channel (its row in the list, or its chip on a pane) and choose Gap factor… to set it (at least 2); it is saved in the app's settings per file and channel, and a change redraws without a rebuild.
- Accepted The cache evicts the least recently opened files when it passes its cap (default 20 GB) or when free disk drops below 10 GB, whichever comes first.
- Accepted Pyramid files: one folder per recording, `pyr/v1/<key>/` under the cache root, holding `manifest.json` and one file per channel with its stored levels back to back (level 4 first). A summary is an 88-byte little-endian record: `t_first`, `t_last` (times of its first and last samples), `min` and `max` as f64 with their sample indexes, `first` and `last` (the values of its first and last samples, NaN included), `sum`, `sum of squares` and the count of non-NaN samples. Summaries are aligned to sample index (summary s of level k holds samples s·4^k to (s+1)·4^k − 1), so finding one needs no search. Values are stored as f64, exact for every type except 64-bit integers past 2^53.
- Accepted Pyramid versioning and invalidation: the key is a BLAKE3 hash of the source's fingerprint (as for CSV), the decoder and its descriptor, and every setting that changes values (scale and offset now, missing values later). Any of these changing gives a new key, so stale summaries are never read; a layout change bumps `v1`. On open each file's size is checked against the manifest. Old keys are left for the cache cap to evict.
- Accepted Interrupted builds resume: a build writes in place into preallocated files in `pyr/v1/<key>/`, in blocks of 4^10 samples, in file order, and after each block records its high-water mark in `progress.json` (replaced by rename). A later open (the app's or the CLI's) continues from the mark; the levels coarser than a block are made from the block summaries at the end, and `manifest.json`, written last, marks the entry complete. There is no folder rename: Windows refuses one while its files are mapped. A lock file keeps two processes from building the same key; the second follows the first's progress. A 10 GB, 20-channel `.tlm` builds cold in 12.5 s on the M1 (the disk reads it in 9.5 s), with 229 MB of memory: each block is read once for every channel, with the next block read ahead.
- Accepted The gap index is stored per time base (channels of a recording share their times) in `gaps.bin` beside the pyramid: every jump over 2 × the median interval, as (sample index, length in ns), sorted, with the median in the manifest. The per-channel factor N (default 4, at least 2) filters it at query time, so changing N rebuilds nothing. The median comes from up to 1M intervals sampled evenly across the file. Fixed-rate channels have no gaps and no file.
- Accepted The chunk index of a time column is stored as `times.bin` beside the pyramid: the time of every 64k-th sample, as i64 (15 KB for 125M samples), read from each block during the build while its pages are in memory. A search for a time first finds its chunk in the index, then searches only that chunk, so a cold search touches a few pages instead of pages across the file. Adding the file bumps the pyramid folder to `pyr/v2`; `v1` folders are no longer read and age out under the cache cap. On a 50M-row CSV with its pages evicted, an exact whole view takes 0.81 s (1.03 s without the index); a cold 4M-sample view 99 ms (105 ms).
- Accepted The LRU chunk store holds decoded chunks only: 64k samples of one channel as a contiguous native array, for data that isn't stored that way (interleaved records like the 20-channel `.tlm`, big-endian, scaled fields). It counts against the 25% of RAM budget; chunks in use are reference-counted and survive eviction until released. Pyramid files and data already contiguous (CSV cache columns, one-channel `.tlm`) are read through mmap and left to the OS page cache, outside the budget. Decoding a block of records fills the chunks of every channel on screen at once. Raw reductions (under 1,024 samples a pixel), values at a time and the raw edges of stats decode the chunks they read; views from the pyramid read their edge samples through chunks already decoded, else in place. A chunk's records are gathered straight from the map when they are in memory, else read with one positioned read. A 1M-sample view of all 20 channels of a 2 GB .tlm on the M1 takes 3.5 ms with its chunks decoded and 10.5 ms decoding them (20 ms read in place); with the file's pages evicted, 44–52 ms (82–88 ms in place).
- Accepted A new crate, `tlmbench-store`, owns the cache root, fingerprints, pyramid and gap files, background builds and the LRU; `tlmbench-core` keeps the pure algorithms (M4, summaries, merging, stats) with no I/O, and `tlmbench-decode` keeps the decoders. The CLI and the app open recordings through the store.

## Downsampling

Use min/max-per-pixel (M4) as the default renderer: it is exact for line charts, cheap, and works directly off the pyramid; use LTTB only where a small, smooth point set is wanted.

**Why it works.** A 1,000-px-wide plot can show at most 1,000 distinct x positions. Drawing 100M samples into it is wasted work: thousands of samples land on the same pixel column, and the line drawn there is just a vertical stroke from the column's min to its max. So for each pixel column we only need a handful of values.

| Method | Keeps per bucket | Visually lossless? | Cost | Use for |
| --- | --- | --- | --- | --- |
| Decimation (every Nth) | 1 sample | No — drops spikes, aliases | O(n/N) | Never for display; only quick previews |
| Average | 1 mean | No — flattens peaks | O(n) | Smoothed overview mode |
| Min/Max | 2: min, max | Nearly; connecting lines between columns can be off | O(n), pyramid-friendly | Envelope / band rendering |
| M4 | 4: first, last, min, max | Yes, pixel-exact for lines | O(n), pyramid-friendly | **Default** line rendering |
| LTTB | 1 chosen sample | Mostly; may skip narrow spikes | O(n), not composable | Small exports, sparklines, scatter-like views |

**M4 in practice.** For viewport \[t0, t1\] and W pixel columns, bucket width is (t1 − t0) / W. For each bucket emit its first, min, max and last samples (in time order). Drawing a polyline through those 4W points produces exactly the same pixels as drawing every raw sample. Because min of mins and max of maxes compose, pyramid summaries merge into pixel buckets without touching raw data.

**LTTB (Largest-Triangle-Three-Buckets).** Splits data into N buckets and, walking left to right, picks the point in each bucket that forms the largest triangle with the previously chosen point and the next bucket's average. It preserves shape well with only N points, but it needs raw samples and cannot be precomputed per level, so it runs on the already-reduced M4 output when used.

### **Edge cases to handle**

- Bucket boundaries must be snapped to pixel columns, or the plot "shimmers" while panning. Align buckets to a global grid: floor(t / bucket\_width).
- When zoomed in past 1 sample per pixel, skip reduction and send raw points (draw markers when spacing > \~6 px).
- Non-uniform sample rates and gaps: a bucket with no samples breaks the line rather than interpolating.
- NaN / invalid values are dropped from min/max and rendered as gaps.

### Decisions

- Accepted Raw samples are sent when the view holds at most 4W samples, not W: that is no more points than M4 would send, and raw is the exact picture. Every trace also includes the nearest sample beyond each edge of the view, so the line runs to the edges while panning.
- Accepted Bucket width is a whole number of nanoseconds (the span divided by W, at least 1 ns), so the global grid `floor(t / bucket_width)` is exact integer arithmetic. Ties for min or max go to the earliest sample.
- Accepted Gaps in Phase 1: a bucket with no samples, or only NaN, breaks the line (one gap for a run of empty buckets). Time jumps between samples in a raw view don't break it yet; that comes with the gap index in the pyramid phase.
- Accepted Views come from the pyramid and stay pixel-exact: a pixel bucket's index range splits into whole summaries of the coarsest level that fits, plus a straddling summary at each edge. A straddler's min or max counts where its sample index falls; only when a straddler's extreme lies on the far side and it could still hold the bucket's extreme on the near side are its raw samples read (at most 255, since level 4 is the finest stored). Each bucket's first and last samples are read raw. The result equals M4 over raw samples point for point, which property tests check. Pixels of fewer than 1,024 samples are reduced from raw samples, which costs about the same. A whole-file view of 100M samples takes 0.31 ms from the pyramid (21 ms raw).
- Accepted A summary whose first or last value is NaN but which holds other samples has its raw samples read to find the first or last real one, so NaN handling matches raw M4. This happens only at the edges of NaN runs.
- Accepted Gaps from the gap index break the line at every zoom: a NaN point goes between the two samples of each indexed jump inside the view, in raw and M4 views alike.
- Modified A coarse view takes each pixel's whole summaries, with values from the summaries, so it reads raw samples only within 256 samples of the view's two edges and of pixels small enough to reduce raw. Every point is a real sample at its true time and every extreme is drawn. A boundary between two such pixels rounds up to one grid for the view (summaries of the coarsest level at most a quarter of its smallest such pixel), the same for both, which moves a sample at most a quarter pixel, leaves none out, and reads no finer summary except next to the view's edges (changed Nov 2, 2026: rounding each pixel's edges by its own level skipped samples at the view's left edge and between pixels of different levels, so a spike could be hidden; a warm full view now costs 0.17 ms a trace instead of 0.12, and a cold one reads the same summaries as before). On the Gate 2 file with its pages evicted, 20 channels at 2264 px draw coarse in 0.22 s and exact in 1.7–2.3 s (0.11 s warm). A cold exact view reads level 4 through and fetches the raw samples at every pixel edge in parallel before reducing, since page faults one by one cost more.

## Rendering

Rust sends each trace as a compact binary buffer of at most \~4W points, and WebGL2 draws it in one call per trace, so a frame costs well under 16 ms with dozens of traces (the target is 20 traces; see Performance).

**Request loop.** On pan/zoom the frontend sends one request for the whole tab: `{generation, precision, entries: [{id, channels, gap_factors, t0, t1, width_px}]}`, one entry per pane plus the overview when it needs redrawing. Rust answers with one batch of M4 buffers tagged by `generation`; stale replies are dropped. While waiting, the frontend re-projects the last buffer with a new transform matrix, so pans feel instant and the sharper data swaps in a frame or two later.

**IPC.** Avoid JSON for sample data. Return raw bytes from Tauri commands (`tauri::ipc::Response` with a `Vec<u8>`), arriving as an `ArrayBuffer` that becomes a `Float32Array` with no parsing. Each trace is a small header then its x and y arrays (layout under Decisions below).

**Precision.** `f32` can't hold nanosecond timestamps. Send x as `f32` offsets relative to the viewport start (or the buffer's own origin) and keep the origin in `f64`/`BigInt` on the JS side. Do the same for y when values have a large DC offset.

### **WebGL pipeline**

1. Upload x/y arrays to a vertex buffer (`gl.bufferData` / `bufferSubData` for live appends).
2. Vertex shader applies the data→clip transform as a uniform; pan/zoom only changes uniforms.
3. Lines: thin lines via `LINE_STRIP`; thick or anti-aliased lines via instanced quads (each segment expanded to two triangles in the shader).
4. Min/max envelopes as a triangle strip with a translucent fill under the line.
5. Axes, grid, labels, cursors and legends in a 2D canvas or DOM overlay on top (text in WebGL is not worth the effort).

**Library choice.** The app uses a thin custom WebGL2 layer (\~1–2k lines), since the data is pre-reduced. The HTML prototype uses `uPlot` (Canvas2D, very fast for pre-reduced data), which also serves as the app's Canvas2D fallback. Keep the renderer behind an interface so a native `wgpu` path stays possible later.

### Decisions

- Accepted The viewport reply format, defined once in the core for IPC now and the daemon's socket later: a 40-byte header since version 2 (magic `TLMV`, version, generation, trace count, `origin_ns` as i64, `x_scale_ns` as f64, flags, and the time up to which the file is summarized), then per trace a 24-byte header (channel, n, kind raw or M4 with its flags, the core's time to reduce it, `y_min`, `y_max`) and `f32 x[n]`, `f32 y[n]`, all 8-byte aligned so the UI takes `Float32Array` views with no copy. A sample's time is `origin_ns + x × x_scale_ns`, with the origin at the view's start and the scale one bucket, so x runs over about 0 to W and f32 stays exact at any zoom. `y_min`/`y_max` cover the samples inside the view, for autoscale; NaN in y is a gap. Times in requests are decimal strings, since a JSON number can't hold i64 nanoseconds.
- Accepted Each trace header also carries `reduce_us`, the core's own time to reduce it: WebKit rounds the page's clocks to 1 ms, too coarse to time a sub-millisecond query.
- Accepted The app's UI is TypeScript and Vite with no framework while it is one pane; the framework is chosen when the prototype's UI is ported.
- Accepted Views over 256k samples are reduced on several threads (rayon), so a 1M-sample view takes about 0.5 ms on an M1 instead of 1.7 ms.
- Accepted Phase 2 IPC: `view` takes every pane of a tab at once (each with its channels and width) under one generation, so the stack redraws together; the overview is one more entry. Each entry carries an id and its own span, width and gap factor per channel; the reply is a small container (magic TLMB, the generation, then an id, offset and length per entry) around one version 2 message per entry. The UI sends one request per frame, with the overview only when it needs redrawing. Wire version 2 adds per trace flags (coarse, from pyramid or raw) and the time up to which the file has been read. `open` returns a channel that streams build progress. `values_at {file, channels, t}` returns each channel's nearest raw sample (time and value) for hover and cursors; `stats {file, channels, t0, t1}` returns exact min, max, mean, standard deviation and count from the pyramid plus raw edges, for Measure.
- Accepted The UI stays TypeScript without a framework through Phase 2: the plot stack, overview, hover and cursors are canvas and a little DOM. The framework is chosen when Explorer, tables and menus are ported.
- Accepted Phase 2 picks channels from a plain list in a left sidebar (checkbox plots into the selected pane, as in the prototype's Explorer rows); the full Explorer comes with the port.

### Loading what's on screen

Opening a file never waits for the whole file. Panes appear at once and each fills in when the data it needs is in. The core reads in this order: the channels of the panes on screen (top to bottom), then the overview, then the channels that rules and checks read. Each pane says what it needs:

| Pane or feature | Needs | Until it's in |
| --- | --- | --- |
| Table | The rows on screen, read in whole chunks (about 64k rows) and cached with the charts' chunks; the row count comes from the index | Rows and the scrollbar show at once; cells not read yet show … |
| Table sort and filter | A scan of the whole column | The table stays usable unsorted; "Sorting and filtering when X is read…" |
| Values pane | One value per channel (one chunk) | The tile shows … and "reading" |
| Chart, chart and table, XY, overview | The whole-file summary (pyramid), or the raw chunks in view when zoomed in | On a first open, fills in left to right with exact data as the file is read (the rest hatched, "Reading 43%"); a view drawn from whole summaries before its edge samples are read sharpens a moment later, labelled "coarse" if it stays up more than 0.3 s; summaries keep every min and max, so no spike is hidden |
| Measure stats | The summary over the range | "Reading X…" instead of a number |
| Rules, checks, event tints | A scan of the channels they read | The Events list says "checking rules…" |
| Derived channels | Their inputs, with lookback for rolling windows; whole-file numbers need everything | "computing" until the inputs are in |

In the app the summary is cached in the app cache after the first open, so charts of a file you've opened before draw at once. A first open builds the coarse levels first. In the prototype, opening flight\_0415 as one table went from 12 s to rows on screen in about 0.5 s, with all 52 columns filled in about 7 s.

#### Decisions

- Accepted Panes appear at once and fill in as their data arrives; on-screen channels load first, and channels nothing shows are not read.
- Accepted Tables fetch row windows from the core; sort and filter run in the core over the whole column, with progress.
- Modified Charts, XY and the overview fill in left to right with exact data while a first open reads the file once, the unread part hatched. A view drawn from whole summaries before its edge samples are read sharpens a moment later, labelled "coarse" only if it stays up more than 0.3 s (a cold first paint), so a quick replacement never flashes. While the file is read, the panes refresh with exact views only (what was just read is in memory) and the hatching glides with them instead of jumping (changed Nov 2, 2026: the label flashed and the fill-in jumped four times a second). Anything that must be exact (stats, values at a cursor, checks) waits for the full summary and never shows a partial answer.

#### Open questions

- **Row window size and cache.**
  - Rejected Fetch the rows on screen plus one screen above and below; keep the last 8 windows per table (a few MB).
  - Accepted Fetch whole chunks (about 64k rows) and cache them by chunk, sharing the cache with charts: sorts and jumps reuse what's read, and nothing is read twice.
- **Coarse charts on a first open.**
  - Rejected Draw from the coarse levels as soon as they cover the file, labelled "coarse", then refine.
  - Rejected Wait for the full summary, as the prototype does.
  - Accepted Every coarse summary needs every raw sample under it, so no level can cover the file before one full read, and by then every stored level is built too. So a first open reads the file once, in order, at disk speed, and panes fill in left to right with exact data; the part not read yet is hatched and says "Reading 43%". A view zoomed into that part is reduced straight from raw samples. "Coarse" then labels a view drawn from whole summaries before its edge samples are read (about 1 pixel of difference at most, every extreme still shown); it sharpens a moment later.

## Live streaming

Live data lands in a lock-free ring buffer, updates the pyramid incrementally, and is pushed to the UI at display rate (30–60 Hz), never per sample.

- **Ingest thread per source** decodes frames with the same descriptor as files and writes into a single-producer ring buffer (`rtrb` or `crossbeam` channels) sized for \~10 s of peak rate.
- **Store writer** drains the ring, appends to the open chunk, and folds each new sample into the open bucket at every pyramid level (O(levels) per sample). Each source's frames are appended to its own .tlm file in the session's folder, so a live session is automatically saved and reopens like any other file.
- **Follow mode**: the viewport tracks "now" with a fixed window (e.g. last 30 s). The UI ticks on `requestAnimationFrame`; at each tick Rust sends only the buckets that changed since the last generation, and the frontend appends them with `bufferSubData`.
- **Scrolling back** while recording leaves follow mode; history comes from the same store as file data. A scrubber across the session in the live strip moves back to any point. The view holds there while data keeps arriving; the thumb turns amber and a label says how far back (−02:10). The right end, End or Jump to live returns. Panning or zooming moves the thumb too. The prototype keeps the last 5 minutes; the app spans the whole recorded session, read back from the session file.
- **Backpressure**: if the UI falls behind, frames are coalesced (latest wins); ingest never blocks. If disk falls behind, keep data in RAM up to a budget, then show a visible "dropping samples" warning with counts.
- **Triggers (v1)**: an oscilloscope-style trigger on a level or edge of any channel or derived channel freezes a window around the event (pre and post time, single or repeat). Triggers reuse the rule engine and drop an event each time they fire, so the same trigger also steps through a recorded file.
- **Connecting a source**: one dialog for every source kind (serial port, UDP, TCP, tail a growing file, and file playback). Each form names its descriptor and previews how frames decode, offers "record while streaming", and can save the connection to the start page.
- **The live tab is a file tab whose files grow.** Each source of a live session is a file in Explorer (under Live, marked LIVE), with the same channel tree, selection, multi-select, drag, Derive… and categories as any file. The panes are the ordinary panes, so every view works: chart, table, chart and table, chart and values, values, XY, plus cursors, Measure and events. A strip above the panes adds what is live-only: the LIVE / PAUSED badge and REC time, a scrubber across the session, the window presets (10 s, 30 s, 1 min, 5 min), Following / Jump to live, and a Sources button whose panel holds each source's card, stream health and the recording file. Any view you choose (pan, zoom, a jump to an event) holds the view; End or Following returns to live. While following, tables keep the newest rows in sight and value tiles show each channel's newest sample.
- **Multiple sources per session**: a live session holds any number of concurrent sources on one time axis (session time, the seconds since the session started), recorded into one session folder with a .tlm file per source. Source 1 keeps plain channel names. Source 2's channels are named like `gps_lat·2`, the same slot rule files use, so layouts and limits refer to them the same way. Each source is a file in Explorer and has a card in the Sources panel with its state, rate and position, plus Pause / Resume / Stop / Remove (and speed, for playback); closing its file removes the source. "+ Add source" adds one to the session in front, and the connect dialog can also open a new live tab. Separate live tabs keep running side by side.
- **One pane system**: live panes are file panes. The core appends each new sample to the channel's grid and folds it into the open bucket at every pyramid level, so drawing a live window costs the same as drawing a file. A live file can sit in a file tab beside a recorded flight (live against a reference run), and a recorded flight can be added to a live tab.
- **Value board and limits**: a tile per channel shows the latest value large, with its unit. Each channel can have warn and alarm limits, low and high. A tile is green inside its limits, yellow past a warn limit and red past an alarm limit, and charts draw each limit as a dashed line with shading beyond it (yellow past warn, red past alarm). Click a tile, or a channel's chip on any chart, to edit its limits. Limits are a setting keyed by channel name, so they apply in live and file panes and to every file slot. File panes also have a Values mode, showing each channel at cursor A (or at the end of the view). Sensible starting limits exist for common channels (battery and cell voltage, ESC temperature, current, rpm, GPS satellites and HDOP, roll and pitch).
- **File playback**: replays a recording as if it were arriving now, each channel at its recorded rate times a speed, through the same Source path. It is used for demos and for building layouts without hardware, and the prototype uses it as its live demo.

### Decisions

- Accepted Live input is designed for both 10 kHz × 32 channels and 1 MHz × 4 (at least 4M samples/s in total).
- Accepted A live tab is a file tab whose files grow: one Explorer, one pane system, one set of settings for both. Only the live strip (status, session scrubber, window, Following, Sources) is live-only. Replaces the separate live view with its own channel list and panes.
- Accepted Live files take no time offset in v1: every source starts at session time 0 and aligns by start.
- Accepted Pause depends on the source. A device keeps sending, so pausing it stops reading and the session clock runs on; resuming leaves a gap. A session that only replays files stops its clock when nothing is streaming, so a paused replay is a real pause: the charts hold still and resume with no gap. In a mixed session, pausing a replay stops just that source and the clock keeps running for the devices. The badge's tooltip says which kind of pause it is (prototype v153).
- Accepted A session is a folder: `session-<start time>/` holds `source-1.tlm`, `source-2.tlm`… (the reference format, records appended as they arrive, the descriptor in each header) and a small `session.json` (its name, start time and sources). Each source reopens like any .tlm with the same reader, pyramid and cache. Chosen over one container file holding every stream, which would need a new format and reader.
- Accepted Live recordings go to the app's data folder by default (`~/Library/Application Support/TelemetryBench/Recordings` on a Mac, the equivalent elsewhere), never next to the data, and a setting can point elsewhere. They are your data, not cache: the cache's cap and low-disk eviction never delete them; the start page lists them and you delete them. Chosen over keeping them in the evictable cache.
- Accepted Tailing a growing file comes in Phase 3 with the other sources: `file:///path/run.csv` (or a growing .tlm) records the whole file so far, then follows it, taking only complete lines or records and holding back a half-written one, through the same import profile or descriptor as opening it. The file is only read; the session's recording goes to the Recordings folder. A truncated or replaced file (log rotation) continues the source in its next part, source-N-2.tlm, with times as written, so nothing is shifted; a row cut short by the rotation is left out and counted. Within one file, a row earlier than the row before is left out and counted in stream health. `tlmbench-gen --append` writes rows at a rate in uneven chunks (lines cut mid-way, one truncation) and `--verify` checks the recording row for row. `just gate3 core` reports the rate a CSV tail sustains, not gated: parsing text costs more than binary records.

### Open questions

- **Scrolling back past memory.** The prototype keeps 5 minutes per source in memory.
  - Accepted Read older chunks back from the session's recording file, so a live tab scrolls back to the session start like a file (the session scrubber spans all of it).
  - Rejected Stop at what's in memory and offer "Open the recording" for the rest.
- **Event rules on live data.**
  - Accepted The core runs rules and checks incrementally as samples arrive, so rule events and failing checks appear live (with a short delay for minimum durations).
  - Rejected Rules run only on the recording, after the session or when you pause.
- **Overview strip in live tabs** (hidden in the prototype).
  - Rejected Show the whole session so far, growing to the right, with the window marked.
  - Modified Keep the overview strip hidden in live tabs; instead the session scrubber's track draws a faint trace of the overview channel, so you still see where the interesting parts are.

## UI and interactions

The main window is a channel browser on the left and a stack of plot panes sharing one time axis, with an overview strip at the bottom showing the whole recording.

### **Layout**

- **Shell:** an icon rail on the left (Explorer, Events, Rules & checks, Layouts, Measure, Live, Settings), one slim top bar, and flat, borderless panes. A rail icon opens its view in the left sidebar; clicking it again hides the sidebar. Measure (cursors and stats) docks on the right and opens by itself when both cursors are placed. Sidebars resize by dragging their edge.
- **Explorer** (left): folders, recordings and their channels in one tree (file → category → channel). The panel's title row holds the frequent actions as icons: add the shown channels to the selected pane, new derived channel, categories on or off, Expand all and Collapse all. Below it are a folder menu (Open folder…, Import all, close all files), a filter with a regex toggle inside it, and ⋯ for the rest (add all, open in a new chart or table, recipes and profiles). The open-files strip has Show only these (a toggle) and Close all. Rows show only what matters: the data rate of imported files, and "primary" when several files are open. One click selects a row and shows its actions beside the name; double-click or Enter opens a file, the checkbox plots a channel, and right-click renames or moves channels between categories.
- **Open folder:** the app's native picker sets a folder, remembers recent ones and watches them for new files. The prototype uses the browser's folder picker: a one-time, read-only snapshot whose CSV, TSV and TXT files import when opened, forgotten on reload. Nothing is ever written to the folder.
- **Import step for text files:** the first time a CSV is opened (from a folder, Open recording…, or dropped on the window) one dialog shows the time column and unit, the data rate (as recorded by default; resample, assume a rate, or sample numbers for files with no time), and every column: name kept verbatim, unit parsed from a trailing \[unit\] or (unit), type (number, text or skip) and category. The choices form an import profile keyed by the header, which also says which panes the file opens with (or the profile's own starter layout), which derived channels its files get (see Derived channels), and, when several files are imported, where they go (each in its own tab, one shared tab, the current tab, or Explorer only). Files with different headers are grouped in the same dialog. Importing the same file again replaces it. Rust converts the file once into the columnar cache; later opens skip the step.
- **Tab and + menus** (tab strip): each tab has a menu (chevron or right-click) with its save status, the files it uses, and Save / Save as / Revert / Share / Delete / Close. The + menu makes an empty tab or opens a layout (yours, or shared in this folder); each entry says what it needs, such as "needs file 2" or "2 files · ready", and a picker shows which file fills each slot before it opens. Your layouts can be deleted from the + menu and the Layouts view, with undo.
- **Start page** (at launch, and from the TelemetryBench name): recent sessions (one per folder: its tabs, open files, time range and cursors), open a folder or a recording, connect a live source (saved connections listed, with a playback demo), and open a layout. A setting skips it at launch.
- **Closing in bulk:** the tab menu has Close tab, Close other tabs and Close all tabs (which also closes the open files); Explorer's folder menu has Close all files. Each bulk close can be undone for a few seconds.
- **Events** (rail): the event list, sorted by start, with add-from-cursors, inline editing, category chips, Expand all / Collapse all, and filters and grouping by file, category, name, channel or source; its Rules button opens the Event rules set. Rules & checks (rail) lists the rule-and-check sets on the files in front (see Checks).
- **Measure** (right): values at cursors A and B, Δt, and exact stats for the visible range or between cursors.
- **Tabs** (top bar): file tabs (a starter layout for one file, which is the primary file while in front) and workspace tabs (a saved layout applied to the open files). Each keeps its own panes, sort and filters, and unsaved changes; the time range is shared. Tabs drag to reorder.
- **Plot stack** (center): panes synced on x, each a chart, table, chart and table, chart and values, values, gauges, checks or XY plot. Panes split right into columns and split down into stacks. A pane title shows its channel chips; on hover it adds only the view switch and ⋯, which holds event markers, axis and scale, close others, close (undoable), duplicate and split. A pane moves by dragging its number. One channel dropped where a new pane goes makes a chart (a setting keeps the source pane's view).
- **Overview strip** (top or bottom, a setting): the full timeline for one channel (following the selected pane, or pinned), with a draggable window showing the current view and a tick per event.
- **Top bar,** left to right: tabs · time range (click to change the time format) · mouse mode (Box zoom, Pan, Cursors), Fit, Back · all panes as charts, tables or both · lock and event markers · remove every channel, remove every pane · ⋯ with the rest, including Focus mode. Narrower windows move the middle groups into ⋯.
- **Status bar** (a setting, off by default): memory, load, query and draw times, and each chart's drawing level.

### **Interactions**

| Action | Input |
| --- | --- |
| Zoom x | Scroll wheel, anchored at the pointer (a setting makes it Ctrl+wheel) |
| Zoom y | Shift + scroll, or drag on the y-axis |
| Box zoom | Drag in Box zoom mode (the default, B). A drag under 4 px isn't a zoom; the box turns solid once releasing will zoom |
| Pan | Drag in Pan mode (P), Shift- or middle-drag in other modes, or arrow keys |
| Cursors | Click in Cursors mode (A, then B), or the A and B keys over a plot or table row; Esc clears |
| Reset / fit | Double-click or double-tap, or F; the view glides to the full range |
| Back / forward | Zoom history with Alt + ← / → |
| Measure | Place both cursors; Measure shows Δt, Δy and frequency |
| Hover readout | Nearest sample per trace, snapping to real samples at high zoom; legend values show while pointing at a plot |
| Select / open (Explorer) | Click selects; double-click or Enter opens; the checkbox plots |
| New pane | Ctrl+Shift+N, or + Add pane |
| Focus mode | Ctrl/Cmd + . (Esc leaves) |

**Calm by default.** Controls answer on press, and nothing appears under a passing pointer: actions show on the selected row, pane titles add only the view switch and ⋯ on hover, and tooltips appear only on icon buttons (a name and shortcut) and retire after a control has been used three times. Readouts, × buttons and resize grips show when they matter. One set of styles covers the whole UI: six font sizes, radii of 4, 8 and 12 px, control heights of 24 and 30 px, two shadow levels, and monospace only for numbers and times. Each of these is a setting (Clicking a row, Tooltips, Quiet interface).

**Motion.** Motion explains where things went and never decorates. Menus grow from the button that opened them and dialogs rise over a dimmed background. Fit, box zoom, Back and event jumps glide over about 0.3 s on a critically damped spring that starts from the view on screen; it runs on the centre point that stays fixed and on log(span), so a 2× and a 200× zoom both look even, and any new wheel, drag or key takes over at once. Cursors and live traces never animate. A box drawn too small fades out ("Drag wider to zoom" the first few times), and zooming past the smallest span says "Maximum zoom". With reduced motion on, nothing animates.

**Tables.** A table pane lists the raw samples inside the current time view, so pan, zoom, the overview and Back move it with the charts. Rows come from Rust, never from the downsampled buffers.

- **Rows**: one per sample of the pane's fastest channel; slower channels hold their last sample, dimmed where unchanged, and gaps are flagged.
- **Paging**: Rust reads rows in whole chunks (about 64k rows), cached with the charts' chunks, and the table asks for the rows in its scroll window (`{pane, t0, t1, offset, limit}` → binary columns), so a view of millions of rows scrolls without loading them all.
- **Sort and filter**: run in Rust over every row in the view (multi-column sort, filters like `> 1.5`, `1..2`, `blank`, joined with and/or); the result is a row-id list the table pages through.
- **Link to charts**: clicking a row places cursor A (Shift: B), double-click centres the charts on it, and the rows on screen show as a shaded band on the charts.
- **Grid**: TanStack Table with virtualized rows, resizable and pinned columns, sticky headers and copy of a selection as CSV. The Columns list scrolls the table to a column when you click its name (showing it first if hidden), and Autofit's first item fits the columns in view to the rows on screen.

### Filters, garbage rows and the charts

A time chart never drops samples silently: anything removed from view is marked where it was. Three tools, from most to least permanent:

| Tool | Where | What it changes | How you see it |
| --- | --- | --- | --- |
| Missing values | Channel settings (right-click a channel → Missing values and display…): raw values to treat as missing (−9999, 65535), a valid range, and an optional Missing when condition on another channel (status == 'ERR') | The data: those samples are gaps everywhere (charts, autoscale, tables, stats, derived channels, rules and checks, exports) | "⌀ 312" on the channel in Explorer; the count in the editor |
| Table filter, Apply to charts | A table's filter strip | This tab's view: rows the filter leaves out become gaps in its charts, autoscale, stats and value tiles. Derived channels, rules and exports still see every sample | A "Filtered · N rows hidden ×" chip on each chart; hatching along the time axis; stats say "Excluding N rows" |
| Ignore outliers | Pane settings → Range | This pane's y-range: it fits the 0.1st–99.9th percentile in view | Spikes beyond it are clipped and marked with a triangle at the edge |

Without Apply to charts, a table filter marks its matches on every chart in the tab (a light shade and a tick along the top). Its strip chooses In view (the rows in the time view) or Whole file (every row; the overview marks them), steps through the matches with ▲ ▼, and Save as rule turns the filter into a rule row in Event rules. Runs closer than 0.25 s count as one match, as event rules join them. A step puts cursor A on the match and scrolls the table to it; with Whole file the charts glide there too. A filter on a slower channel matches whole runs of rows, so the shading covers the slow sample's time.

#### Decisions

- Accepted Known garbage is fixed on the channel (missing values), and applies everywhere.
- Accepted A table filter marks its matches on the charts by default; Apply to charts hides the rows it leaves out, for this tab only, always over the whole file, with a visible chip and hatching.
- Accepted Ignore outliers is a per-axis range option that clips and marks instead of hiding.
- Accepted Masked samples are a view: derived channels, event rules and exports always read the full data (minus channel missing values).

#### Open questions

- **Missing when another channel says so** (status = ERR).
  - Modified Add an optional "Missing when" condition to a channel's Missing values, written in the rules language (`status == 'ERR'`) and matched by time. It applies everywhere: charts, tables, derived channels, checks and exports.
  - Rejected Leave it to Apply to charts with a filter on the status column (it only affects one tab's charts).
- **Keeping a filter as events.**
  - Modified "Save as rule" in the filter strip turns the filter into a rule row in Event rules (prefilled, opened in the editor), so its matches show everywhere as events.
  - Rejected Not needed: rules already cover it.

### Gauges and widgets

A pane can show **Gauges** as one of its views. It draws one widget per channel, reading the value at the pointer, the playhead, cursor A or the end of the view, in that order. Colours come from the channel's limits (normal, warning, alarm), so a gauge and a chart always agree.

| Widget | Best for | Picked automatically when |
| --- | --- | --- |
| Gauge (180° needle) | speed, RPM, voltage | unit is rpm or V |
| Ring | percentages | unit is % |
| Bar | current, power | unit is A or W |
| Tank | temperature, level | unit is °C or the name has temp |
| Sparkline | anything, with the last few seconds | default |
| State light | modes, flags, enums | a flag, or labels with two values |
| Compass | heading | the name has yaw or heading |
| Horizon | attitude | roll in degrees (pitch is found beside it) |
| Number | big readout with a trend arrow | on request |
| Commanded vs actual | setpoint tracking: actual as a bar, target as a tick, difference coloured by tolerance (5% by default) | the pane also has \<name>\_target, \_cmd, \_set, \_sp or \_demand |
| Bar group | motors, cells, tyres: one scale, median line, spread, the odd one out highlighted | three or more channels in the pane differ only by a number, same unit |
| Flags | status words: a named light per bit, bad bits in red | the decoder gives bit names, or the unit is bits |
| State timeline | modes: current state, how long it has lasted, and a strip of states across the view | an enum, or labels with three or more values |
| XY dot | g-g diagram, stick position (dot, 2 s trail, view cloud); GPS track at equal scale | the pane has an \*\_x/\*\_y pair, or lat/lon |

**Pairs and groups** are found from names within the pane, so adding motor\_rpm\_target next to motor\_rpm makes one Commanded vs actual widget instead of two. The menu of a pair or group has Separate / Ungroup; a single widget's menu offers the pairs and groups it can join, and Automatic puts it back. Channels outside the pane are never pulled in.

**Min/max in view** marks the lowest and highest value since the start of the view on the gauge, bar, tank, bullet and bar group: ticks on the gauge, small arrows on bars. On by default; off per widget in its menu.

The range is the channel's own min and max, widened to its limits and rounded to tidy steps. Each widget's ⋯ menu changes its type, sets a range, or sets its size in the grid (S, M, L or 2×2). Choices are stored in the pane (`widgets[channel] = {type, min, max}`), so they travel with layouts and shared views.

**Your own widgets and icons.** The prototype has a first step: a widget is an SVG template with bindings, shared as a TOML file that holds the SVG, (`{{value}}`, `{{unit}}`, `{{label}}`, `{{pct}}`, `{{angle}}`, `{{color}}`, `{{level}}`, `{{min}}`, `{{max}}`, `{{name}}`). The editor shows a live preview, and the SVG is cleaned before use (no scripts, event handlers, foreign objects, images or outside links). The app has two tiers, both in v1:

- **Declarative widgets** (SVG plus bindings, as in the prototype). Safe by construction, shareable as one file, and enough for most dials, levers and indicators.
- **Code widgets**: a folder with `widget.toml` (name, icon, inputs by role such as value, series, or a pair like pitch/roll, and parameters) plus an HTML/JS entry. Each runs in a sandboxed iframe with no network or file access and gets values, a short series, limits and theme colours by `postMessage`, at most once per frame. It can't read other channels or call the core.
- **Icons** are SVGs that use `currentColor`, so they follow the theme. They can be used for categories, widget menus and toolbar buttons that run a recipe or layout.
- **Storage** follows the cache rule: personal widgets and icons live in the app cache. Sharing writes them to `.tlmbench/widgets/` and `.tlmbench/icons/` next to the recordings, like shared layouts. A shared code widget asks once before it runs on someone else's machine.

**Drawing widgets at scale.** File size barely matters: a widget reads one value and, for a trace, a downsampled series (about 110 points for a sparkline, 200 columns for min/max). What costs is the number of widgets and how much of the page each one touches per frame.

| Approach | Good at | Costs | Use for |
| --- | --- | --- | --- |
| SVG kept in the page (attributes updated) | crisp at any zoom, theme colours, screen readers, user templates | a few µs per changed attribute; fine for hundreds of widgets | dials, rings, bars, tanks, flags, compass, horizon, text |
| Canvas 2D | thousands of points per widget | text, theme and HiDPI by hand; no screen readers | sparkline, XY trail and cloud, GPS track |
| WebGL / WebGPU | thousands of widgets or millions of points | about 16 contexts per page, so one shared canvas over all cards; text needs glyph atlases; much more code; WebGPU support uneven in Tauri webviews | only if a dashboard needs thousands of animated traces; reuse the chart pipeline |
| OffscreenCanvas in a worker | drawing off the main thread | data copied or shared to the worker | heavy trace widgets, in v1: canvas, fed by the same M4 reductions as charts |
| HTML/CSS transforms | needles and bars moved on the GPU | only simple shapes | not needed |

**How the prototype draws them (v149).** Each widget still renders to an SVG string, which keeps widgets and user templates easy to write. The page is only touched where something changed:

1. A widget whose inputs are unchanged (time, view, data, settings, check results) is skipped before any work.
2. A quick scan reads the string's structure and values without parsing it.
3. When the structure is unchanged, only the values that changed are written into the elements, which were remembered when the widget was built. Nothing is parsed, built or laid out anew.
4. A widget is rebuilt only when its type or member count changes.
5. Off-screen cards are skipped and drawn when they scroll in.

Work that only depends on the view (traces, min/max, XY clouds, state strips, ranges, the layout of pairs and groups) is cached per view. "How long in this state" uses an index of where each state starts, built once per channel.

**Measured** in the sandbox (2 cores, headless), with 76 widgets on screen out of 112 cards:

| Case | Before | After |
| --- | --- | --- |
| Moving frame (playback, values change) | 34–45 ms | about 7 ms |
| Still frame (paused, nothing changes) | about 30 ms | 0.5 ms |

`t_perf_gauges` checks a moving frame fits a 60 fps frame (under 16 ms; 7–12 ms in the sandbox), that nothing is rebuilt while values move, that still frames are almost free, and that off-screen cards are skipped.

**In the app** the remaining per-widget work moves to the core and to typed updates:

- One call per frame returns every widget's value, min/max and the start of each state run.
- Built-in widgets get an update function (set the needle angle, set the text) instead of a string.
- Traces go to canvas.
- The budget: 100 widgets in under 2 ms per frame.

#### Decisions

- Accepted Gauges are a pane view, not a separate panel, so a layout can mix charts, tables and gauges.
- Accepted Widget type is picked from the unit and name, and can be changed per channel. The choice is saved in the pane.
- Accepted Gauge colours come from channel limits. There are no separate gauge thresholds.
- Accepted Declarative SVG widgets ship in v1, with sanitising and a live preview.
- Modified Code widgets in a sandboxed iframe with a manifest, in v1 (moved from after v1), with the trust prompt decided under Open questions.
- Accepted Five more built-in widgets: Commanded vs actual, Bar group, Flags, State timeline, XY dot (prototype v146).
- Accepted Pairs and groups are found by name within the pane only, and can always be separated or joined from the widget menu.
- Accepted Min/max in view marks are on by default for the gauge, bar, tank, bullet and bar group.
- Accepted Widgets stay SVG, updated in place: skipped when nothing they read changed, values patched when the structure is the same, off-screen cards skipped (prototype v149).
- Accepted Trace-heavy widgets (sparkline, XY, GPS track) draw on canvas in the app; dials stay SVG.
- Accepted No WebGL for widgets in v1; if ever needed, one shared overlay canvas using the chart pipeline.

#### Open questions

- **More built-in widgets?**
  - Accepted Add a min/max "hold" mark on gauges, showing the extremes in the view. Built in v146.
  - Accepted Add a multi-channel bar group (e.g. four motor RPMs or cell voltages side by side). Built in v146.
  - Accepted Add a 2D position widget (top-down track with a dot) for GPS or XY pairs. Built in v146 as the XY dot.
- **Free-form dashboards?** Widgets placed anywhere on a canvas, at any size, instead of a grid in a pane.
  - Rejected Keep the grid in v1; a free canvas later if people ask for it.
  - Modified Keep the grid and add a size per widget: S, M, L, or spanning 2×2, saved with the pane. A free canvas only if people ask for it later.
- **Who can run shared code widgets?**
  - Modified Ask once per widget, showing who shared it and what it can see, and remember the answer per folder; an org setting can turn code widgets off. They run sandboxed with no network or file access either way.
- **How often do widgets update?**
  - Accepted Every frame while playing or live, but text values at most 10 times a second so numbers stay readable; needles, bars and traces move every frame. No setting.
  - Rejected A setting: 60, 30 or 10 updates per second.
- **Typed widget updates in the app?**
  - Accepted Built-in widgets get a small build() and update(values) pair; user SVG templates keep the string path (it is already fast enough).
- **Many widgets on a slow machine?**
  - Accepted If a frame of widgets takes longer than 4 ms, update visible widgets in turns (half each frame) and show nothing different. No setting.

### Time modes: Analyze, Play, Live

A tab is a layout plus a time mode. The same layout works in all three, so the panes, gauges and tables set up for a test bench can be used live, to replay a run, and to analyse it.

| Mode | Where the view goes | How you get there |
| --- | --- | --- |
| Analyze | stays where you put it | default for a file tab |
| Play | follows a playhead moving through the file at a chosen speed | Play back in the toolbar, or Space |
| Live | follows now | a live tab, always |

**The transport** is the strip above the panes in Play: play/pause, the time, a scrubber for the whole file, speed (¼× to 1 day per second, picked so the file plays in about 3 minutes), window presets (10 s, 1 min, 10 min, 1 h) and Follow. The overview keeps working: the playhead shows as a line on it and on every chart, and clicking or brushing the overview moves the view as usual. Panning or zooming stops following while the playhead keeps going; Follow brings the view back. Closing the transport leaves the view where it is. Tiles, gauges and readouts follow the playhead unless you point at a chart. A loop toggle repeats the stretch between cursors A and B, "Stop at" pauses at events (any, or chosen categories), and each tab remembers its playhead, speed and window, restored paused.

Play and Live share the same code path: "view follows a moving time". Live is Play where the playhead is now and the file grows.

#### Decisions

- Accepted Tabs are layout + time mode; no separate playback tab type.
- Accepted Play uses a transport strip with a scrubber, and keeps the overview as it is.
- Accepted Any manual pan or zoom stops following; Follow resumes. Same rule as Live.
- Accepted Switching tabs closes the transport; the view stays.

#### Open questions

- **Does the live "File playback" source stay?** It replays a file as if it were live.
  - Rejected Remove it: Play covers replays.
  - Accepted Keep it for testing rules, alarms and live layouts against a recording in real time, without hardware. Play in a file tab is for watching a replay; File playback is for testing the live path. Pausing it is a real pause.
- **Keep play state per tab?**
  - Accepted Remember each tab's playhead, speed and window, and restore them paused.
  - Rejected Don't; Play always starts fresh.
- **Loop a range?**
  - Accepted Loop between cursors A and B when both are set, with a loop toggle in the transport.
- **Sound or alerts on events during Play?**
  - Modified A "Stop at" choice in the transport: nothing (default), any event, or events of chosen categories. It pauses with the event in view, and the next play continues past it. No sound.

### Checks: expectations over a whole recording

A check states what should be true and gets a verdict for each file: *motor\_rpm < 5000 when flight\_mode is Mission*, *battery\_v > 21 while armed*, *max(esc\_temp\_1) < 80*, *time in failsafe < 5 s*. It builds on two things the app already has. Rules find where a condition holds and make events. Derived channels give an expression language that runs in the core over every sample. A check is a rule turned around: "expect X when S". Its violations are the samples where S holds and X doesn't. On top of a rule it adds a verdict and a report.

| Part | What it is | Example |
| --- | --- | --- |
| Expect | a derived-channel expression that should be true | `motor_rpm < 5000`, `abs(roll) < 30` |
| When (optional) | scope: only these samples count | `flight_mode == 'Mission'`, `armed` |
| Grace | ignore violations shorter than this; settle time after the scope starts | 0.2 s; 2 s after entering Mission |
| Summary checks | one number over the scope, compared with a limit | `max(esc_temp_1) < 80`, `pct(gps_sats >= 10) >= 95`, `duration(failsafe) < 5` |
| Verdict | Pass, Fail, Not applicable (the scope never holds), Can't run (a channel is missing) | Fail · 3 violations · 4.2 s · worst 5,310 rpm at 01:12:40 |

**How it runs.** In the core, in the same pass over the chunks that builds the summary pyramid, so checks cost about as much as opening the file. Channels at different rates are lined up by holding the last value. This suits states and scopes, and it is the same rule derived channels use. Missing values count as "no data" time, not as failures; an option makes them fail. Results are cached in the app cache by file contents and check version. On a read-only folder nothing is written next to the recordings.

**Where you see results.**

Nothing new appears until a set runs on a file. Then results show in places that already exist:

| Where | What you see |
| --- | --- |
| Explorer, file row | ✓ / ✗ count badge; hovering lists the failed groups; clicking opens the Rules & checks panel |
| Explorer, channel row | a small red dot on channels used by a failing check |
| Tab | the same badge, over every file in the tab; it flashes when a live check starts failing |
| Chips | the red dot, on channels used by a failing check |
| Charts and overview | each violation is an event in the Check failed category: a band over its time, listed in Events |
| Gauges | the card turns red and says "✗ check" while the time it reads (pointer, playhead, cursor A) is inside a violation |
| Rules & checks panel (rail) | set → groups → checks, with verdict, count, total time, worst value or the summary value; ▲▼ step through violations; double-click edits |
| Checks pane view | the same summary as a pane, so a layout can carry it; with channels in the pane it shows only the checks that use them |
| Folder run | files × groups grid for one set; click a cell to open that file at its first violation in that group; results as CSV; a Report tab shows pass rates and trends across runs |

**Sets and where they run.** A check set is a named list of checks, each with a group and a severity (Fail or Warn). Sets live in the app cache, and Share writes them to `.tlmbench/checks/`. A set refers to channels by name, so one set works on any file that has them; a check whose channel is missing says "can't run" instead of failing. A set runs on a file when it is attached in one or more of these ways, all chosen in the editor:

- **Every file with these channels.** Files where nothing can run don't show it. Where only some groups can run, those run and the rest show as Not run.
- **This file.**
- **Files with this import profile** (CSV).
- **This layout:** whenever the layout is in front, on every file in its tab.

**Roll-up.** A group takes its worst result: Fail beats Warn, Warn beats error and can't run, and those beat Pass. Not applicable (the When never held) doesn't count. A set takes the worst of its groups. Groups marked info only show their results but never fail the set. A file's badge sums its sets.

**The editor.** Each row has an on/off switch, a kind (Check or Rule), a name, a group, a severity (checks) or event category (rules), Expect (or Mark when), When, grace and settle. The group box is labelled, and its tooltip explains roll-up; a new name starts a group. Expect and When can each be used two ways, and both edit the same saved expression:

- **Lists.** "every sample of / the max of / the min of / the mean of", then a channel, a comparison and a value. Channels with value names offer is / is not and a list of their names. Most checks are this simple, and nothing can be misspelt.
- **ƒ, an expression.** It autocompletes channel names (derived ones too, marked ƒ), functions with their arguments, and value names after `== '`. It checks the syntax as you type. Lists comes back when the expression is simple again.

**Rules in the same editor (v152).** Each row has a kind:

- **Check:** gives pass or fail. It has a severity (Fail or Warn), and its violations become Check failed events.
- **Rule:** marks an event, in its category, wherever its condition holds for at least the grace time. It takes no part in pass/fail.

Rows can be switched between the two, and turned on and off. The panel shows a rule as ⚑ with its event count; groups made only of rules show their event count instead of a verdict. Rule events are listed as "from rules" in Events, as before.

The rules you had moved into a set, "Event rules", that runs on every file:

| Old condition | As an expression | Lists mode |
| --- | --- | --- |
| above / below a level | `ch > a`, `ch < a` | every sample of · channel · > · value |
| magnitude above | `abs(ch) > a` | the magnitude of · channel · > · value |
| outside a band | `ch < a \|\| ch > b` | channel · outside · a and b |
| past warn / alarm limits | `past_warn(ch)`, `past_alarm(ch)` | channel · past its warn limits (the limits are shown) |

`past_warn` and `past_alarm` follow the channel's limits, so a rule or check reruns when you change them. A channel's right-click menu has **Mark when…** next to **Add check…**: it adds a rule row to Event rules, prefilled from the channel's limits. Editing a rule event in Events opens its row in the editor.

Rules run in the same idle-time pass as checks. Sets that contain rules go first, and the Events panel says "checking rules…" until they finish.

Every row shows a live preview on the file in front: "On synthetic\_flight.bin: fails · 3× · 6.58 min · worst 7995 rpm · first at 00:25:47". The top of the editor says where the set runs. Quick starts:

- **Add check…** on a channel's right-click menu, prefilled from its limits.
- **+ From limits** in the editor, or **Check against limits…** in Sets…, adds a check for each alarm limit in the file.

Checks add four functions to the expression language, which work in derived channels too: `pct(cond)`, `duration(cond)`, `occurrences(cond)` (times it became true) and `since(cond)` (seconds since it last held). Value names go in single quotes: `flight_mode == 'Mission'`. An unknown name says which names the channel has.

**In the prototype.** Checks run in idle time, after the first view is up, one check per step, with its channels loaded first. A set runs on each open file in front, and results are kept until the set changes (live files: every 3 s). The sample set "Flight acceptance" (16 checks in GPS, Power, Propulsion, Status and Environment, which is info only) runs on every flight. "ESC bench limits" is attached to the bench file. To see it all:

- **Layouts › Flight acceptance (checks):** a Checks pane, charts and gauges.
- **Layouts › Widgets tour (gauges).**
- **Rules & checks** in the rail, then **Run on folder…**

**If at all?** Yes, but kept small. Expressions plus a handful of summary functions cover most real test specs (thresholds, states, durations, percentages). Anything beyond that, such as sequences ("A within 2 s after B"), or custom statistics, goes to Python outside the app: the Python bindings read the same recordings and .tlmbench/ config, and results come back as events TOML, Parquet or reports. The app never runs Python. Cross-file comparisons are in v1 through derived channels.

#### Decisions

- Accepted Checks in v1, sharing the expression engine and core pass with derived channels and rules (prototype v147).
- Accepted One editor, "Rules and checks": a rule marks events, a check expects and gives a verdict. Same condition, scope, grace and category fields. Built in v152; the old rules popup is gone.
- Accepted Violations become events in a Check failed category, so charts, the overview and stepping work unchanged.
- Accepted Results are cached in the app cache by file contents and set version; nothing is written next to recordings unless you export or share.
- Accepted Folder run in v1: files × groups for one set, click to the first violation, CSV.
- Accepted Sets attach to every file with the channels, a file, an import profile, or a layout.
- Accepted The editor has lists and expressions for the same saved text; lists whenever the expression is simple enough.
- Accepted Roll-up is worst first; Not applicable doesn't count; info-only groups never fail the set.
- Accepted Badges and dots update in place (Explorer, tabs, chips), so selection and scroll never jump.
- Accepted A Checks pane view, so layouts can carry a summary; with channels, it filters to the checks that use them.

#### Open questions

- **How are value labels written in expressions?** Double quotes already mean channel names (`"Adj Close"`).
  - Accepted Single quotes for labels: `flight_mode == 'Mission'`. Built in v147.
  - Rejected A bare word after `==` on a labelled channel: `flight_mode == Mission`. It would clash with channel names.
- **Which summary functions in v1?**
  - Modified min, max, mean, pct(cond), duration(cond), occurrences(cond), first, last. It was count(cond) at first, but count already means "number of samples".
- **Sequences ("after A, B within 2 s")?**
  - Modified Not in v1. `since(cond)` (seconds since cond last held) covers most cases: `since(failsafe) > 2 || rc_link`. Built.
  - Modified Anything since() can't express goes to Python scripts outside the app, using the v1 Python bindings. The app never runs Python. Scripts hand results back as files the app already reads: events in `.tlmbench/events/<recording>.toml`, extra channels as a Parquet file opened beside the recording, and pass/fail as the same JSON or JUnit report `tlmbench check` writes. Script checks run from inside the app (with an installed Python) could come later, if needed.
- **CI and command line?**
  - Modified `tlmbench check <folder> --set <name>` with JUnit, CSV, JSON and HTML reports, in v1 rather than after: see Command line and headless use.
- **Severity?**
  - Accepted Each check is Fail or Warn, so a folder run can pass with warnings. Built.
- **Checks vs limits?**
  - Accepted Limits stay per channel and always on; "Check against limits" turns them into checks when a verdict is wanted. Built.
- **Grace defaults?**
  - Accepted Grace 0 s and settle 0 s by default, set per check.
- **Live alarm behaviour?**
  - Accepted A failing live check flashes the tab badge and adds an event; sound off by default.
- **Should a set that applies to every file also show on files where only some checks can run** (gps\_logger.csv: 1 of 16)?
  - Rejected Show it with "can't run" groups (prototype).
  - Modified Show only the groups where at least one check runs, with one line at the bottom: "N checks can't run here" (expandable). The badge counts only what ran.
- **Folder report as a tab?**
  - Accepted Make it a tab (Report) that is saved with the set and can be reopened and re-run in the app; cells link to each file at its first violation. It is a dialog in the prototype.
- **Comparing runs?**
  - Modified In the Report tab, sort and filter by group, and add a trend column for any summary value across files (e.g. max ESC temperature per flight), drawn as a small chart in date order; clicking a point opens that file. It shows slow drift that every single run still passes.
- **Check results in exports?**
  - Accepted Export to HTML includes the checks summary and marks violations on the charts, from the app and from `tlmbench export html --checks` alike.
- **Where do merged rules and checks live in the UI?**
  - Modified One rail item, Rules & checks. Rules aren't a second list: they are rows in the same sets, next to checks. Events keeps only the events; its Rules button opens the Event rules set. Built in v152.

**Events.** An event is abstract: a name, a start, a duration (0 means an instant), a category and a source (detected from the file, or added by the user). Nothing about shocks, gaps or dropouts is built in. Events come from four sources: the file, the user (markers and ranges), rules, and failed checks. A rule is a row in a rule-and-check set (see Checks): an expression over any channels, derived ones included, held for a minimum time, marked in a category. The core runs it over the samples and makes read-only events that regenerate when the rule, the data or the limits change. Because rules and checks can make many events, the Events list filters by text (name, category, channel) and source, and by category with a row of chips (click hides or shows one, double-click shows only it, All brings everything back). It groups by category, name, channel or source, each group collapsible with a count. Expand all / Collapse all sit in the panel's title row; collapsing an ungrouped list groups it by category first. Editing a rule or check event opens its row in the editor. An event may list the channels it concerns (a detected gap on accel\_x, a GPS dropout across the GPS channels). In tables, every row inside an event gets its category tint and edge stripe, and an Event column carries a tag on the event's first row (▸ name), its last row (◂ end) or the single row of an instant (● name). The cells of the channels it concerns are tinted more strongly. Clicking a tag highlights the event in the Events list and scrolls the table to the first channel it concerns.

- **Categories** are user-defined in Settings: name, color, shown or hidden. Deleting one leaves its events as Uncategorized (grey). Defaults such as Alarm, Data quality, Signal loss, Mode change and Milestone are only a starting set. Categories can also be added from the rules editor (+ New category…) and edited from the Events panel, and are shared as .tlmbench/categories.toml.
- **Adding events.** "Marker at A" adds an instant at cursor A; "Range A–B" spans the two cursors. Each event can be renamed, re-categorized or deleted inline. Events are stored per recording in the app cache; Share writes them to .tlmbench/events/\<recording>.toml.
- **Markers stay quiet.** A range is a wash that fades from a thin bar at the top of the plot to nothing; an instant is a fading hairline under a small notch. There are no labels on the plot: hovering a marker shows a tooltip (name, category, start, duration), and the pointer readout names the event under it, which also covers touch screens.
- **Elsewhere.** Table rows inside an event get a colored stripe and light tint; the overview strip shows a tick per event; the Events list is sorted by start, shows start and duration, and zooms to an event on click.
- **Toggles.** A global Events button (shortcut E) shows or hides every marker; each pane has its own toggle in its header, saved with the workspace layout; category visibility applies underneath both.

**Settings and view lock.** Every toolbar and panel choice is a setting, saved as it changes, with Reset to defaults: theme (system, light, dark; also a toolbar toggle), mouse mode, double-click/double-tap to fit, wheel zoom, event display and categories, overview position, side panels and their widths (sidebars resize by dragging their inner edge). A separate Reset UI layout restores the panels, sidebars, widths and overview position without touching tabs, files or other settings.

Settings opens on a **Basic** view of the twelve everyday choices (mouse mode, opening files, theme, time format, categories, sidebars, how channels from several files are labelled, clicking a row, tooltips, quiet interface), with **All settings** one click away. Each explanation is folded behind an ⓘ. The time format defaults to Automatic: the clock for short files, dates for files with absolute time spanning days, and days or years for long files without; hours, days, weeks and years are also choices. Data rates read in natural units (Hz and kHz, then "every 5 min", hourly, daily, weekly), and rate fields take Hz, per minute, per hour or per day.

**XY panes.** One pane type plots any channel against another (a GPS track, current vs throttle) without changing the time-based model. It uses the samples inside the shared time view, so zooming, panning or brushing a time chart changes what it shows. Cursors A and B and the pointer appear as markers, and hovering or clicking a point moves the crosshair or places a cursor on every time pane. The X channel is chosen in the header. With one Y channel the trace is coloured by time, early to late, with hollow and filled dots marking the start and end of the view; several Y channels use their own colours. Spatial zoom is local to the pane and never changes the shared time view. The toolbar's mouse mode applies as on time panes (Box zoom drags a box, Pan drags the view, Cursors places A and B on the nearest point; Shift- or middle-drag pans in any mode), and scrolling zooms at the pointer; and double-click or Fit to go back to automatic ranges. A zoomed pane keeps its ranges while the time view moves, so you can scrub along a track in a fixed area. A 1:1 toggle keeps both axes at the same scale (longitude corrected by the cosine of latitude); it is on by default for longitude/latitude pairs. Alt-drag is the one link from space back to time: it sets the time view to span the points inside the box. The core pairs samples by time (resampling to the slower channel, keeping gaps) and reduces them to a pixel grid instead of M4, which needs no new storage, only one new query. The pane's view (chart, table, chart and table, chart and values, values, XY, gauges, checks) is picked from a single View menu, which keeps pane headers narrow.

**Channel display and pane axes.** Two levels, so each choice lands in the right place:

- **Channel display**, keyed by channel name and kept in settings like limits, so it applies in every pane, file slot and live source. It covers: name shown (alias), colour (palette or custom), decimals, unit label, scale and offset (raw to engineering units, applied before the pyramid is built; a descriptor can set them too), line width, dashed, and steps for flags and modes. Clicking a channel's chip opens one editor with Display and Limits tabs. Settings › Channels lists everything customised, with a reset per channel and for all.
- **Pane axes**, stored with the pane, so saved and shared layouts keep them. For each axis: range (fit the view, fit including zero, or fixed min/max), a label drawn inside the plot's top corner (so plot areas stay aligned across panes), decimals and log scale. They open from an axis button in the pane header.
- **Pane actions** (new, clear channels, remove all) live in the Explorer's Panes bar, next to the controls that fill panes.

The lock (toolbar, shortcut L, off by default) stops accidental changes on shared monitors and touch screens:

| Locked by default | Still allowed | Setting |
| --- | --- | --- |
| Time view: zoom, pan, fit, Back, event clicks | Hover, table scrolling, tabs, settings | — |
| Layout: moving, closing, splitting or duplicating panes, adding or removing channels, chart/table views, heights, column widths | — | Also lock the layout (on) |
| — | Cursors A and B | Cursors still work while locked (on) |
| — | Table sort and filter | Table sort and filter still work while locked (on) |

While locked, swipes over a chart scroll the page instead of zooming, and a blocked action shows a short "locked · Unlock" notice.

## Files, workspaces and sharing

Everything works from the app cache against read-only folders; only actions you start (Share, Export to HTML) write into a folder.

**Workspace files** save the layout, channel assignments, each pane's view (chart, table or both) with its sort and filters, styling, cursors and annotations as JSON, so a setup can be reopened against a new recording of the same format. Export: visible range or selection to CSV/Parquet, and the view to PNG/SVG.

**Files and multiple recordings.** Real use is a folder of runs, so the app opens a folder and lets several files be open at once. Each open file is another `Source` in the chunk store; this costs UI and time-alignment logic, not new storage.

- **Explorer.** Open folder sets a root; Explorer shows its subfolders and the files a decoder recognizes, with size, duration and status (open, indexing %, primary when several are open). Double-click or drag to open; multi-select opens several; the folder menu imports all. No rename, move or delete: that stays in the OS file manager.
- **Channel identity: numbered file slots.** The primary file is file 1 and keeps plain channel names; each other open file takes the next free slot, and its channels read `accel_x·2`, `accel_x·3`. Slots, not file names, are what layouts store. What people see is the file's short name instead: with more than one file open, channels read "AAPL · Close" in legends, tables, readouts, menus and event labels (a setting shows numbers instead). With more than one file open, the channel tree gains a file level (file → category → channel), and each open file closes from its row in Explorer. Closing a file frees its slot and removes its channels from every pane and tab.
- **The primary file is a role, not a fixed file.** Clicking a file opens it in its own tab with the layout chosen in Settings › When a file opens. The default is the layout that file had last time (remembered per file name); a file opened for the first time gets one table of every channel, the most honest first look at an unknown file. The other choices are starter charts (one pane per group for its first three groups, up to two channels of one unit each), one values pane of every channel, or nothing plotted, or brings its tab forward if it is already open; while a file tab is in front, its file is file 1 and defines time zero. Workspace tabs (your saved layouts or views shared in the folder) apply to whichever files are open; there are no built-in layouts, since the app can't know your channels, and Save as… turns a file tab into a workspace. "Make primary" on any open file swaps it into slot 1; channel names, sort/filter settings and colors are renamed to match, and other files' offsets are re-measured from the new primary file. Closing a file closes its file tab; closing the primary file promotes the lowest-numbered remaining file, and numbers stay compact (1, 2, 3). With nothing open, the plot area shows an empty state, and the timeline length always follows the open files (primary file's start to the last file's end). A tab's layout outlives its files: opening a new primary file into an emptied tab restores its workspace.
- **Time alignment.** All files share one timeline, and each non-primary file has an offset set in the Files panel:

| Mode | Offset | Use |
| --- | --- | --- |
| Same clock | recording clock difference | files logged in the same session (an external GPS logger) |
| Align starts | 0 | comparing separate runs |
| Offset | entered by hand (s, may be negative) | clocks that drift or were never synced |

**Time maps.** Every alignment method, built-in or custom, produces one thing per file: a monotonic, piecewise-linear map from the file's own clock to the shared timeline, `t_shared = map(t_file)`. It is stored as knots `[(t_file, t_shared), …]`. One knot is a plain offset (Align starts, Same clock and Manual above). Two knots add clock drift. More knots follow clock jumps, power cycles and slow wander. Because the map is monotonic it always has an inverse, which the core needs to turn a shared time back into a sample index in that file.

- **Applied at query time, never by rewriting data.** Samples stay at their native timestamps in the chunk store. The core maps query ranges through the inverse, and returned timestamps through the map. Changing alignment is instant and needs no re-indexing. Pyramid buckets stay valid: the map moves and stretches bucket edges but leaves each bucket's min and max unchanged.
- **Everything reads through the map**: charts, tables, cursors and Measure, events (a file's events are stored in its own clock), rules, derived channels that combine files, and exports.
- **Built-in estimators:** Align starts, Same clock and Manual offset. Sync points: pick matching moments in two files; two points give offset plus drift, more give a piecewise fit. Event matching: pair the same rule events across files (an armed flag rising, a throttle step) and fit knots to them. Cross-correlation on a channel both files share, run per window to track drift. Time-source channel: a GPS time or PPS channel in the file becomes the map directly.
- **Custom estimators are WASM plugins**, on the same wasmtime runtime as decoder plugins. They are sandboxed (no file or network access unless granted), run offline and are deterministic, so the cache-only guarantee holds. The core does the windowing and decimation, and a plugin only receives the channels it declares.
- **UI:** each file has an Alignment section, in Explorer and in the tab menu. It shows the method and its parameters, a residuals plot with a quality score, an overlay preview before applying, a manual nudge on top of any method, and a lock. The hover readout shows both clocks, e.g. "file 2 · 01:12:03.40 (own clock 01:12:01.95)".
- **Storage:** the map, method and parameters live in the app cache under the file's identity, like events. A shared layout carries the method and parameters so a colleague can re-run them on the same files, plus the knots for an exact reproduction.
- **Scope:** v1 ships the time-map model with Align starts, Same clock, Manual, Sync points and Event matching. Cross-correlation, time-source channels (GPS or PPS time in the data) and the WASM estimator interface are in v1 too. They only produce knots, so the core does not change.

```rust
/// (t_file_ns, t_shared_ns), strictly increasing in both. One knot = offset, two = offset + drift.
pub struct TimeMap { pub knots: Vec<(i64, i64)> }

pub trait AlignEstimator {
    /// Channels this estimator needs from the reference and target files, given its params.
    fn needs(&self, params: &serde_json::Value) -> Vec<ChannelRef>;
    fn estimate(&self, reference: &[Series], target: &[Series], params: &serde_json::Value) -> Result<Estimate>;
}

pub struct Estimate { pub map: TimeMap, pub residuals: Vec<(i64, f64)>, pub quality: f32 }
```

- **Compare runs.** Select two files and choose Compare (selection bar or right-click), or right-click a file → Compare with the file in front. It opens an ordinary two-file tab with starts aligned and one pane per channel both files share (the first of each group, up to four). Any two files with shared channel names qualify, two stock CSVs included. It is generated from the files, not a built-in layout; Save as… keeps it as a workspace, and because layouts refer to slots it then works for any pair of runs.

**Workspaces, sessions and sharing.** Two things are kept apart: a *workspace* is a layout, and a *session* is what you were doing. Neither is written next to the data unless you ask.

|  | Workspace (layout) | Session | Shared view |
| --- | --- | --- | --- |
| Holds | panes, channels by slot name, views, sort/filters | open files and their slots and alignment, tabs, time range, cursors | a layout plus the files by relative path, slots, alignment and time range |
| Opens files | never; asks for a file when a slot it uses is empty | yes, restores them | yes, opens the listed files |
| Saved | in your app data | automatically, every few seconds | only on Share… |
| Where | app data (user-specific) | app cache by default, keyed by folder path + file fingerprints (size, mtime, hash of the first block) | `.tlmbench/views/<name>.tlmview.json` in the folder |
| Who sees it | you | you | anyone who opens the folder |

- **Shared views appear** in the + menu under "Shared in this folder" and as a `.tlmbench` entry at the top of the Files tree. They behave like built-ins: open and change freely; changes become your own copy with Save as (with "Update shared copy" offered when you can write there).
- **Read-only folders**: Share… reports that it can't write and offers Export file… instead.
- **Slot remapping**: if a shared view lists a file for slot 2 but slot 2 is taken, the file takes the next free slot and the view's channel names follow it.

**Export to HTML.** This is how a view reaches people who don't have the app. Export writes a folder of static files: the viewer (a reduced frontend bundle) plus the decoded data for the chosen layout, files, channels and time range, cut into chunks the viewer fetches as you zoom. It works from any static host with nothing to install. The app's Export and `tlmbench export html` write the same folder (see Command line and headless use, which also sets the size budgets). Binary formats need nothing special, because decoders and plugins have already run: the export holds columns and pyramid levels, never the original bytes. Export takes seconds, since the pyramid already exists, and the view opens in about a second and pans and zooms through the same WebGL path.

- **Integrity rule**: every value a viewer sees comes from raw samples or exact pyramid aggregates, never from drawn lines. Each pyramid entry keeps exact min and max (with sample index), first, last, sum, sum of squares and count. So charts stay true at any zoom, and range stats are exact: whole blocks come from the pyramid, ragged edges from raw samples.
- **What needs raw samples**: table rows, sort and filter, values at a cursor, stats with edges inside a block, derived channels such as `delta`, and XY panes.
- **Two modes**, picked in the export dialog, which shows the size of each: **Full** (the default when it fits the size budget, 250 MB per export folder) ships raw samples for every exported channel, so any pane switches freely between chart, table, values and XY. **Light** ships raw samples only for channels already shown in a table, values or XY view, and pyramid levels for chart-only channels. Switching a chart-only channel to a table there says "raw samples not in this export", or offers summary rows (min, max, mean per block), clearly labelled.
- **Never approximate**: a widget whose raw samples aren't in the file says so instead of showing a pyramid value as if it were a sample.
- **Size**: about 16 MB per million samples before compression, with nothing extra for channels whose time comes from a sample rate.
- **v1 also ships a browser-only mode**: the core compiled to WASM, opening raw files dropped into a browser tab. Descriptors and WASM plugins would work there, native plugins would not. Expect roughly 1.5 to 3x slower than native with no mmap: fixed-width binary still reads lazily through `File.slice`, while CSV needs a full parse, so a few GB is the practical ceiling.

### Decisions

- Accepted Channel prefixes for multi-file tabs are configurable. An import profile sets the rule: file name (default), up to the first \_ - . or space, part of the name by pattern, or custom text. Any single file can take its own alias (right-click → Channel prefix…). Aliases live in the app cache, keyed by file identity.

### Open questions

- **Alignment plugins.** The WASM estimator interface (inputs, decimation, limits), and the external command (decided: not in v1).
  - Accepted WASM interface: `estimate(ref, other, hint) → knots + confidence`. The core decimates both channels to at most 1M samples each and passes the current time map as the hint. Limits: 2 s CPU and 256 MB per call, no file or network access.
  - Modified External command: not in v1 (it would run any program on the machine). WASM covers custom code safely, and a Python script can compute an alignment and save it as a time-map file in `.tlmbench/` for the app to use.
  - Rejected External command: ships in v1 behind an off-by-default setting, reading the same decimated inputs on stdin and writing knots to stdout.

### Shared configuration and version control

Everything a team defines is personal by default (in the app cache) and shared as files in the folder's `.tlmbench/` (Share in the app, or `tlmbench config export`). The app, the CLI and the Python bindings all read the nearest `.tlmbench/`, or `--config <dir>`.

| What | Where it lives | How it's shared |
| --- | --- | --- |
| Events detected from the file | in the recording | with the file |
| Events you add (markers, ranges) | app cache, per recording, by file identity | Share → `.tlmbench/events/<recording>.toml` |
| Rule and check events | not stored: recomputed from the rules when data, rules or limits change; only results are cached | share the rules |
| Rule-and-check sets | app cache | Share → `.tlmbench/checks/<set>.toml` |
| Event categories | settings | Share → `.tlmbench/categories.toml` |
| Derived-channel recipes | app cache | Share → `.tlmbench/recipes/<recipe>.toml` |
| Import profiles | app cache | Share → `.tlmbench/profiles/<profile>.toml` |
| Decoders (format descriptors) | app cache or `.tlmbench/` | `.tlmbench/decoders/<format>.toml` |
| Layouts (shared views) | app cache | Share → `.tlmbench/views/<name>.tlmview.json` |
| Widgets and icons (SVG and code widgets) | app cache | Share → `.tlmbench/widgets/<name>/` (widget.toml plus its SVG or code) and `.tlmbench/icons/` |
| Time maps (alignment between files) | app cache, per file pair | Share → `.tlmbench/timemaps/<name>.toml` |
| Caches (pyramids, check results) | app cache only | never shared, never in `.tlmbench/` |

**Made for version control.** `.tlmbench/` is meant to be committed: next to the recordings, or as its own repository pointed to with `--config`, so many recording folders share one reviewed set of rules. The file rules that keep diffs small and merges easy:

- **One small file per object:** each set, recipe, profile, decoder, layout and per-recording event list is its own file, named from the object. A change to one rule is a one-file diff.
- **Stable output:** the app writes fixed key order and indentation, stable IDs, and no timestamps, "last opened" or other churn. Re-saving an unchanged object rewrites nothing.
- **Portable references:** channels are referred to by name, recordings by slot ("file 1") or by path relative to the folder, never by absolute path. The same files work on every machine and in CI.
- **Versioned:** every file starts with `schema = N`. Older files keep loading after upgrades (the app migrates on save, never silently on read). `tlmbench validate` lints the folder and can run as a pre-commit hook or a CI step.
- **Only configuration:** recordings belong in Git LFS or on a file share. Generated data stays in the app cache, so nothing generated ever lands in a commit. Share offers a `.gitignore` line for recordings when the folder is a git repository.

**Formats.**

- **TOML** is for what people edit by hand and review: rule-and-check sets, recipes, import profiles, decoders, categories and shared events. It is plain text with comments, no indentation rules, clear types and multi-line strings for long expressions. It is widely used (Rust's `Cargo.toml`, Python's `pyproject.toml`) and is read by the Rust core with the `toml` crate and by Python's standard library (`tomllib`).
- **JSON** is for deeply nested files nobody hand-edits: layouts (`.tlmview.json`) and export manifests (`export.json`).

A rule-and-check set in TOML:

```toml
schema = 1
name = "Flight acceptance"
runs_on = "every file with these channels"
info_only = ["Environment"]

[[check]]
name   = "RPM below 7500 in Mission"
group  = "Propulsion"
expect = "motor_rpm < 7500"
when   = "flight_mode == 'Mission'"
grace  = "0.5s"

[[rule]]
name     = "Shock"
group    = "Alarm"
category = "alarm"
mark     = "abs(accel_x) > 1.6"
```

#### Decisions

- Accepted `.tlmbench/` holds only configuration, one small file per object, with stable output and portable references, so it can go straight into version control.
- Accepted TOML for hand-edited configuration (sets, recipes, profiles, decoders, categories, shared events); JSON for layouts and export manifests.
- Accepted Every config file carries `schema = N`; files are migrated on save, never on read; `tlmbench validate` checks a folder.
- Accepted Your own events can be shared: Share writes `.tlmbench/events/<recording>.toml` (stable IDs, one event per table), and the app merges them with your personal ones. They also go into HTML exports and the CLI's JSON reports.
- Accepted Event categories can be shared in `.tlmbench/categories.toml`, so a shared rule keeps its category's name and colour for everyone; your own categories add to the folder's.
- Accepted Caches (pyramids, check results, the session) never go into `.tlmbench/`.

#### Open questions

- **When the folder's categories and yours disagree** (same id, different colour)?
  - Accepted The folder's win for shared rules and events; yours apply only to your own. Settings shows both lists, the folder's marked, so a shared view looks the same for everyone.
- **Who can change shared events?**
  - Accepted Anyone who can write the folder. The app keeps each event's author and shows it, and a conflict (two edits to one event) is a normal git merge of a small TOML file. Permissions belong to the folder and git, not the app.
- **What if a recording referenced from shared files is renamed or moved?**
  - Accepted Match by file identity (size and content hash) as well as by name, and offer to update the references in one click.
- **Keep the prototype's JSON for everything instead?**
  - Accepted No: TOML for hand-edited files as above. The prototype keeps JSON internally; only what's shared changes.

## Derived channels

Derived channels are computed from a file's channels by an expression and ship in v1. They behave like any channel: plotted, tabled, limited, selected and dragged.

**Expressions.** A small language: + − × ÷ ^ %, comparisons and `if(cond, a, b)`. Names are bare or in quotes; `"Close|Adj Close"` takes whichever column the file has. Value labels go in single quotes (state = 'flight'). Channels from another open file are named with its prefix, and interp(ch) resamples a channel onto this one's timestamps.

| Family | Functions | Behaviour |
| --- | --- | --- |
| Math | abs sqrt ln log10 exp pow min max clamp round floor ceil sign sin cos tan atan2 deg rad | Element by element; inputs at different rates resample to the fastest, holding the slower |
| Sample to sample | delta prev(x, n) pct\_change rate cumsum integral running\_max running\_min | Gaps are skipped: prev() of Monday is Friday |
| Rolling | sma ema rolling\_std rolling\_min rolling\_max rolling\_sum | Window in samples (20) or time (30d, 5s) |
| One number | mean std min max first last count sum pct duration occurrences | Plotted as a flat line; Explorer shows the value |
| Filters | movavg lowpass highpass bandpass median | Cutoffs in Hz or as a window; zero-phase on recorded files, causal on live data. A spectrum (FFT) pane view shows any channel over the current time view |

**Recipes.** A recipe is a name, an expression, a unit, a category and decimals, kept in one library in the app cache. It applies to a file when it is marked "Any file with these channels", when the file's import profile picks it, or when it was added to that file, and only if every channel it needs exists. Recipes can use other recipes.

**Import profiles.** Each CSV header gets a profile: time column, rate, names, types, categories. It also holds the recipes its files get and, optionally, its own starter layout. Set up stock data once and every new price file opens with returns and volatility plotted. The import dialog lists the recipes the header can satisfy as checkboxes.

**Where you meet it.**

- Right-click a channel → Derive…; with two selected, A − B, A ÷ B, A × B or A + B. The dialog checks the expression as you type (errors name the character), previews the result, and asks where it applies: this file, this profile, or any file with these channels.
- Derived rows carry ƒ. Right-click → Edit recipe…, or Remove from this file, its profile or every file, with undo.
- Recipes and profiles opens from Settings, the Explorer ⋯ menu and the folder menu. Recipes: edit, duplicate, delete, automatic on or off, export and import as TOML. Profiles: pick recipes, rename, use the current tab as its layout, delete.

**In the app.** The Rust core evaluates the same syntax tree chunk by chunk at the channel's rate, carrying the lookback a rolling window needs across chunks, and builds the pyramid like a raw channel's. Results are cached by file and recipe hash in the app cache; nothing is written beside the source.

### Decisions

- Modified Derived channels in v1 include basic math, delta, filters (moving average, Butterworth low/high/band-pass, median) and a spectrum (FFT) pane view. Changed from "filters and FFT come later".
- Accepted One recipe library; profiles and single files pick recipes from it (chosen over recipes stored inside each profile).
- Accepted Recipes name channels by display name, with `a|b` alternatives for variant column names.
- Accepted Inputs at different rates resample to the fastest input, holding the slower one's last sample.
- Accepted An import profile can carry its own starter layout, used before Settings' choice.

### Open questions

- **Sharing recipes.** How does a recipe reach a colleague?
  - Modified Both: Share writes each recipe to `.tlmbench/recipes/<recipe>.toml` (see Shared configuration and version control), and a shared layout embeds the recipes it plots, offering to add the ones you don't have when it opens. Files are TOML, not JSON.
  - Rejected One team recipe file at `.tlmbench/recipes.json`, written only when you choose Share recipes (replaced by one file per recipe).
- **Cross-file expressions** such as AAPL Close ÷ MSFT Close.
  - Rejected After v1: v1 recipes read one file; cross-file needs the tab's time maps.
  - Modified In v1: reference another file's channels the way the tab already names them (`Close·2`, or the file's prefix such as `MS Close`), resolved through the tab's time map, holding the last sample. A recipe that uses another file applies only in tabs that have it.
- **Interpolation for mixed rates.**
  - Modified Hold the last sample by default, and `interp(x)` for linear interpolation, both in v1. Holding never invents values between samples (states and flags stay sharp); interp is for smooth signals.
  - Rejected Interpolate linearly by default, with `hold(x)` to opt out.
- **Expression engine in the core.**
  - Accepted Our own small parser and chunked evaluator, matching the prototype's grammar and functions (value names, summaries, past\_warn and the rest), with consistent error messages and autocomplete.
  - Rejected Build on an existing crate (evalexpr) and add the series and rolling functions. It evaluates single values, not series, so most of the work would remain.

## Command line and headless use

Everything the app knows how to do with a recording also works without a window, from one command-line tool, `tlmbench`. It uses the same core and reads the same saved configuration. A team's decoders, import profiles, derived channels, rules and checks, and layouts drive conversions, CI checks and shareable HTML views. Nothing is defined twice. All of it is in v1.

**Structure.**

- **`tlmbench-core`** is a Rust library: decoders and sources, the chunk store and pyramid, the expression engine, rules and checks, the layout format and the export writer. It has no Tauri or webview dependency. The desktop app is this core plus the UI; the CLI is this core plus argument parsing.
- **`tlmbench`** is one self-contained binary per OS (Windows, macOS, Linux; x64 and arm64). It is also published as a small Docker image for CI. It needs no runtime and no installed app.
- **The config bundle is shared.** Decoders, import profiles, recipes, rule-and-check sets, event categories, shared events and layouts are files in `.tlmbench/` next to the recordings (TOML, except layouts in JSON; see Shared configuration and version control). The app's Share writes them there. `tlmbench config export --out <dir>` (and the same item in the app) writes your personal ones out of the app cache, so CI uses exactly what you tested with. The CLI uses the nearest `.tlmbench/` above each input, or `--config <dir>`, which can be given more than once; later ones win. Each config file carries a schema version, and every output records the config hash and the core version, so a result can be reproduced.
- **The read-only rule holds.** The CLI reads recordings and writes only under `--out` (or to stdout), never next to the inputs. Its cache (pyramids, check results) goes to the same app cache as the desktop app, or to `--cache <dir>` in CI.

| Command | What it does |
| --- | --- |
| `tlmbench convert <files> --to csv\|parquet\|arrow` | decode any supported format and write common formats, with derived channels if asked |
| `tlmbench check <files\|folders> --set <name>` | run rule-and-check sets; reports and an exit code for CI |
| `tlmbench export html <files> --layout <name>` | write a static, zero-install viewer of a layout over the decoded data |
| `tlmbench info <file>` | channels, units, rates, duration, time base, gaps; `--json` for scripts |
| `tlmbench validate [<config dir>]` | lint decoders, profiles, recipes, sets and layouts, and try them on a sample file if given |
| `tlmbench derive <file> --expr "…"` | evaluate one expression; a series as CSV on stdout, or one number |
| `tlmbench config export` | write the app cache's profiles, recipes, rule-and-check sets, categories, events, widgets and layouts to .tlmbench/ |

The same operations are library calls, and v1 also ships Python bindings over them (pyo3, wheels on PyPI): open a file, list channels, read a range as NumPy or Arrow, derive, check, convert and export. That is where the scripting API from the checks section lives.

### Converting formats

```
tlmbench convert flight.bin --to parquet --out out/ [--profile "ESC telemetry"]
    [--channels "motor_*,battery_v"] [--derived] [--from 00:10:00 --until 00:40:00]
    [--resample 100Hz | --layout long] [--time seconds|iso|ns] [--raw]
```

- **Streams chunk by chunk.** Memory stays flat on 100 GB inputs, and folders convert in parallel (`--jobs`).
- **`--derived`** adds the recipes that apply to the file (auto, its profile, or attached to it) as real columns. `--expr name="…"` adds one-offs.
- **Display settings apply by default.** Profile scale/offset and missing values are applied, as in the app. `--raw` writes the decoded raw values.
- **Channels at different rates (decided).** By default each rate group gets its own file: `flight.200Hz.csv`, `flight.10Hz.csv`, and so on, each with its own time column and only real samples. Nothing is invented or repeated, every file opens in a spreadsheet, and a `flight.channels.csv` manifest lists channels, units, rates and files. Two options:
  - `--resample <rate>` writes one wide file on a single grid. Slower channels hold their last value, the same rule derived channels use, and a header comment says so.
  - `--layout long` writes one tidy file: time, channel, value.

  Parquet and Arrow follow the same default (one file per rate group) and the same two options. Channels with irregular timestamps go to their own file with their own time column.
- **Outputs.** CSV (with a header row, `--delimiter`, and units in a second header row if asked), Parquet (zstd, one row group per chunk) and Arrow IPC. MAT (v7.3) and HDF5 are in v1; MAT v7.3 is HDF5 underneath, so one writer covers both.

### Checking files

```
tlmbench check flights/ --set "Flight acceptance" [--set "ESC bench limits"] [--sets-from-profile]
    --report junit,csv,json,html --out reports/ [--jobs 8] [--fail-on warn]
```

- **Same results as the app.** It runs exactly what the app's folder run does: the same expressions, When and settle, grace, the worst-first roll-up, info-only groups, and Fail or Warn. Rules in the sets also run, and their events are listed in the JSON and HTML reports.
- **`--sets-from-profile`** uses whichever sets each file's import profile names, so CI and the app agree without listing sets twice.
- **Exit codes.** 0 all pass; 1 a check failed; 2 a file or check couldn't run (missing channel, unreadable file); 3 a configuration error. `--fail-on warn` treats warnings as failures, and `--allow-cant-run` keeps 2 from failing the build.
- **Reports.**
  - JUnit: each file is a suite and each check a test case, with the worst value and first time in the message.
  - CSV: one row per file and check, as in the app's folder-run CSV.
  - JSON: everything, including violation times.
  - HTML: the files × groups grid. It is the same static viewer as Export to HTML, and each failing cell links to an exported view of that file at its first violation (see below).
- **Cached.** Results are keyed by file contents and set version, so re-running a campaign only checks new or changed files.

### Headless Export to HTML (zero install)

```
tlmbench export html flight.bin [gps_logger.csv] --layout "Flight review" --out site/
    [--checks "Flight acceptance"] [--from … --until …] [--budget 250MB] [--mode full|light]
```

The export is a folder of static files: `index.html`, the viewer's script and styles, and data files. It works from any static host or file share (S3, GitHub Pages, an internal web server, a CI artifact) with no server code, no sign-in of its own and nothing to install. Access is whatever the host already enforces. The app's Export to HTML writes the same folder, so the two always match; it is one export, not two.

- **What it holds.** The layout's panes for the files given (slots filled in order, alignment as saved), only the channels the layout uses (`--all-channels` to include the rest), events, and optionally a checks summary with stepping through violations. Data files are chunked columns and pyramid levels, never the original bytes. The viewer fetches chunks as you zoom, so opening loads about 1–2 MB (the viewer and the overview level) whatever the export's size.
- **What the viewer can do.** Pan, zoom, Back and Fit; cursors and Measure; the time modes (Analyze and Play); switching a pane between chart, table, values, gauges and XY where the data allows; events with the category chips; the checks summary. It cannot import files, edit layouts, rules or checks, or make derived channels. Derived channels the layout shows are computed at export time and stored like any channel.
- **Size budgets (decided).** The default budget is 250 MB per export folder. Only the parts someone looks at are downloaded, so the budget mainly limits what sits on the host.
  - **Full mode** (raw samples for every exported channel, so every pane can be a table or XY) is chosen automatically when it fits the budget.
  - **Light mode** is chosen otherwise. It ships the full pyramid for every channel, plus raw samples for the channels the layout shows as tables, values or XY. It also ships raw samples in a window around every event and check violation: 30 s, or 3× the event's duration when longer, capped at 10 min, and configurable. Stepping to a failure is exact there.
  - **If Light still doesn't fit,** the finest pyramid levels are left out first. The viewer then says "finer detail not in this export" past that zoom instead of guessing.
  - The export prints, and writes to `export.json`, what it included and its size per channel. `--budget` and `--mode` override.
- **Integrity rule unchanged.** Every value shown comes from raw samples or exact pyramid aggregates, and a view whose raw samples aren't in the export says so.
- **Safety.** The data carries no code. User SVG widgets are sanitised as in the app, and code widgets export as plain value cards, without their code. `--redact <channels>` leaves channels out even if the layout uses them.
- **One-file variant.** `--single-file` packs the same export into one HTML file for email or `file://`. It is still static, but every chunk is inlined, so it has its own smaller default budget of 25 MB, Light mode, and the coarser levels only.
- **In CI.** `tlmbench check … --report html` together with `tlmbench export html … --checks …` gives a report whose failing cells open the exported view at the violation, ready to link from a pull request or a test log.

### Decisions

- Accepted The CLI is in v1: convert, check, export html, info, validate, derive and config export.
- Accepted One core library for the app and the CLI; the CLI is a self-contained binary per OS, plus a Docker image.
- Accepted One config bundle (`.tlmbench/` files, schema-versioned) drives both; `config export` moves personal settings into it.
- Accepted Multi-rate conversion writes one file per rate group by default, with only real samples; `--resample` and `--layout long` on request.
- Accepted Exports are static files (a folder), the same for the app and the CLI, with no server code; access is the host's.
- Accepted Export budget 250 MB per folder: Full when it fits, otherwise Light with raw windows around events and violations, then coarser levels; one-file variant 25 MB.
- Accepted `check` exit codes 0/1/2/3, with JUnit, CSV, JSON and HTML reports, and results cached by file contents and set version.
- Accepted Python bindings in v1, over the same core library.
- Accepted Two more commands: `tlmbench cache` (build, list and clear cache entries, as Settings does) and `tlmbench query` (a view as M4 points, values at a time, or exact stats over a range, as CSV or JSON). They make the store testable headlessly and are what the Python bindings wrap. `info` also reports gaps once the gap index exists.

### Open questions

- **How wide should the raw windows around events and violations be?**
  - Accepted 30 s, or 3× the event's duration when that is longer, centred on it and capped at 10 min. Configurable: `--detail 30s` (or `--detail 3x,30s,10min` for factor, minimum and cap) in the CLI, the same three fields in the app's export dialog, and a default per rule-and-check set. `--detail 0` turns the windows off.
- **Should a CLI run record what it did in the export or report?**
  - Accepted Yes: command line, config hash, core version and input file hashes in `export.json` and the report's footer.
- **Python bindings in v1 too?**
  - Accepted Yes, in v1: a thin layer over the same library calls (open, channels, read a range as NumPy or Arrow, derive, check, convert, export), published to PyPI as wheels for each OS. No separate engine.
  - Rejected After v1, once the CLI's operations have settled.
- **Watch mode for test benches?**
  - Accepted In v1: `tlmbench check --watch <folder>` keeps running and checks each new recording once it is complete (its size has stopped changing, or its writer has closed it). It updates the reports and can export a view of each failing file, for a test-bench PC that records run after run with no one at it.
- **Signed exports?**
  - Accepted Add a checksum manifest: `export.json` lists the SHA-256 of every file in the export, and the viewer checks each chunk against it as it loads. A truncated upload or a stale cached chunk then shows an error instead of wrong data. Combined with the recorded input hashes, anyone can confirm a shared view really came from a given recording and config. No signing in v1.

## Performance targets and testing

These targets are starting points for a mid-range laptop (8 cores, 16 GB, SSD); tune them once the prototype exists.

| Metric | Target |
| --- | --- |
| First paint, 10 GB fixed-width binary | < 1 s (mmap + coarse pyramid level) |
| First open, 10 GB CSV (parse + cache) | Shows progress immediately; \~1 GB/s parse across cores |
| Reopen of a cached file | < 1 s |
| Pan/zoom frame time | < 16 ms with 20 traces |
| Viewport query in Rust | < 5 ms per trace |
| Live ingest | ≥ 4M samples/s total without drops (covers 1 MHz × 4 and 10 kHz × 32 channels) |
| Live display latency | < 50 ms sample-to-pixel |
| Idle RAM | < 300 MB plus cache budget |
| Gauges frame (playback) | < 2 ms for 100 widgets; a still frame ≈ 0 |
| Rules and checks | About one extra read pass per file, in idle time; never delays drawing |
| HTML export, first view | < 2 s from a static host at the default budget |

### **Testing**

- Property tests (`proptest`): M4 output from the pyramid equals M4 computed from raw data for random series and viewports.
- Golden image tests: render known series headlessly and compare pixels against stored PNGs.
- Synthetic data generator: sine + noise + spikes + gaps at configurable rates and sizes (1 GB, 10 GB, 100 GB).
- Benchmarks with `criterion` for parsing, pyramid build and viewport queries, tracked per commit.
- Fuzz the binary decoders (`cargo-fuzz`) since they parse untrusted files.
- Golden-output tests for the CLI and Python bindings: convert, check, derive and export html on fixed recordings, compared byte for byte (exports through their checksum manifest).
- Prototype suite: 21 browser tests (Playwright) with one runner, `./run.sh`. It runs three at a time, then the timing tests alone: how fast rows appear, and a 100+ widget frame. Each test checks behaviour, not pixels (the chart and table libraries are stubbed). The same flows become the app's end-to-end tests against the real core.

## Roadmap

Build the riskiest loop first — Rust reduces, IPC transfers, WebGL draws — and only add the pyramid, live sources and format tooling once each gate is met.

&#91;embedded content: roadmap · 5 phases, each closed by a measurable gate\]

The phases and their gates, as the diagram shows them (Phases 1 to 3 are done; Phase 4 is next):

1. Core loop: fixed-width and CSV, M4 from raw data, a single WebGL pane, the binary IPC path. Gate 1: 100M points at 60 fps.
2. Scale: pyramid and cache files, the LRU chunk store, the plot stack and overview, hover and cursors. Gate 2: 10 GB first paint under 1 s.
3. Live: serial, UDP and TCP sources, ring buffer ingest, follow mode, auto-recording. Gate 3: 4M samples/s with no drops.
4. Formats and polish: the TOML descriptor UI, compressed packets, data export, workspace files. Gate 4: formats complete.
5. Workbench: multiple windows and widgets, rules, checks and reports, derived channels, shared `.tlmbench/` config, the CLI and Python bindings, static HTML export. Gate 5: a folder check and a 2 h HTML export within budget; the v1 release.

Phase 1 deliberately skips the pyramid: computing M4 straight from mmapped raw data is fast enough up to \~100M points and teaches the downsampling core before adding caching.

**Phase 1 result (Nov 1, 2026): the gate passes** on a MacBook Air M1 (8 GB, 59 Hz display, 2264 px wide pane) over 100M f32 samples in a `.tlm` file. In a 30 s scripted sweep, 3 of 1,799 frames missed a vsync (0.17%), the longest interval was 41 ms, and the core reduced views of up to 1M samples in 0.5 ms (median) and 1.4 ms (p95). Whole-file views took 21 ms (median), and 657 ms on the first, cold read of the file: bounding those is the pyramid's job. The UI's round trip was 4 ms (median), and moving 64 KB over IPC took under 1 ms.

**Gate 2 result (Nov 2, 2026): the gate passes** on the same Mac (1736 px wide panes) over a 10 GB `.tlm` of 20 channels × 125M f32 samples. Reopened with its pyramid cached and its pages evicted from memory, the whole view of all 20 channels drew in 70 ms (coarse) and exactly in 1.3 s. A 30 s sweep over the 20 traces missed 1 of 1,800 vsyncs, with a longest interval of 40 ms; the core answered zoomed views of all 20 channels in 4.4 ms (p95; 15.7 ms before the chunk store and view batches). A first-ever open showed axes and build progress in 62 ms and built the pyramid in 9.9 s (12.5 s in the first run), against 9.5 s to read the file cold; the pyramid adds 1.1 GB to the cache.

**Gate 3 result (Nov 2, 2026): the gate passes** on the same Mac. Core (`just gate3 core`): a sender streamed 4M f32 samples/s over TCP for 60 s in each of two shapes, 4 channels at 1 MHz and 32 at 125 kHz, and the recorder kept every record, identical to what was sent, with nothing dropped or missing and its ring buffer at most 0.8% full, at 3.7% and 6.5% of a core. Reported: UDP, the same with a larger receive buffer (45% and 22% of a core); CSV tails kept up at 1M and 4M samples/s. App (`just gate3 app`): a live tab followed each stream with a 10 s window and missed 0 of 3,539 and 3 of 3,541 vsyncs (longest 24 and 36 ms), with views asked for 30 times a second (p95 under 20 ms), and the session's recording matched the sender record for record. Phase 3 ships serial, UDP, TCP and growing-file sources, ring buffer ingest, sessions recorded to the Recordings folder, pyramids that grow, live tabs that follow now, and the app's UI ported to the prototype's (shell, panes, Explorer, tabs, Play).

Phase 5, Workbench, holds the rest of v1: multiple windows, widgets, rules and checks with reports, derived channels, shared `.tlmbench/` config, the CLI and Python bindings, static HTML export and the browser-only mode (its cache lives in the browser's private file storage, so nothing is written next to the data). Its gate is a folder check plus an HTML export of a 2 h flight within the default budget; passing it is the v1 release.

The command-line tool ships in v1 with the app. It is built on the same core from Phase 1 on, so each phase's work is usable headlessly as soon as it lands. Its own gate: `convert` and `check` on a 10 GB file run at disk-read speed with flat memory, and `export html` of a 2 h flight fits the default budget and opens in under 2 s from a static host.

### Decisions

- Accepted How the Phase 1 gate is judged (`just gate`): a scripted 30 s sweep in the release app (zoom from the whole file to 1 s and back, pans), one view per frame. It passes when at most 1% of frames miss a vsync (an interval over 1.5 refresh periods) and none exceeds 3 periods, and when the core reduces views of up to 1M samples in p95 under 5 ms, timed in Rust. Missed vsyncs replace a 16.7 ms p95 because WebKit rounds the page's clocks to 1 ms. Whole-file views are reported, not gated. A run in which the window was hidden counts as invalid, not as a fail.
- Accepted The Phase 1 gate uses one 100M-sample trace; the 20-trace target is reported later, once the pyramid exists.
- Accepted How Gate 2 (10 GB first paint under 1 s) is judged, since reading 10 GB takes at least 3 s on the development Mac: reopening a 10 GB recording with its pyramid cached, after the OS file cache is flushed (`purge`), draws the whole view in under 1 s; a first-ever open shows axes and build progress in under 1 s and builds the pyramid at disk speed (reported, not gated). The recording is a 10 GB `.tlm` of 20 channels × 125M f32 samples, which also reports the 20-trace target. It is generated on the internal disk for the run and deleted afterwards.
- Modified `just gate2` builds the pyramid with the CLI, drops the recording's and its cache entry's pages from the file cache (msync with MS\_INVALIDATE, no root; changed from `sudo purge`), then opens the file in the release app and times the whole view drawn; then it clears that cache entry, flushes again and times a first open (axes and progress, then the build). The 20 channels sit in a stack of panes; a 30 s sweep like Phase 1's reports the 20-trace frame times. `just gen --size 10G --channels 20` writes the file.
- Accepted Gate 2(a) times the first paint, which is the coarse view (it hides no extreme and is labelled), and reports how long the exact view takes to replace it. The flush can also be done without root: `msync(MS_INVALIDATE)` on a map of each file drops its pages on macOS, which evicts just the recording and its cache entry.
- Accepted The app takes the prototype's UI at the start of Phase 3, before Live's UI is built: the shell (top bar, tabs, Explorer, the pane stack with its menus), the time modes and transport (Analyze, Play, Live), settings, and the prototype's Playwright suite turned into the app's end-to-end tests. Phase 3's core work (sources, ring buffer ingest, Gate 3) goes on alongside, since it doesn't touch the UI. Why then: follow mode, the transport and auto-recording are already designed in the prototype, so building them in the interim UI would mean building them twice, and hover, cursors and Measure gain tests with the port. Later options: in Phase 4 (Live ships with interim UI), or in Phase 5 (the interim UI grows through two more phases). The prototype's features beyond that (widgets, checks, derived channels, sharing) come over in the phases that build them.
- Accepted The port uses Svelte with TypeScript: compiled components with no virtual DOM suit a UI that is mostly canvas and WebGL with a little DOM around it, and its small runtime keeps the HTML export light. The renderer, axes and wire decoder stay plain TypeScript modules. Alternatives: React (larger ecosystem, heavier runtime), or no framework (the prototype's own approach, harder to keep consistent at its size of about 11k lines).
- Accepted How Gate 3 (4M samples/s, no drops) is judged, `just gate3`: a synthetic sender (`tlmbench-gen --send`) streams to the release app over TCP on localhost at a fixed rate, in two runs of 60 s each: 4 channels at 1 MHz and 32 channels at 125 kHz (4M f32 samples/s each, the two shapes the live target names). Every frame carries a sequence number and deterministic values. The app ingests it as a live source with auto-recording on, and a live tab follows it (window 10 s), drawing every frame. A drop is any of: a missing or repeated sequence number at the decoder, a frame refused by a full ring buffer, or a sample missing from or different in the session recording, checked against the sender afterwards (count and checksum per channel). It passes with no drops, the ring buffer never more than half full, and at most 1% missed vsyncs as in Gate 1. Reported: samples sent, received and recorded, the ring's high-water mark, sample-to-pixel latency (p50, p95), the disk write rate, CPU and memory. UDP is run the same way and reported, not gated, since localhost UDP can lose packets in the kernel's buffers before the app sees them (those losses are counted apart). Until the live tab exists, the same sender checks the core alone (ingest and recording, headless) as `just gate3 core`.
- Accepted How Gate 4 (formats complete) is judged, `just gate4`: one synthetic recording is written as `.tlm`, CSV, Parquet, a big-endian scaled fixed-width file with its own descriptor, and packet files compressed with zstd, lz4 and gzip. Each opens in the CLI and the app with the same per-channel count and checksum and the same views, and `convert` from each to CSV, Parquet, Arrow and MAT/HDF5 reads back exactly. A 10 GB compressed packet file also reopens with its index cached and draws its whole view in under 1 s; its first index build runs at disk speed (reported, not gated).
- Accepted Phase 4's exports are everything `convert` lists: CSV, Parquet, Arrow IPC, and MAT v7.3 with HDF5 (one writer, libhdf5 built statically), so "formats complete" holds. The app's Export (visible range or selection) writes the same files.
- Accepted The descriptor form is drafted in the prototype first (a fields table, framing and packets, a preview of decoded rows, the TOML beside it) and reviewed there, then ported to the app, as the rest of the UI was.
- Proposed The descriptor form (drafted in the prototype, `t_descriptor`): opening a binary file that no descriptor claims opens the form on the file's own bytes. You pick a descriptor you have or describe the layout (framing, byte order, file header, time, a fields table), and a preview decodes the first records as you edit: the first bytes in hex tinted by field, a table of decoded rows with each checksum, and problems worded as the decoder words them, with Open disabled until they're fixed. Find frames proposes a sync word that repeats at a fixed spacing. The TOML is shown as written. Save keeps the descriptor in your decoders (app cache); Share… writes it to `.tlmbench/decoders/<name>.toml`; Open saves it and opens the file. Right-click a binary file → Format descriptor… edits it.
- Proposed A descriptor says which files it decodes with a `files` key of name patterns (`files = ["esc_bench_*.bin"]`, `*` matching any text). A binary file opens through the first descriptor whose pattern matches its name, so one descriptor covers every file from a logger; the form proposes the pattern from the file's name with its digits as `*`.
- Proposed `[packets]` may leave out the length field: each packet then holds one record (fixed-size frames with a sync word and a checksum, a common logger format). A compressed payload still needs a length.
- Proposed The app's Export (drafted in the prototype, `t_export`): the ⋯ menu's Export group holds Export data…, Save the view as an image… and Export to HTML…. Export data picks the format (CSV, Parquet, Arrow IPC, HDF5, MAT v7.3), the time (the time view, between cursors A and B, or the whole recording), the channels (this tab's panes, or every channel of the open files), wide or long, rows as recorded or resampled to a rate, how times are written, and stored values. It shows the channel, row and column counts, the size, the files (`<name>.<ext>` and `<name>.channels.csv`) and the matching `tlmbench convert` command, and remembers the choices. Save the view as an image writes every pane or the selected one as PNG (as on screen or twice as sharp) or SVG.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Tauri IPC throughput for large binary payloads | Laggy zoom if transfers are slow | Payloads are capped at \~4W points per trace (\~100 KB); benchmark early in Phase 1 |
| WebView differences (WebView2, WKWebView, WebKitGTK) | WebGL bugs or slower GPU paths on Linux | Test all three in CI; keep a Canvas2D fallback |
| Compressed formats without random access | Slow seeks into the middle of a file | One-time packet index in the app cache (never beside the file); recompress to cache if packets are huge |
| CSV quirks (locales, quoted fields, mixed timestamps) | Wrong or failed parses | Import dialog with preview, explicit column types and timestamp format |
| Pyramid build time on first open | Wait before full-res browsing | Build in one background pass at disk speed, cached for later opens; stream progress, and fill panes left to right with exact data |
| v1 scope (multiple windows, checks, derived channels, widgets, CLI, Python, sharing) | A late release or uneven quality | Phases with measurable gates; each feature lands in the core and CLI first, so it is tested headlessly before it gets UI |
| Shared code widgets and plugins | Untrusted code arriving with a shared folder | Trust prompt per folder; plugins in the WASM sandbox; HTML exports drop the code |
