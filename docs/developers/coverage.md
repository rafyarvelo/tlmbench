# Test coverage

How much of the code the tests run, how to measure it, and what is left uncovered on purpose. The goal (owner, Nov 24)
is as close to 100% as can honestly be reached, Rust and the UI; the work is step R6c of `docs/release/release-plan.md`,
whose status table says where it stands. Coverage is a report, not a target: a test exists to check behaviour, and a
line it reaches without checking anything doesn't count.

## Measuring

**Rust** (about 30 minutes; `target/llvm-cov-target` grows to 17 GB, so check `df -h /` first and delete it after):

```
cargo binstall cargo-llvm-cov            # once
rustup component add llvm-tools-preview  # once
CARGO_INCREMENTAL=0 cargo llvm-cov nextest --workspace --no-report
cargo llvm-cov report --summary-only     # per file
cargo llvm-cov report --json > cov.json  # per line: a segment with a zero count is code no test ran
rm -rf target/llvm-cov-target
```

On the 8 GB Mac add `CARGO_BUILD_JOBS=2` (four rustc at 2.8 GB each filled the swap) and never start a second cargo
command while one runs. To list the uncovered lines of a crate from `cov.json`: for each file's `segments` entry
`[line, col, count, hasCount, isRegionEntry, isGap]`, the lines from a segment with `count == 0`, `hasCount` and not
`isGap` to the next segment's line.

**The UI** (about 25 minutes; never while cargo builds, or the harness fails at boot):

```
cd app/ui && npx vite build --sourcemap inline
cd tests && TLM_JS_COVERAGE=/tmp/jscov/raw ./run.sh
cd .. && npm run build                   # the normal build again
```

