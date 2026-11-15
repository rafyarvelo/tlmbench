You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to the v1
release. R12 (the viewer opens Locked, with Unlock), B4 (a build slow enough to look at) and the first half of R2.2
(`.tlm` and Parquet in a tab) are done and committed; the owner has not re-tested them. **Your job: finish R2.2,
browser-only mode: packets, descriptors, several files in a tab (with alignment), events, checks, derived channels, WASM
plugins and folders (the list in section 2).** Nothing is pushed: pushing is the owner's call. One agent works at a time:
start from a clean `main`, leave a clean `main`. If another Claude session is running in this repository, agree with it
which files each of you touches, and when each runs cargo or a UI suite, before you edit.

## First, in this order

1. Read `CLAUDE.md`. Its hard requirements bind you: the app works from its cache alone on a read-only folder; nothing is
   written next to data except by Share and exports; exports are static files; everything in the design is v1 and you never
   propose cutting scope; the app never runs Python or other programs.
2. Read `docs/release/owner-test-sheet.md` (open items only, in the owner's words), `docs/release/release-plan.md`,
   `docs/release/ledger.md` and `docs/release/ui-audits.md`. A feature is done only when its ledger row is Done with evidence
   `ci/ledger_check.py` can check.
3. Skim `docs/browser-only.md`, `docs/exports.md`, `docs/follow-ups.md` and the memory files in
   `/Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/` (`r12-viewer-lock.md`, `owner-decisions-nov14.md`,
   `viewer-architecture.md`, `disk-space.md`, `headless-tests-miss-real-window.md`, `release-plan.md`).

## Where things stand (Nov 15, later)

- **R12 and B4 built earlier** (ledger D-ui-34, D-ui-35, D-brow-05, X-win-11): the viewer opens Locked (`viewLocked()` in
  `app/ui/src/ui.svelte.ts`, `edit()` in `stack.svelte.ts`); `TLMBENCH_SLOW_BUILD=<ms>` in `crates/store/src/pyr.rs`.
- **The gaps the last session left are closed.** `t_windows` was flaky (about 1 run in 3), and it was two things: the test
  set the next pick before the first dialog was asked (the file is open from launch, so its `until` passed at once), and
  `openPath` awaited between its "already open" check and creating the tab, so two opens of one file at once made two tabs
  (a double click, two drops). In-flight opens by file and part now join (`opening` in `app.ts`); `t_fileopen` covers it and
  `t_windows` ran 8 of 8. The whole UI suite passed (104 of 104, `t_perf_gauges` alone too) and `cargo nextest run
  --workspace` passed (283, 1 skipped), `cargo clippy --workspace --all-targets -- -D warnings` is clean, `cargo fmt --all`
  was run (it had drifted in four files). **Not done:** trying the viewer's drag in WebKit or Safari (Playwright's `webkit`
  isn't installed here); the owner's drag failure still isn't reproduced.
- **R2.2 so far** (D-brow-01 to D-brow-07):
  - CSV in a Worker, a reload brings files back, a static-host recipe, no folder or live offers in a tab.
  - **`.tlm`** (D-brow-06): `crates/web/src/import.rs` `import_tlm`, over the decoder's `descriptor.rs`, `reference.rs` and
    `strided.rs` included by `#[path]` (rayon, threads and mmap never reach the WASM). `import_file` picks CSV or `.tlm` by the
    bytes.
  - **Parquet** (D-brow-07): the pure part of the decoder's reader is now `crates/decode/src/parquet_core.rs` (the time
    column, units, nulls, widening), used by the app's `parquet.rs` and by the new crate **`crates/web-parquet`** (own
    workspace and target, like `crates/web`). Its WASM (3.96 MB, 1.08 MB gzipped) is fetched by the Worker only when a
    Parquet file opens (`app/ui/src/parquet-wasm.ts`, `browser-worker.ts`); the main core stays 1.33 MB (it was 0.97 MB
    before `.tlm`: `toml` and the descriptor code). Pure-Rust codecs; **Zstandard goes through `crates/web-parquet/zstd-shim`,
    a `[patch.crates-io]` stand-in for the `zstd` crate over `ruzstd` (decode only)**, because the real crate needs a C
    compiler that targets WebAssembly (Apple's clang has none). Fixtures for every codec: `tools/testdata/parquet_codecs.py`
    writes `crates/web-parquet/tests/fixtures/`; `tlmbench-web-parquet::codecs` and `t_browseronly` read them. The same
    dump form (`crates/web/src/import.rs` `dump`/`load`) is what the Worker returns and what OPFS keeps, so a Parquet file
    reloads from the cache like a CSV.
  - S5.10-04 stays Partial: no packets, descriptors from files, plugins, derived channels, events, checks or several files in
    a tab; folders not started.
- **What I found for the next steps** (not built):
  - **Several files in a tab:** `addToTab` in a tab fails with "group_set is not available here": the browser-only command
    table (`app/ui/src/browser-only.ts`) doesn't wire `group_set`, `group_state` (a stub returns nothing) and `group_map`
    to the core, as `viewer.ts` does at lines 236 to 238. `align_estimate` is not in the WASM core at all: it is
    `tlmbench_store::align::estimate` (the app's `main.rs:663`), and the clock, starts and manual methods need no data
    while sync, events, cross-correlation, time source and plugins do. Decide with the owner whether to build the needed
    part of `tlmbench-store` for WASM (release plan: "the owner decides it once, with a recommendation") or do the simple
    methods in `crates/web` and disable the estimators in a tab with the reason (the rule: act, be left out, or be
    disabled with a tip). My recommendation: the simple methods now, the estimators when the store builds for WASM.
  - **Packets and descriptors:** `packets.rs` mixes framing (`frame`, `frame_checked`, `packet_records`, `decompress`:
    pure) with the index cache, mmap and rayon. Split the pure part into a `packets_core.rs` the way `parquet_core.rs` was
    split. It calls `zstd::stream::read::Decoder`, which the shim doesn't have yet (add it over ruzstd's
    `StreamingDecoder`) and `lz4_flex`, `flate2` (zlib-rs) and `crc32fast`, all pure Rust. A descriptor in a tab needs
    somewhere to live (localStorage or OPFS, by file name pattern: `descriptor::name_matches`) and the descriptor form's
    commands (`descriptor_*`, `preview.rs`, `needs_descriptor`, `packet_parts`; see the stubs in `wasmcore.ts`).
  - **Disk:** a full `cargo nextest run --workspace` fills this Mac's disk even from 21 GB free (debug test binaries were
    19 GB). This worked in 8 GB: `rm -rf target/debug`, then
    `CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo nextest run --workspace --no-fail-fast`; delete `target/debug` after.
