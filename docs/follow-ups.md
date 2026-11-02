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

## Design doc updates from Phase 1

Decisions taken while building Phase 1, to carry into the living doc and `docs/design.md`:

- **Reference binary format** (`.tlm`): 64-byte-aligned header with magic `TLMBENCH`, version, header and descriptor
  lengths, then the descriptor itself as TOML, then fixed-width records. Layout in `crates/decode/src/reference.rs`.
- **Descriptor schema, Phase 1 subset**: schema version, byte order, header and record size, time from a rate, an
  integer field (ns/us/ms/s) or the index, and per field name, type, offset, unit, scale and value offset.
- **CSV cache layout**: one plain little-endian file per column (`t.i64`, `c<k>.f64`) plus `manifest.json`, chosen
  over Arrow IPC.
- **Cache key**: size + mtime + BLAKE3 of the first and last MiB and 63 sampled blocks, not a whole-file hash (which
  would cost about a second per open on multi-GB files).
- **Cache root**: `$TLMBENCH_CACHE`, else the OS cache dir's `tlmbench` folder, shared by the CLI and the app, rather
  than Tauri's per-identifier app_cache_dir.
- **M4 raw threshold**: raw samples are sent when the view holds at most 4W samples (not W), since that is no more
  points than M4 would send. Each trace includes the nearest sample beyond each edge.
- **Gaps in Phase 1**: empty or all-NaN buckets break the line; time jumps between samples in a raw view don't yet
  (the gap index arrives with the pyramid).
- **Time that goes backwards**: an error naming the row or record for now, not flagged in the index.
- **Relative times**: times before 2000-01-01 display as offsets from zero.
- **CSV ingest**: ~800 MB/s on an M1 with positioned reads per chunk; memory maps serialize parallel page faults on
  macOS, which matters for the Phase 2 pyramid build too.
- **Gate definition**: missed vsyncs instead of a 16.7 ms p95 (WebKit timer resolution), core reduce time measured
  in Rust, whole-file views reported but not gated.
- **Bundle identifier**: `dev.tlmbench.desktop` is a placeholder; the owner should choose the real one.

## Phase 1 loose ends

- Visual check of the app: screenshots from this environment can't capture other apps' windows (no Screen Recording
  permission), so the pane's look (axes, labels, dark mode) needs a look by eye.
- The CI image doesn't include the app UI's npm packages; the Linux job installs them on each run.
- Whole-file views read every raw sample: 21 ms warm, 657 ms cold for 100M. The Phase 2 pyramid fixes both.
- M4's inner loop isn't vectorized (about 600M samples/s per core); worth doing alongside the pyramid.