Then, in a scratch folder (nothing is added to the repository's dependencies): `npm install monocart-coverage-reports
@bcoe/v8-coverage`; merge the tests' files per script with `mergeProcessCovs` (keyed by the script's path, since each
test serves the page from its own port), keep each script's `source` once, and give the merged list to
`monocart-coverage-reports` with `reports: ['json-summary', 'json']`, the entries filtered to `/assets/*.js` and the
sources to `src/`. Fed the 120 raw files directly it runs out of memory even with a 5 GB heap.

The tests that start their own browser (the browser-only, viewer and real-core tests) don't go through
`tests/lib.js` yet, so the files only they run (`browser*.ts`, `wasmcore.ts`, `wasmcap.ts`, `parquet-wasm.ts`,
`viewer*.ts`) have no figure (R6c.4).

## Where it stands

| Measured | Rust lines | UI lines (branches) |
| --- | --- | --- |
| Nov 24, `ff087fc` | 76.5% | |
| Nov 24, `b6580c6` (R6c.1's five tests) | 77.5% | |
| Nov 24, `441c477` | | 91.7% (75.9%) |
| Nov 25, `f9ffec7` (R6c.3, decode's first pass) | 78.3% (decode 93.7%: csv 96.5%, packets_core 96.6%, fixed 97.1%, descriptor 95.5%, packets 94.9%, cache 95.2%) | |
| Nov 25, `00a1f9d` (R6c.3, decode's second pass) | 79.0% (decode 96.3%: payload 99.0%, plugin 100%, reference 100%, parquet_core 98.6%, parquet 97.4%, cache 97.4%, fixed 98.9%; parquet_pages 73.1%; live 93.8%, store 89.4%, core 94.3%, expr 95.3%, export 88.5%, plugin 81.4%, cli 79.8%, app 27.6%, tools 0%) | |
| Nov 25, third pass (R6c.3, decode; Sonnet) | 79.5% (decode 97.1%: parquet_pages 99.2%, lib 100%, reference 100%, plugin 100%, backwards 100%, payload 99.0%, fixed 98.8%, profile 98.6%, parquet_core 98.6%, parquet 98.1%; live 93.8%, store 89.4%, core 94.3%, expr 95.3%, export 88.5%, plugin 81.4%, cli 79.9%, app 27.6%, tools 0%) | |
| Nov 26, `93fa262` (R6c.3, live and store; Sonnet) | 81.5% (live 99.3%, store 93.9%: adjust, joined, events, widgets, xy, config_core, error, chunks, sys 100%; group 97.5%, lib 96.1%, live 95.8%, trigger 96.9%, derived 91.8%, pyr 91.8%, recipes 92.9%, checks 92.0%, align 93.6%, config 87.8%; decode 97.2%) | |

## What is measured

The figure is the Rust workspace's own test run (`cargo llvm-cov nextest --workspace`). **`crates/web-parquet`'s tests
are not counted** (owner, Nov 25): that crate is the WASM build that includes `decode`'s `parquet_core.rs` as source,
and its tests run in a browser engine, not in this run. What `decode`'s Parquet code does is covered by native tests in
`decode` itself: `parquet.rs`'s (every time column and value type, with and without nulls) and `parquet_pages.rs`'s
(files written with the `parquet` crate's `WriterProperties`: every compression, dictionary on and off, v1 and v2 data
pages, several pages to a chunk; page headers written by hand; files cut or bit-flipped on purpose).

## What stays uncovered, and why

Only these may stay unreached; each is a range with its reason. Anything not listed is a gap to close.

| File | Lines | Why |
| --- | --- | --- |
| `crates/decode/src/cache.rs` | `cache_root`'s `TLMBENCH_CACHE` branch | Reading an environment variable: a test would have to set it for the whole process, which races with the other tests of the same binary under `cargo test`. The CLI's and the gates' runs use it. |
| `app/ui/src/gate.ts` | all | The release gates' own driver: it runs only in the release window (`just gate`, `gate2`, `gate3 app`, `gate4 app`), whose results `docs/developers/gates.md` records. |
| `crates/live/src/lib.rs` | 284, 292, 340, 395 | A thread that can't be started (`thread::Builder::spawn(..)?`): the OS refusing a thread. |
| `crates/live/src/lib.rs` | 355, 387 | `Demux::new`'s sync-word error (validation before it refuses the same word) and the `File`/`Replay` arm of the packet match (`start_packets` refuses those first); both guard an invariant. |
| `crates/live/src/lib.rs` | 384, 670 | A packet stream or a source on an open serial port: needs a pty, which the Linux CI test `a_serial_port_is_recorded` makes; there is none on the Mac or Windows. |
| `crates/live/src/lib.rs` | 464-466 | A source thread that panicked (`join` fails): nothing in the ingest or writer panics on any input; guards an invariant. |
| `crates/live/src/lib.rs` | 540, 607-608, 692, 700, 716 | An address that resolves to nothing, `set_read_timeout`/`set_nodelay` failing, a UDP socket that can't be made or read: OS failures on a working machine. |
| `crates/live/src/lib.rs` | 709 | `ErrorKind::Interrupted` on a UDP read (EINTR): a signal during `recv`. |
| `crates/live/src/lib.rs` | 759, 769, 822-823 | `sync_data`, the header write and the record writes failing: disk full or a broken disk. |
| `crates/live/src/lib.rs` | 1002-1003 | `$TLMBENCH_RECORDINGS` set: the test checks whichever branch the environment gives (setting a variable races with the other tests of the process, as in `cache.rs`). |
| `crates/live/src/lib.rs` | 1039, 1042, 1045, 1069, 1091, 1111 | The session folder, its `.held` lock and `session.json` failing to be made or written: OS errors. |
| `crates/live/src/lib.rs` | 1261, 1315 | A test's own timeout assertion (runs only when a source stalls). |
| `crates/live/src/tail.rs` | 37, 85, 116 | `metadata()`, `identity()` and `seek()` on a file just opened or read failing: OS errors. 173: the `unreachable!` of `Parser::Unknown` after it was replaced. 637, 652, 736, 778, 800: tests' own timeout assertions and the root-user early return of the permission test. |
| `crates/live/src/packets.rs` | 184, 189 | A framed packet whose id is in no plan (the plans and the framing's known ids come from the same descriptor), and a stream with no rings at all; both guard an invariant. |
| `crates/live/src/replay.rs` | 163 | A test's `panic!` for the wrong endpoint kind. |

## Left to do in `decode` (R6c.3's first crate)

Done: the descriptor's refusals; CSV's quoted reader, label limits, empty files and profiles (which found the `stamp [ms]`
bug, `868883c`); every field type in fixed-width and packet files; packet framing and streams; `payload.rs`; the cache's
chunk boundaries; a plugin column of every type; every Parquet time column and value type; a decoder plugin's faults; a
reference file's header; `parquet_pages.rs` (Nov 25, third pass); a file that can't be opened, by each way of opening
(`tests/unopenable.rs`). Nov 25: each earlier test of a tricky path was broken on purpose (Date64's factor, a Boolean or
half-float time, a null written as 0, a plugin's duplicate-name, column-count, column-size, time-count and `next()`
checks, a reference header's alignment, length and UTF-8 checks, the field wrapper's masked view) and every break made a
test fail.

Left (decode is 97.1%, 171 lines; list them with the recipe under "Measuring"):

- **Not producible in a test, by design** (a reason each; they are not yet in the table above because the owner's
  rule is a written reason per range, and these are one reason for many lines): the `?` that returns an OS error from a
  file just opened, created, written, flushed, renamed or removed (disk full, a file deleted while it is read): `cache.rs`
  (`fingerprint`/`Source` reads 52-128, the cache writers 248-340 and 388-456, `bytes::map` 306-360, 525-531),
  `plugin.rs` 98-192, `parquet.rs` 30-128, `payload.rs` 93-287, `packets.rs` 117-153 and 738-751, `profile.rs`
  237-254, `descriptor.rs` 457, 861-902, `fixed.rs` 58-65 and 483-486, `preview.rs`' unreadable-file arms, `csv.rs`
  1059-1067. `unreachable!` arms of matches the validation before them excludes: `parquet_core.rs` 174 and 225,
  `packets_core.rs` 43, `strided.rs` 141. Windows-only or OS-failure paths in `bytes.rs` (35, 48, 56, 70-71, 77). Failure
  branches of the tests themselves (a `wrong.push` that runs only when a case fails: `parquet.rs` 310-312, 405-407,
  424-426; `parquet_pages.rs` 302-304, 416, 645). `parquet_pages.rs` 172: the depth check after a field in `fields` can't
  be reached, because `skip` refuses that depth first (a candidate for removal).
- **Behaviours still to test** (each a few lines; read the line first): `csv.rs` time-unit spellings (`us`, `µs`, `ns`, `s`,
  `sec`, `secs`, `seconds`) and an ISO time column read by a profile or an override (471, 504, 521, 552-567, 1000,
  1019-1023); `descriptor.rs` refusals (618 no fields, 680 a float sequence, 692 a time field with no channels, 263 a
  sync word that is not hex, 335 a seconds time unit, 752 and 824-837 discovery's skipped files); `strided.rs` 25, 57-59
  and 133-137 (time fields of 8, 16 and 32 bits and plain bytes); `preview.rs` 60-77 (every type, an offset past the
  record), 102-113, 139-161 (a descriptor with packet types, a read error); `packets_core.rs` 75-92, 174, 197, 217,
  279-301, 332-335, 364-369 (a zstd stream that fails, a header that hasn't all arrived, a packet type's known sizes);
  `packets.rs` 138, 183, 216, 279-302, 324-343, 384, 417, 473-478 (a stale index, damaged packets); `fixed.rs` 185,
  348-349, 382, 394, 429-436, 533, 895; `profile.rs` 82, 209, 269 (the profile matcher's thresholds); `payload.rs`
  189-193; `cache.rs` 186-188 (`f64_type`), 299 (resampling a file with no time), 774.

## Left to do in `store` (R6c.3, Nov 26)

Done Nov 26 (`crates/store/tests/coverage.rs`, one file per behaviour group): events (labels, missing values, the cap,
damaged and unreadable files); a missing-when that is true everywhere, false everywhere or wrong; the XY query's edges;
a widgets frame on a growing file and on a channel with too many runs; a recording and pyramid held in memory; a joined
recording read through every `Recording` method against its parts; triggers (Above, Below, the level itself, the quiet
window, disarming, NaN, a constant); the recipe library (names, renames, deletes, problems, import, `check`); the
shared-config reader's every refusal; damaged cache entries and a build that follows another process; groups (routing,
map changes, build). Mutation checks: a break of joined routing, trigger `Above`'s comparison, the event cap, the widget
look-back, the XY mask and the gaps-file size check each made a test fail (the first trigger tests missed `Above`'s
`<=`, so `a_value_on_the_level_is_on_neither_side_of_it` was added).

Left (store is 93.9%, 297 lines, measured on `93fa262`; list them with the recipe under "Measuring"):

- **Not producible in a test, by design:** the `?` of an OS error on a cache or events file just made or written
  (`lib.rs` 208-209, 252-253, 327, 341, 375, 993-996, 1018-1019, 1059-1061; `pyr.rs` 108, 235, 260, 270, 446, 493-513, 519,
  579-587, 601-604; `events.rs` 106, 109, 127, 135; `recipes.rs` 88-100, 118-131; `derived.rs` 274-290, 317-324, 331-394), `read_at`'s unix wrapper being unreached on Windows
  (`pyr.rs` 388-390), `trigger.rs` 184-187 (an expression that parses and resolves but fails while running).
- **Behaviours still to test** (each a few lines; read the line first): `config.rs` (87.8%: the shared folder's writers,
  `share`, merging and the overlay, 65-66 to 1093); `checks.rs` (92.0%: 124-126, 293-345, 364-369, 664-713, 741-800, 957-1184);
  `align.rs` (123-401: the alignment methods' refusals); `derived.rs` (a recording with no time column, `RecEnv::limits`,
  the empty range, recipes over more than a million samples that fail midway: 60, 117, 146-148, 161, 181, 189-191, 331-377,
  394, 417, 437-453); `pyr.rs` (`warm` 194-210, a resumed build of a time column 526, 534-535, `from_memory`'s growing
  path 145-145, a damaged level 220-324); `lib.rs` (the planned `warm` and prefetch paths 722-792, `filter_mask` 906,
  `with_pyramid` of a growing file); `live.rs` (68-426, not yet read line by line: the growing pyramid's `?` branches, `LiveDerived`'s
  skipped recipes and error paths; some are OS errors, the rest are behaviours); `group.rs` 87, 159, 173, 186, 216, 266, 366, 397, 416 (a file with no times, a view
  built part-way, a held column with no index).
