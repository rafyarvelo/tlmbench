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