- **Open owner question** (sheet R12): keep an unlocked arrangement across reloads? The recommendation, no, is on the sheet.
  Don't build persistence unless the owner says yes.
- **Not yet tested by the owner:** R12, B4, S1 to S4 (S4 now covers `.tlm` and Parquet), and B6, C2, C3, D1, D2, F4 to F10, G1
  to G3, H1 to H5.

## 1. What is left of the gaps

Only the WebKit or Safari drag check above, if you can do it (Playwright's `webkit` browser, if installed; else say so).

## 2. Finish R2.2 (browser-only mode), as `release-plan.md` says

Done: `.tlm` and Parquet. Left, each one tested and its ledger row moved before the next:

1. **Descriptors and packets** (see "What I found"), with the descriptor stored in the browser and found by file name.
2. **Several files in a tab** (wire the group commands; alignment as decided), then **events**, **checks**, **derived
   channels** (the expression crate is already in the WASM core), **WASM plugins** (the browser's own WebAssembly; native
   plugins can't exist in a tab, so say so in the UI where they would be offered).
3. **Folders.** Where the browser allows it (`showDirectoryPicker`), list a folder's recordings in the Explorer, read-only,
   as the prototype does; where not, say so once and offer Open recording… A folder's `.tlmbench/` is read for shared
   configuration the same way, read-only: a tab writes nothing beside the data.

Each step: the Explorer, the start page and Settings show only what works (the rule above: act, be left out, or be disabled
with the reason). When checks and layouts exist in a tab, look again at the rail's Layouts and Rules & checks icons and close
ledger D-brow-05. A binary file whose format is not read yet still says what a tab reads. Keep the OPFS cache keyed as
`keyOf` does and wrapped in try/catch, so the page works without it.

Tests: extend `t_browseronly` (it runs against `app/browser/dist` and a real Chromium), one case per format, with a
generated file; a large file must still keep the page drawing frames (the Worker). Run the apple-design audit at the end of
R2.2 into `ui-audits.md`, browser-only light and dark, each new screen (none was added yet: the work so far changed what
opens, not what is shown).

## Hand the owner, when you report

- **A rebuilt release app** if the app's Rust or UI changed (`just viewer`, then `cd app/src-tauri && cargo tauri build
  --no-bundle`; about 5 to 12 minutes). Put its commit in the sheet's "App: release build from commit" line.
- **A re-test list**: new S items for what R2.2 adds, written like section S (what changed, Do, Expect, `Result:`,
  `Notes:`). Keep the sheet to open items only. When the owner confirms an item, remove it and the original items it covers.
- **Design-doc notes owed** (list them; don't edit the Claude Doc):
  - Earlier: BLAKE3 checksums (D-cli-08); checks as their own pass (S5.3-04); the check TOML (D-check-08); CSV `#` comments
    (D-ingest-08); redacted checks leave the export (D-cli-10); group routing in `crates/web`.
  - Nov 14: ⌘Z is undo (Backspace goes back a view); the copy key (Option or ⌘ on the Mac, Ctrl elsewhere); the default
    line width and style; per-pane Events (E is the default); the top bar's Export menu; pane actions on hover, giving way by
    width; Chart and values' grip at the pane's bottom; "Show only these" as an Explorer header icon; the folder watcher with
    Refresh; the export closes on start with progress under the top bar.
  - Nov 15: export tiers (Folder or One file, raw samples when they fit, the size limit field, `--single-limit`); the viewer
    draws the app's min/max band; the native menu bar from one command table; browser-only mode so far; one reset (Reset
    app… with Things the app remembers; Reset to defaults is gone); **the viewer opens Locked with Unlock, so decision 6's
    "Read-only view" header becomes the Locked/Unlocked pill (provenance in its tooltip); what a viewer or a tab can't do is
    left out or disabled with the reason; `TLMBENCH_SLOW_BUILD`.** Added later on Nov 15: browser-only mode reads `.tlm` and
    Parquet (the Parquet reader is a separate WASM module fetched only when a Parquet file opens, 1.08 MB gzipped; the
    decoder's Parquet rules are `parquet_core.rs`, shared by the app and the tab); opening one file twice at once is one
    tab (an `openPath` race).

## Rules

- A row moves to Done only with evidence you ran in your session (a test by name, a file, or a command with its date). Run
  `python3 ci/ledger_check.py` before every commit, and update the counts line at the top of the ledger. Ledger IDs are
  unique: the next free ones are D-ui-36, D-brow-08 and X-win-12; check the highest in use before you add a row.
- A test for every behaviour change, the tests README's table, and a design-doc note when behaviour differs from the doc.
- Headless tests don't see the real window. Anything only a real window shows is an Owner pass row with what to look at.
  Try the release app yourself before calling a step done (`screencapture -x` of the running window works for a look).
- The Mac has 8 GB of memory and a small disk (about 13 GB free at the last check; `target` is 17 GB). Never run two cargo
  builds or two UI suites at once. Prefer workspace-wide cargo. A full `cargo nextest run --workspace` debug build needs about
  20 GB, so check `df -h /` first and `rm -rf target/debug` when short (the owner has OK'd deleting caches and builds).
  Run `cargo clean --manifest-path python/Cargo.toml` after a Python build.
- Build order: `just viewer` before the CLI or the app, or they embed a stale viewer. `cargo tauri build` runs
  `npm run build` and embeds `app/ui/dist` while it compiles the app crate: don't edit `app/ui/src` or rebuild the UI during
  that step. A release CLI build after a store change took 9 minutes: start it in the background.
- UI tests: `node tests/t_x.js` for one test or `tests/run.sh` for all, from `app/ui`. `t_viewer` and `t_viewerctl` need
  `just viewer` and the release CLI; the real-core tests (`t_exportreal`, `t_sharero`, `t_watchreal`, `t_resetreal`) drive
  `target/release/tlmbench-app --daemon` and skip without it. `STRICT_SKIP=1 ./run.sh` turns a skipped test into a failure.
- Commands that run longer than the tool's two-minute limit go to the background; wait on a file or a process, not a sleep.
- If you need the owner's settings, work on a copy of
  `~/Library/Application Support/io.github.rafyarvelo.tlmbench/settings.json`; never change the original.
- Shell: write scripts with the Write tool and run them; macOS `sed -i ''`; never `git stash`; add files by name; don't push
  or rewrite history unless the owner asks. End commits with the attribution line your session's instructions give.
- Concise replies to the owner: lead with what changed and what's left, no recaps. Never propose cutting scope.
