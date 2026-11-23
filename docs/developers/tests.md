# The test suites

Every behaviour change comes with a test. Run the ones for the area you touched while you work, and the whole set before
you finish. Tool versions are pinned ([Building and testing](building-and-testing.md#pinned-versions)).

| What | Run with | Where |
| --- | --- | --- |
| Rust unit, property and doc tests | `just test` (`cargo nextest run --workspace`) | beside the code in each crate |
| The WASM crates' tests, natively | `just test` | `crates/web`, `crates/web-parquet` |
| Python binding tests, and the API reference check | `just test` | `python/tests` |
| The app's UI tests (Chromium, a fake core) | `just app-test [t_name …]` | `app/ui/tests` |
| The UI against the real daemon | `t_browser` and the `t_*real*` tests, in the same run | `app/ui/tests` |
| The prototype's tests | `just proto-test [t_name …]` | `prototype/tests` |
| Lint, formatting, the reference pages, licences | `just lint` | |
| Fuzzing | `just fuzz-smoke [secs]`, `just fuzz <crate> <target>` | `crates/*/fuzz` |
| Nothing written next to the data | `just ro-test`, `just ro-test-mac` | `ci/ro_check.py` |
| The ledger | `python3 ci/ledger_check.py` | `docs/release/ledger.md` |
| Benchmarks | `just bench`, `bench-record`, `bench-compare` | `crates/*/benches`, `bench/history.json` |

`just ci-linux` and `just ci-native` run everything a CI job runs.

## The app's UI tests

`app/ui/tests` has one `t_*.js` per area (about 135). Each loads the built UI (`app/ui/dist`: `just app-test` builds it first)
in headless Chromium, with `fakecore.js` standing in for the app's commands, and checks behaviour and state, not pixels.
`lib.js` is the harness: `run('name', async ({ p, check, fake, until, text }) => …, { files, launch, settings, setup })`. A test
fails when a check fails, the page throws, or the script throws; its full output goes to `out/<test>.txt`, and a failing
test's screenshot to `out/fail_<name>.png`. [`app/ui/tests/README.md`](../../app/ui/tests/README.md) lists what each one covers.

- **The fake core** answers by synthesising recordings (`fake.recording`, `fake.csv`, `fake.bin`): sines, NaNs where a test
  says, files by path. It keeps what the UI sent (`fake.opened`, `eventSaves`, `cfgCalls`, …) so a test checks the call as well
  as the result. Grep it for a field before adding one: most names exist.
- **Hooks on `window`** (`__tlmApp`, `__tlmFake`, `__tlmChecks`, `__tlmGauges`, …) give tests the state a selector can't reach.
  Top-level `const`s aren't reachable from Playwright, so add to a hook.
- **Charting libraries are stubbed** where they'd draw; check visual changes with a screenshot.
- A test that needs something not built (the CLI, the viewer, the release window) prints `SKIP` and passes;
  `STRICT_SKIP=1` turns a skip into a failure, which the release gate and CI use.
- **Load flakes.** The harness waits a fixed time for the page: under a cargo build every core is busy and tests fail at
  boot (`__tlmApp` undefined). Never run the UI suite while cargo builds, and rerun a failure alone. `t_browser`,
  `t_descriptor` and `t_windows` are known to fail under full-suite load and pass alone.
- **No `prompt`, no synchronous `confirm`.** Chromium has both; the app's webview doesn't (wry answers null, Tauri's `confirm`
  returns a promise). Use `askText` (`ask.svelte.ts`) and await confirms.
- **Parity.** `just app-parity` compares the app with the prototype scene by scene (menus, dialogs, hover states) and writes
  `out/parity/index.html`. It reports only. Add a scene when you touch a panel.

## The prototype and its tests

`prototype/plotter.html` is the single-file UI prototype, the design reference (data synthesised in the page, Share and
Export simulated). `prototype/tests` has its Playwright suite (`./run.sh`, about 4 minutes; timing tests run alone at the end).
The prototype is frozen: it isn't developed further.

## Fuzzing

`crates/decode/fuzz` has targets for the CSV, packet, Parquet and reference-format readers; `just fuzz-smoke` runs each
briefly on the nightly pinned in `ci/versions.env`. `seeds/<target>` holds committed starting inputs; corpora are not committed.

## Read-only

`ci/ro_check.py` records every path under a folder (type, size, mtime, SHA-256), runs the commands in `ro_commands` (the
justfile) and fails if anything changed or a command failed. With `--require-ro` it also fails unless the folder is really
read-only. `just ro-test` uses `prototype/tests/fold`; `just ro-test-mac` copies it into a read-only disk image; CI mounts it
read-only in the container. `ci/ro_cli.sh` runs every CLI command that reads recordings over each CSV there.

## The ledger

`docs/release/ledger.md` is the only feature status: every v1 requirement with its source, its verdict (Done, Partial,
Missing, Diverged, Unverified, Owner pass) and the evidence, usually a test name. `ci/ledger_check.py` checks its format and
that its counts line is right. A row moves to Done with evidence in the same commit as the work.

## Test data and screenshots

`tools/testdata/make.py` writes the data the owner's release test plan uses (flights as CSV, a small `.tlm`, Parquet, a
binary file with its descriptor, text modes, a growing file and a sender for live), the same every time on every OS.
`app/ui/tests/shots.js` takes screenshots from the built UI: with no flag, the README's (`docs/images`); with
`--manual`, the User Manual's (`shots-manual.js`, into `docs/images/manual`, with the list of which chapter uses which).
Always pass an output folder.
