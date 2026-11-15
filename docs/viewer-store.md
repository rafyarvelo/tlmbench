# The viewer's sparse store: what an export holds and what a view reads

How the HTML export's viewer holds data (built in release step R2.1, Nov 11, 2026; written as its plan, kept as the reference). It covers Light's raw windows and a trimmed export, which both
need a viewer core that holds a channel partly: raw samples for some stretches only, and a pyramid whose finest level
may be above level 4.

## The integrity rule (what the tests hold)

Every value the viewer shows is **a raw sample, or an exact aggregate** (a summary's min, max, first or last, at its
true time, or stats merged from whole summaries and raw edges). A view that lacks the raw samples it would need says
so (`coarse: true`, which the UI labels "finer detail not in this export"); it never draws a value that was
interpolated, snapped or invented, and a sample that is not in the export is never read as a number (a missing
sample in the store reads as absent, not as 0 or as NaN data).

## What an export holds (writer, `crates/export/src/html.rs`)

- **Pyramid:** per channel, levels `first_level..=top` end to end (`first_level` is 4 unless trimmed). Unchanged.
- **Raw:** per channel a list of **windows**: sample ranges `[a, b)` held in files `c<n>.<j>.bin`, f64 LE.
  Full is one window `[0, n)` split into chunks of `CHUNK` samples; Light's windows are the stretches around events
  and check violations, merged where they overlap or touch, and cut at chunk size. The manifest lists
  `raw_chunks: [[first sample, samples], ...]`, one entry per file `c<n>.<j>.bin` in file order; a channel with none has `raw: false`.
- **Checksums:** BLAKE3 of every file, as now (decision 2).

## What the viewer core holds (`crates/web/src/data.rs`)

`Col.raw` becomes a `Raw`: a sorted list of non-overlapping spans `(start, Vec<f64>)`. It answers
`covers(a, b)` (every sample of `a..b` is held), `get(i)` (`None` outside a span) and is the `Samples` the core's
queries read (`NaN` outside, only reached when a caller skipped `covers`; no path shows it). `put_raw` takes a span
start and appends/merges. The store (`tlmbench-store`, no `native`) is used for what is shared with the app: the
core's `Recording`/`Opened` queries over a file, groups with time maps (several files in one export, `accel_x·2`),
checks and widgets.

## Which data a view reads (the plan)

For channel `c` and a viewport over samples `lo..hi` (edges widened by one sample):

1. **Raw covers the view** and the view is exact-worthy (not `coarse_first` with summaries paying): the core's raw
   view (`m4_breaking`) over the held samples. Reads only `lo..hi`. Not coarse.
2. **Raw does not cover it, a pyramid exists:** the *summary view*: the finest stored level `L >= first_level` for
   which a summary is at most about a pixel wide (else the coarsest), each non-empty summary in the view
   contributing its min and max at their true times in order, a gap line where times jump over four median
   intervals. (A view that is only partly inside a window is drawn from the summaries
   throughout; zooming into the window makes it exact.) Always `coarse: true`.
3. **Neither:** an empty trace, `coarse: false` (nothing to say it is coarse about) and the page shows the empty
   state "not in this export".
4. **`values_at`** answers only from raw (the nearest sample, which must be held); none otherwise. Never from a summary.
5. **`stats`** is exact only when the span's two ragged edges are covered by raw (the middle comes from whole
   summaries); otherwise the channel is left out of the reply, as now.
6. **`rows`** (the table) reads held samples; a sample not held is a blank cell, not NaN data.

The core's `Pyramid` keeps its rule that level 4 is first; a trimmed channel never goes through it. Only the viewer's
summary view (2) reads a pyramid that starts higher, so core is unchanged.

## Which files the page fetches (`app/ui/src/viewer.ts`)

1. `export.json`, the WASM core, then every file's times and pyramids (small: the view draws from them at once).
2. Raw windows in the background, **nearest to the view first** (the page orders its queue by distance from the last
   view's time span, and again when the view moves), each verified against `export.json`'s BLAKE3 before it is put in
   the core; a mismatch shows an error in place of data.
3. A read that needs raw (values, stats, table) waits for the windows covering it, not for the whole channel.

## As built (Nov 11)

`Raw` is in `crates/web/src/data.rs`; the summary view and `exact_summary` (stats from whole summaries and raw edges,
for any first level) are in `crates/web/src/ops.rs`; the writer's `Window` (centred on the event, `--detail`) is in
`crates/export/src/html.rs`. The viewer core uses `tlmbench-store`'s no-`native` build only for what the app shares
(`Opened::in_memory`, checks). **Owner decision, Nov 11:** several files are routed in `crates/web/src/group.rs` on core's `TimeMap` over the same sparse-aware per-file queries, because the store's `Opened` queries would read an absent sample as missing instead of as "not in this export"; making them sparse-aware is a rewrite of the store's query layer.

## Tests

- Rust (`crates/web`): `Raw` span semantics (merge, gaps, covers); every shown view point is a raw sample or a
  summary extreme at its time (property check over random windows and trims); `values_at` never answers from a
  summary; stats exact or absent.
- Export (`crates/export`): Light windows around events (30 s or 3x the duration, capped at 10 min, merged), the
  manifest, trimmed levels.
- `t_viewer` (page): a Light export draws, labels "finer detail not in this export" outside its windows, is exact
  inside; a corrupted chunk shows an error.
