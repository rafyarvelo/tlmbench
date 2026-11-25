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

## What stays uncovered, and why

Only these may stay unreached; each is a range with its reason. Anything not listed is a gap to close.

| File | Lines | Why |
| --- | --- | --- |
| `crates/decode/src/cache.rs` | `cache_root`'s `TLMBENCH_CACHE` branch | Reading an environment variable: a test would have to set it for the whole process, which races with the other tests of the same binary under `cargo test`. The CLI's and the gates' runs use it. |
| `app/ui/src/gate.ts` | all | The release gates' own driver: it runs only in the release window (`just gate`, `gate2`, `gate3 app`, `gate4 app`), whose results `docs/developers/gates.md` records. |

## Left to do in `decode` (R6c.3's first crate)

Done Nov 24: the descriptor's refusals; CSV's quoted reader, label limits, empty files and profiles (which found the
`stamp [ms]` bug, `868883c`); every field type in fixed-width and packet files; packet framing and streams.
Done Nov 25 (`00a1f9d`): `payload.rs` (the field wrapper's every call, checked against a column that differs; a
plugin's errors; damaged packets; a batch with no rows), the cache's chunk boundaries (one 17 MB file: a 200 KB row on
the first boundary, a quote after the schema's sample), a plugin column of every type, every Parquet time column and
value type with and without nulls, a decoder plugin's faults, a reference file's header. Left, each a line or a few
(list them with the recipe under "Measuring": a segment with a zero count that has `hasCount` and isn't a gap):

- `parquet_pages.rs` (73%): the page checks of `check_chunk` for compressed, dictionary and v2 pages and damaged
  headers; needs files written with those options (the `parquet` crate's `WriterProperties`) and files cut or flipped
  on purpose. The WASM crate's tests (`crates/web-parquet`) are not counted: decide to measure them or leave them.
- `cache.rs`, `csv.rs`, `descriptor.rs`, `packets.rs`, `packets_core.rs`, `fixed.rs`, `lib.rs`, `preview.rs`,
  `strided.rs`, `bytes.rs`, `profile.rs`: mostly error branches (an unreadable file, a failed write) and one-line
  guards; each needs a read of the line to say test or reason. `bytes.rs` 35, 48, 56, 70-71, 77 are Windows-only or
  OS-failure paths: reasons to write.
