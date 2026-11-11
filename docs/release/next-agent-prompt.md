You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to the v1
release. Step R2.1 (exports) is done and committed. **The owner is now testing the release app and will give you a list of
actions. Your first job is to ask for that list and wait for it; do not start R2.2 or any other step before you have it.**

One agent works at a time; you start from a clean `main` and leave a clean `main`. Nothing is pushed: pushing is the owner's
call (CI on `d45727c` was green once; R0 needs two green runs in a row with no rerun).

## First, in this order

1. Read `CLAUDE.md` (its hard requirements bind you: the app works from its cache alone on a read-only folder; nothing is
   written next to data except by Share and exports; exports are static files; everything in the design is v1 and you never
   propose cutting scope; the app never runs Python or other programs).
2. Read `docs/release/release-plan.md` (steps R0 to R9, the ten release decisions), `docs/release/ledger.md` (one row per
   requirement; a feature is done only when its row is Done with evidence `ci/ledger_check.py` can check) and `docs/release/review.md`.
3. Read what R2.1 left: `docs/release/r2-1-sparse-store.md` (the viewer's sparse raw store and which data a view reads),
   `docs/exports.md`, `docs/follow-ups.md` ("Left from step 5.10" and "Suggested by the owner's real-window pass"), and the
   memory files in `/Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/` (`viewer-architecture.md`,
   `disk-space.md`, `headless-tests-miss-real-window.md`, `roadmap-v1.md`, `release-plan.md`).
4. `git log --oneline -12` and `git status`: confirm `main` is clean and the last commit is `0be7b83` (or later).
5. **Then ask the owner for their list of actions.** The owner may hand you `docs/release/owner-test-sheet.md` filled in (a
   `Result:` of PASS, FAIL or SKIP per test, `Notes:` in plain words, plus "Other things I noticed" and "Anything you want
   built"); every FAIL and every non-empty Notes line is an item for your triage table, cited by its ID (A1, F5…). If they give a
   different list, use that. If they haven't, give them the sheet. Say in one or two lines what you have read and that you are waiting for
   the list. Do not guess what it contains.

## What the owner is doing

They are trying the release app (`target/release/tlmbench-app`, built Nov 11 after R2.1) and will come back with a list. Expect
bugs, UI differences, requests and decisions, in any order and mixed. When it arrives:

- **Triage before you build.** Number each item, give each a verdict (bug, gap in a ledger row, new requirement, decision for
  the owner, can't reproduce) and the ledger row it belongs to (add a row when none does; never renumber an ID). Show the owner
  the short table and the order you propose, and what you will not do yet. Ask about anything that is a design decision with a
  recommendation (CLAUDE.md: decisions belong to the owner).
- **Reproduce each bug first**, ideally with the release build in daemon mode against the real core
  (`target/release/tlmbench-app --daemon --ui app/ui/dist`, as `app/ui/tests/t_exportreal.js` does) or the real window. If the
  owner's own settings are involved, work on a copy of `~/Library/Application Support/io.github.rafyarvelo.tlmbench/settings.json`
  and never change the original. The Explorer item below was found exactly that way.
- **Fix with a test for every behaviour change**, in small commits, one topic each, ledger updated in the same commit.
- Fixes to the owner's list come **before** R2.2. When the list is done, report what changed and what is left, then ask which
  step they want next (the plan says R2.2).

## Already known (put these on the list yourself, flagged as yours)

- **Explorer looks empty when "Show only these" is on and nothing is open.** The owner's saved settings have `exOnlyOpen: true`,
  so a freshly opened folder shows its header and no files (`list_folder` returned all entries; the filter hid them). It is in
  `docs/follow-ups.md`. Proposed, not decided: say "Showing only open files" with a Show all button when the filter is on and
  nothing is open, and clear the filter when a different folder is opened. Ask the owner before building it.
- **Owner pass rows from R2.1** (only a real window shows them; ask the owner to confirm each or put it on their list): the
  one-file export opened from a disk path, widget cards in a viewer, a check report's links (`#show=` framing a violation), the
  export dialog's three raw-window fields, a two-file export in the viewer, and Light/trimmed exports drawing labelled coarse.
- **Left Partial by R2.1:** a default `--detail` per rule-and-check set (D-cli-06); no viewer test drives Play, cursors or
  Measure (D-cli-09); the viewer's footer doesn't show the provenance; a time range for a tab of several files is refused by the
  app's Export; `tlmbench query view` shows no gap breaks until the pyramid is built. All in `docs/follow-ups.md`.
- **Not run at the end of R2.1:** the Python tests (`just` recipes need a `maturin` build; the crate compiles and lints). Run
  them when you next touch Python or the export manifest (its `raw_chunks` is now `[[first sample, samples], ...]`).
- **Design-doc notes owed** (the doc stays the source of truth; list them for the owner, don't edit the Claude Doc): checksums
  are BLAKE3, not SHA-256 (D-cli-08); checks run as a pass of their own after open (S5.3-04) and a set's TOML is what
  `examples/recordings/.tlmbench/checks/` shows (D-check-08); CSV `#` comment lines are Accepted (D-ingest-08); a check that
  reads a redacted channel leaves the export (D-cli-10); group routing for several files lives in `crates/web` (decision Nov 11).

## Rules

- A row moves to Done only with evidence you ran in your session (a test by name, a file, or a command with its date). Run
  `python3 ci/ledger_check.py` before every commit.
- A test for every behaviour change, the tests README's table, and a note for the design doc when behaviour differs from it.
- Headless tests don't see the real window. Anything only a real window shows is an Owner pass row with what to look at; before
  calling a step done, build the release app and hand the owner the checklist.
- The Mac has 8 GB of memory and a small disk. Never run two cargo builds or two UI suites at once; workspace-wide cargo only.
  Check `df -h /` before big builds and `rm -rf target/debug` when under 5 GB free (a full disk fails builds with a confusing
  error: it did in R2.1). `cargo clean --manifest-path python/Cargo.toml` after a Python build. The owner has OK'd deleting
  caches and builds.
- Build order: `just viewer` before the CLI or the app is built, or they embed a stale viewer. The release app is
  `cd app/src-tauri && cargo tauri build --no-bundle` (about 10 minutes). The viewer is the app's UI: a change to shared UI
  code changes both, so run the app's tests, not only `t_viewer`.
- UI tests run one at a time with `node tests/t_x.js` from `app/ui`, after `npx svelte-check --fail-on-warnings && npm run build`;
  `t_perf_gauges` runs alone at the end. `STRICT_SKIP=1 ./run.sh` turns a skipped test into a failure (the release gate).
- Shell: write scripts with the Write tool and run them; macOS `sed -i ''`; never `git stash`; add files by name (`git add -A`
  sweeps untracked files); don't push or rewrite history unless the owner asks. Commits end with
  `Co-Authored-By: Claude <noreply@anthropic.com>`.
- Concise replies to the owner: lead with what changed and what's left, no recaps. Never propose cutting scope.

## After the owner's list

Once the list is done and the owner says go, the plan continues with **R2.2 (browser-only mode)** in `release-plan.md`: parse in
a Worker, Parquet, `.tlm`, packets and descriptors (make `tlmbench-decode` build for WASM instead of including three of its
files as source), derived channels, events, checks and several files in the browser, a reload remembering which files were
open, and a static-host recipe. The store already builds for WASM (`native` off): R2.1 did that part.
