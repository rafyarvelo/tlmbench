You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to the v1
release. The features are built (roadmap steps 4.1 to 5.10) and a review (step R1) has checked every requirement against
the code and the tests. Your job is to start closing the gaps it found, beginning with **step R2.1 (exports)** of
`docs/release/release-plan.md`. One agent works at a time; you start from a clean
`main` and leave a clean `main`.

## Read first, in this order

1. `CLAUDE.md`. Its hard requirements bind you: the app works from its cache alone on a read-only folder; nothing is
   written next to data except by Share and exports; exports are static files; everything in the design is v1 and **you
   never propose cutting scope** (if something looks infeasible, say so with evidence and ask the owner); the app never
   runs Python or other programs.
2. `docs/release/release-plan.md`: the steps R0 to R9, their order, and the ten decisions the owner has to make.
3. `docs/release/ledger.md`: one row per requirement with a verdict, the evidence and the step that closes each gap. From
   here on a feature is done only when its row is Done with evidence `ci/ledger_check.py` can check. The header of
   `ci/ledger_check.py` says how evidence is written.
4. `docs/release/review.md`: the ranked gaps, the drift found in the documents, what couldn't be checked, and the ten
   decisions the owner needs to make (section 6).
5. `docs/design.md` for whatever you are about to build (it is the source of truth), `docs/follow-ups.md` "Left from step
   5.10", and the memory file `viewer-architecture.md` in
   `/Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/` (how the export, the viewer and `crates/web` fit
   together), plus `disk-space.md` and `headless-tests-miss-real-window.md` there.

## Where things stand (Nov 11, 2026)

- **R1 is finished.** `docs/release/review.md` is written, `python3 ci/ledger_check.py` prints `ok`, and the review is
  committed. The ledger has 305 rows: Done 194, Partial 46, Missing 18, Diverged 7, Unverified 26, Owner pass 14.
- Verified in the review, all on this Mac at `d45727c`: `cargo fmt --all --check`, clippy and `just lint` clean; 269
  Rust tests pass (1 skipped: the sort timing test, release only); all 89 app UI tests pass, run one at a time, none
  rerun; all 29 prototype tests and 9 Python tests pass; `just ro-test`, `just gate4 matrix` and `just gate5` pass;
  `just app-parity` reports 212 differences in 23 scenes.
- Not re-run (they need the release window or the 10 GB recordings; R6 does them): Gates 1 and 2, `gate3 core` and
  `app`, `gate4 app` and the 10 GB packet file, `just fuzz-smoke`, `just bench`.
- CI run 43073397717 on `d45727c`: macOS and Windows passed; the Linux job was still running after two and a half
  hours. Check it first (`gh run view 43073397717`). No 5.10 commit has a fully green run yet, and R0 asks for two in a
  row with no rerun: if Linux failed on a known flake (`t_live`, `t_trigger`, `t_checks`, `t_windows`, the prototype's
  `t_folder`), find the race before starting R2.1. None of them failed in the review's run on the Mac.
- Everything of R1 is committed on `main` (not pushed; don't push unless the owner asks).

## Decisions: taken as the review recommended

The owner read `review.md` and said to begin with its recommendations (Nov 11). Treat these as decided; don't ask again.
Where one turns out not to work, stop and say so with evidence before building around it.

1. **The store for WASM: one store everywhere.** Make the part of `tlmbench-store` that groups, time maps, checks and
   derived channels need build for `wasm32-unknown-unknown`, with what can't (mmap, arrow and parquet, zstd, threads,
   the cache on disk) behind features or a small storage trait, and have `crates/web` use it, instead of writing a
   second group inside `crates/web`. First spend an hour finding what actually stops the build
   (`cargo check --target wasm32-unknown-unknown` on the store and on decode) and tell the owner what you found and how
   you'll split it. If it would take a rewrite of the store rather than a split, that is the case to stop and ask.
2. **Export checksums: keep BLAKE3.** The viewer verifies every file it fetches against `export.json` and shows an
   error instead of data on a mismatch (a truncated upload, a stale cached chunk). Note for the design doc that it says
   SHA-256 and should say BLAKE3 (D-cli-08).
3. **The design absorbs what the code settled:** checks run as their own pass after open (S5.3-04), and a check set's
   TOML is what `examples/recordings/.tlmbench/checks/` shows (D-check-08). Mark both rows Done with the note for the
   design doc; no code changes.
