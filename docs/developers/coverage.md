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

## What stays uncovered, and why

Only these may stay unreached; each is a range with its reason. Anything not listed is a gap to close.

| File | Lines | Why |
| --- | --- | --- |
| `crates/decode/src/cache.rs` | `cache_root`'s `TLMBENCH_CACHE` branch | Reading an environment variable: a test would have to set it for the whole process, which races with the other tests of the same binary under `cargo test`. The CLI's and the gates' runs use it. |
| `app/ui/src/gate.ts` | all | The release gates' own driver: it runs only in the release window (`just gate`, `gate2`, `gate3 app`, `gate4 app`), whose results `docs/developers/gates.md` records. |

## Left to do in `decode` (R6c.3's first crate)

Done Nov 24: the descriptor's refusals; CSV's quoted reader, label limits, empty files and profiles (which found the
`stamp [ms]` bug, `868883c`); every field type in fixed-width and packet files; packet framing and streams. Left:

- `payload.rs`: the field wrapper's remaining delegations (`view_masked`, `stats_masked`, `view_coarse` through a
  plugin field) and the payload decode's failure paths (a plugin that returns too few rows, an error).
- `parquet_core.rs`, `parquet_pages.rs`: run by the WASM crate's tests (`crates/web-parquet`), not the workspace's;
  either count those (`cargo llvm-cov` in that crate too) or add native tests of the paths only they reach.
- `cache.rs`: a row longer than the splitter's window at a chunk boundary, the first 16 MB cut at a row end, quotes
  first met in a later chunk during the build (all need files of 8–16 MB: one test can build one), and the cached
  columns of every type (a decoder plugin that returns each type).
- `preview.rs`, `plugin.rs`, `strided.rs`, `bytes.rs`, `reference.rs`, `profile.rs`, `lib.rs`: a handful of lines each.
