# Follow-ups

Work that is planned but deliberately not done yet. Remove an item when it lands.

## Large test recordings (10 GB and 100 GB)

The design's testing section calls for synthetic recordings at 1 GB, 10 GB and 100 GB (sine + noise + spikes + gaps).
Phase 1 needs only the 1 GB set (100M points as fixed-width binary is about 0.4–1.2 GB). The larger sets wait for an
external SSD: the development Mac has about 24 GB free.

When needed:

- The generator (added in Phase 1) is deterministic from a seed, so recordings are rebuilt, never stored or committed.
- Write them outside the repo to a folder named by `TLMBENCH_DATA` (default `~/tlmbench-data`); point it at the
  external drive for 10 GB and 100 GB. Add `just gen <size>`.
- 10 GB serves the first-paint target and the CLI gate (`convert` and `check` at disk-read speed, flat memory);
  100 GB serves `convert`'s flat-memory claim. Neither suits CI runners (about 14 GB free); run them on the Mac.
- Add those folders to `ro_commands` coverage, mounted read-only (`hdiutil` image or a read-only external volume).

## Benchmark tracking per commit

The design asks for criterion benchmarks "tracked per commit". CI only runs them once untimed (`bench-smoke`),
because shared runners are too noisy. To decide: a dedicated runner, a hosted tracker, or results saved locally with
`just bench -- --save-baseline`.

## Phase 1 loose ends

- The CI image doesn't include the app UI's npm packages; the Linux job installs them on each run.

## Phase 2 loose ends

- An exact whole view of a cold 10 GB, 20-channel file takes 1.7-2.3 s (the coarse view draws in 0.22 s). Most of it
  is level 4 at every pixel edge; storing each pixel edge's straddling extremes differently, or a finer
  coarse-to-exact handoff (only the pixels whose edges matter), could cut it.
- A writer thread for pyramid builds was tried and left out: writes take about 0.6 s of a 5–6 s cold build of the
  10 GB file (the CLI, M1, `ci/evict.py` before each run), and overlapping them with the reads gave 5.4–7.5 s
  against 5.7–6.6 s without, within run-to-run noise: reads and writes share the one SSD. Gate 2's first-ever
  build took 9.9 s in its last run (12.5 s before), against 9.5 s to read the file cold.


## Phase 3 (done Nov 2, 2026): what was left for later

- Play's "Stop at" events waits for events in the app (Phase 5's rules and events).
- The pane view switch (chart, table, both, values) and "Axis and scale…" wait for tables and axis settings (Phase 4).
- Live views are whole views at 30 Hz: the design's incremental path (the core sends only the buckets that changed,
  the UI appends them with bufferSubData) would cut the core's work and the replies' size.
- A live session holds one source in the app; "+ Add source" (several sources on one time axis, named `·2`) and
  the session scrubber's faint overview trace are still to come, as is saving a connection to the start page.
- A recorded session isn't listed on a start page yet (the app has none); its files open like any recording.

## Phase 3 loose ends

- A UDP source reads one datagram per call, which costs 45% of a core at 4 × 1 MHz in 8 KB datagrams (TCP: 3.7%).
  Reading several per call (`recvmmsg` on Linux) would cut it; it is reported, not gated.
- A tail polls every 20 ms, so a truncation in place loses what was written in the last moments before it (about
  20 ms). Watching the file (kqueue, inotify) would shorten that window; renaming rotations lose nothing.

## Phase 4 (in progress): where it stands

Decisions are in the design's Roadmap and Data ingestion decisions (Gate 4 = a format matrix plus a 10 GB compressed
packet file; exports CSV, Parquet, Arrow, MAT/HDF5; `[packets]` keys; packet types open one recording each; records
read as header then record). Done and pushed:

- Parquet input through the column cache (`crates/decode/src/parquet.rs`), with a page-header check and a panic guard
  against damaged files (`parquet_pages.rs`, fuzz target `parquet`).
- `tlmbench convert` to CSV, Parquet, Arrow IPC, HDF5 and MAT v7.3 (`crates/export`): wide or long, `--resample`,
  `--time`, `--raw`, channel globs, a span, `<name>.channels.csv`. The CSV reader skips leading `#` lines (Proposed).
- Packet files: `[packets]` (sync, optional length, checksum, zstd/lz4/gzip), a packet index in the cache, packets
  decoded on demand (`crates/decode/src/packets.rs`, fuzz target `packets`); `tlmbench-gen --format zstd,lz4,gzip`.
- `[[packet_types]]` (`[packets] id`, one recording per type; `crates/decode/src/packets.rs`, `Store::open_all`,
  CLI `--type`, `convert` writes `<name>.<type>.<ext>`). The prototype's form doesn't know types yet.
- The descriptor form, drafted in the prototype (`t_descriptor`) for the owner's review; three Proposed decisions
  (the form, `files = [...]` name patterns, packets without a length).

Next, in order:

1. The app's slots for packet types (one recording per type, `flight.bin · gps`): with the descriptor port in step 2,
   since the app doesn't open descriptor files yet. Done in the core: `[[packet_types]]`, `Store::open_all`/`open_part`,
   CLI `--type`, `info` and `convert` per type.
2. Once the owner accepts the form: port it to the app (descriptor lookup by `files` pattern in the app cache and
   `.tlmbench/decoders/`, the form on an unclaimed binary file, Open through the descriptor); the CLI finds
   descriptors the same way when `--descriptor` is left out.
3. The pane view switch and tables: done are the view menu (Chart, Table, Chart and table, Values), value tiles, and
   table panes over the `rows` command (a window of raw rows read as the table scrolls; row click places A, Shift B,
   double-click centres; Go to A/B; resizable columns and the table's height saved with the layout). Left for
   tables: sort and filter in Rust (a row-id list the table pages through), the Columns list, Autofit, pinned
   columns, copy as CSV, and the rows on screen as a band on the charts. "Axis and scale…" is done (a unit's axis
   each, left and right; range, decimals, label, log).
4. Workspaces (design: Files, workspaces and sharing). Done: your layouts in app data, by channel name (the
   `.tlmview.json` shape): Save as… / Save layout / Revert / Delete (Undo) in a tab's menu, ⌘S, the + menu (empty
   layout, your layouts), workspace tabs on the file in front, restored with the session. Shared views: the + menu
   lists `.tlmbench/views/` of Explorer's folder (opening one opens its file, layout and time range, read-only);
   Share with this folder… / Update shared copy…; Export file… (also offered when the folder can't be written).
   Left: the `.tlmbench` entry at the top of Explorer's tree, the Layouts sidebar, and slots once tabs hold several
   files (a layout then asks for a file per empty slot; shared views open all their files).
5. The app's Export (visible range or selection → the `convert` writers; PNG/SVG of the view): drafted in the
   prototype (`t_export`) as a Proposed decision; port it once the owner accepts it.
6. `just gate4`: the format matrix (one recording in every format with identical per-channel checksums and views;
   `convert` round trips) and the 10 GB zstd packet file (cached reopen, whole view < 1 s). The generator needs a
   mode whose values every format holds exactly (e.g. multiples of 1/512, no NaN gaps for the i16 file).

Known: Linux CI's `t_widgets` failed twice on "the size is kept" and then passed (not reproduced locally, even with
4× CPU throttling). Live sources and `.tlm` headers refuse `[packets]` for now.