4. **Widget traces stay SVG until measured.** R6 measures 100 widgets in the release app; canvas only if they miss
   2 ms (S5.4-06 stays Diverged until then).
5. **Live views stay whole views at 30 Hz**, noted for the design doc, unless R6's soak shows a cost (D-live-09).
6. **CSV `#` comment lines are Accepted** (D-ingest-08): note it for the design doc.
7. **Packet types become slots of one tab** (S4.3-08), in R2.5.
8. **No phone layout in the app**; `t_phone` stays prototype-only.
9. **The release gate fails when a UI test skips; the gate harness stays in release builds.** Make
   `app/ui/tests/run.sh` take a flag (or an environment variable) that turns a skip into a failure, and use it in CI
   once CI builds what those tests need.
10. **CI first.** Check run 43073397717 before anything else. If the Linux job failed, fix that before R2.1: for a known
    flake, find the race (a retry is not a fix). R0 is done at two green runs in a row with no rerun; pushing is the
    owner's call, so ask when you need a run.

The ten release decisions in `release-plan.md` (licence, platforms, signing and the rest) are still open and not needed
until R4; don't hold R2 for them, and don't decide them.

## The work: R2.1, exports

The list is `release-plan.md`'s R2.1 plus every ledger row whose last column names R2.1 (`grep -n "R2.1"
docs/release/ledger.md`). Core and CLI first, tested headlessly, then the viewer page. The writer is
`crates/export/src/html.rs`, the viewer's core `crates/web/src/{data,ops}.rs`, the page `app/ui/src/viewer.ts`, the CLI
`cli/src/export_html.rs`. What makes it hard:

- **Light's raw windows and a trimmed export** both need the viewer's core to hold data that is partly absent: raw
  chunks present for some stretches only, and a pyramid whose finest level is not level 4. Today it assumes all or
  nothing, and a trimmed channel shows nothing at all. Design the sparse store and the plan for which chunks a view
  reads before writing either feature; the integrity rule (every value shown is a raw sample or an exact aggregate, and
  a view that lacks its raw samples says so) is what the tests must hold.
- **`file://` does not work for a folder export** (the browser refuses to fetch the WASM core and the data), and
  `--single-file` is the design's answer. Don't paper over it: build the one-file variant and say plainly in the docs
  why a folder needs a server.
- **Build order:** `just viewer` must run before the CLI or the app is built, or they embed a stale viewer.
- **The viewer is the app's UI.** A change to shared UI code changes the app too; run the app's tests, not only
  `t_viewer`.

## Rules

- A row moves to Done only with evidence you ran in your session: a test by name, a file, or a command with its date.
  Update the ledger row in the same commit as the code, never renumber an ID, and run `python3 ci/ledger_check.py`
  before every commit. If you find a gap the ledger lacks, add a row.
- A test for every behaviour change, the tests README's table, and a note for the design doc when behaviour differs
  from it (CLAUDE.md).
- Headless tests don't see the real window. Anything only a real window shows is an Owner pass row with what to look
  at; before calling the step done, build the release app and hand the owner the checklist.
- The Mac has 8 GB of memory and a small disk. Never run two cargo builds or two UI suites at once. Workspace-wide cargo
  only (`-p` subsets pile up multi-GB copies; note that `just ro-test` builds with `-p` and cost 9 GB in the review).
  Check `df -h /` before big builds; `rm -rf target/debug` when under 5 GB free; `cargo clean --manifest-path
  python/Cargo.toml` after a Python build. The owner has OK'd deleting caches and builds.
- UI tests run one at a time with `node tests/t_x.js` from `app/ui`, after `npx svelte-check --fail-on-warnings &&
  npm run build`. `t_perf_gauges` runs alone at the end.
- Shell: write scripts with the Write tool and run them (the safety check rejects some heredocs); macOS `sed -i ''`;
  never `git stash`; `git add -A` sweeps untracked files, so add files by name; don't push or rewrite history unless the
  owner asks. Small commits, one topic each, ending with `Co-Authored-By: Claude <noreply@anthropic.com>`.
- Concise replies to the owner: lead with what changed and what's left, no recaps.

## Done when

Every ledger row that names R2.1 is Done with evidence; `just gate5`, the whole app UI suite, the Rust tests and
`just lint` pass; the roadmap's status table, the tests READMEs and `docs/follow-ups.md` say what changed; and you have
told the owner what is done, what is left, and which step comes next (R2.2).
