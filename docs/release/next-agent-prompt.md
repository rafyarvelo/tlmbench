You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to the v1
release. R12 (the viewer opens Locked, with Unlock) and B4 (a build slow enough to look at) are done and committed; the
owner has not re-tested them. **Your job: close the gaps the last session left (below), then finish R2.2, browser-only
mode.** Nothing is pushed: pushing is the owner's call. One agent works at a time: start from a clean `main`, leave a
clean `main`. If another Claude session is running in this repository, agree with it which files each of you touches, and
when each runs cargo or a UI suite, before you edit.

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

## Where things stand (Nov 15)

- **R12 built** (commits `7a96b07`, `892b51e`; ledger D-ui-34, D-ui-35, D-brow-05, X-win-11). The viewer opens Locked; the
  pill in the top bar unlocks it; `viewLocked()` in `app/ui/src/ui.svelte.ts` is the one test, and `edit()` in
  `stack.svelte.ts` refuses a layout change while locked. `t_viewerctl` clicks every visible control and menu item,
  locked and unlocked (about 5 minutes; `run.sh` allows it 600 s). The same rule ("a control acts, is left out, or is
  disabled with a short tip saying why") now holds in browser-only mode for the rail's Live icon and Settings.
- **B4 built**: `TLMBENCH_SLOW_BUILD=<ms>` (in `crates/store/src/pyr.rs`) waits after each block of about a million
  samples. The release app was rebuilt from `892b51e` and the owner's sheet says so.
- **R2.2 so far** (D-brow-01 to D-brow-05): a CSV parses in a Worker, a reload brings files back, a static-host recipe, no
  folder or live offers in a tab. S5.10-04 is Partial: no `.tlm`, Parquet, packets, descriptors, plugins, derived channels,
  events, checks or several files in a tab. `crates/web` includes only the decoder's `csv.rs`, `backwards.rs`, `error.rs`
  and `profile.rs` by `#[path]`.
- **Open owner question** (sheet R12): keep an unlocked arrangement across reloads? The recommendation, no, is on the sheet.
  Don't build persistence unless the owner says yes.
- Not yet tested by the owner: R12, B4, S1 to S4 (the Nov 15 work), and B6, C2, C3, D1, D2, F4 to F10, G1 to G3, H1 to H5.

## 1. Gaps the last session left

- **`t_windows` is red on a clean `main`** (checked on a worktree of the commit before R12): after "Move to new window" the
  moved tab stays in the first window, and the tab list before it already shows the file twice ("flight.tlm |bench
  run.csv |bench run.csv"), as if ⌘O opened the file twice. Find out whether the page opens it twice (the keydown handler
  in `app.ts` against the command table's `lastKey` guard, decision 8) or the test's fake does. If it is a real
  double-open it is a bug the owner would meet. Fix it and say what it was.
- **The whole UI suite was not rerun after the last edits** (the add-pane row's lock, the pill's text, the run.sh limit).
  Run `npx svelte-check --fail-on-warnings && npm run build` from `app/ui`, then `tests/run.sh`, then `t_perf_gauges`
  alone. Check the build succeeded: svelte-check warnings stop it and the tests then run the old `dist`.
- **Not run in that session**: the Rust tests (`cargo nextest run --workspace`, see Rules for disk), `python3
  ci/ledger_check.py` was run and is green.
- **Dragging in the owner's browser.** Headless Chromium drags fine in the viewer; the owner's failure was not reproduced.
  If you can, try the export in WebKit (Playwright's `webkit`, if installed) and Safari, unlocked. If a drag fails there,
  it is a bug in the viewer, not the owner's setup.

## 2. Finish R2.2 (browser-only mode), as `release-plan.md` says

Parquet, `.tlm`, packets, descriptors, plugins, derived channels, events, checks, several files in a tab, and folders where
the browser allows them (the prototype opens folders: `showDirectoryPicker`, and a `webkitdirectory` input where it is
missing). Close S5.10-04. Suggested order, each one tested and its ledger row moved before the next:

1. **`.tlm`** (fixed layout, the simplest). Include the decoder's reader into `crates/web` by `#[path]` as the CSV one is;
   the WASM crate has its own workspace and target (`just viewer` builds it). Nothing may use threads, mmap or the file
   system there: the file's bytes come from the page.
2. **Parquet.** The `parquet` crate may not build for `wasm32-unknown-unknown` with the features the workspace uses
   (`snap`, `zstd`, `lz4`, `flate2-zlib-rs`, `brotli`): try pure-Rust codecs only, measure the WASM size against the
   viewer's budget, and report the numbers before you accept a bigger download.
3. **Descriptors and packets**, then **several files in a tab** (the viewer's `group_*` commands already do the
   alignment in the core), then **events**, **checks**, **derived channels** (the expression crate is already in the WASM
   core), **WASM plugins** (the browser's own WebAssembly; native plugins can't exist in a tab, so say so in the UI where
   they would be offered).
4. **Folders.** Where the browser allows it (`showDirectoryPicker`), list a folder's recordings in the Explorer, read-only,
   as the prototype does; where not, say so once and offer Open recording… A folder's `.tlmbench/` is read for shared
   configuration the same way, read-only: a tab writes nothing beside the data.

Each step: the Explorer, the start page and Settings show only what works (the rule above: act, be left out, or be disabled
with the reason). When checks and layouts exist in a tab, look again at the rail's Layouts and Rules & checks icons and close
ledger D-brow-05. A binary file whose format is not read yet still says what a tab reads. Keep the OPFS cache keyed as
`keyOf` does and wrapped in try/catch, so the page works without it.

Tests: extend `t_browseronly` (it runs against `app/browser/dist` and a real Chromium), one case per format, with a
generated file; a large file must still keep the page drawing frames (the Worker). Run the apple-design audit at the end of
R2.2 into `ui-audits.md`, browser-only light and dark, each new screen.

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
    left out or disabled with the reason; `TLMBENCH_SLOW_BUILD`.**

## Rules

- A row moves to Done only with evidence you ran in your session (a test by name, a file, or a command with its date). Run
  `python3 ci/ledger_check.py` before every commit, and update the counts line at the top of the ledger. Ledger IDs are
  unique: the next free ones are D-ui-36, D-brow-06 and X-win-12; check the highest in use before you add a row.
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
