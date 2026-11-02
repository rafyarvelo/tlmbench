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
- Column-time recordings binary-search the time column for every pixel edge; a cold file faults on each search. The
  design's chunk index (first time of every 64k samples, kept in the cache) would bound that to one page.

