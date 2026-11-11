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
- Uncommitted and the owner's own: the edit to `docs/README.md`, and `docs/release/release-plan.md` and
  `review-prompt.md` (untracked). Leave them unless the owner asks; add files by name.

## Then: ask the owner before building

Decisions belong to the owner. Ask these in one message, each with your short recommendation, and build nothing that
depends on an answer you don't have:

1. **The store for WASM** (needed by R2.1 and R2.2): make the needed part of `tlmbench-store` build for
   `wasm32-unknown-unknown`, or write a group inside `crates/web`. Look at what stops the store building for WASM
   (arrow, zstd, mmap) before you recommend.
2. **The export's checksums:** keep BLAKE3 and change the design, or write SHA-256 as the design says. Either way the
   viewer must verify each chunk as it loads (D-cli-08).
3. Whether to confirm R2.1 as the next step, or take R0's de-flaking first if the Linux CI job failed on a known flake.
4. Whichever of `review.md`'s decisions (section 6) the owner hasn't answered; numbers 3 to 9 there don't block
   R2.1, so don't wait on them.

The ten release decisions in `release-plan.md` (licence, platforms, signing and the rest) are not needed until R4; don't
hold R2 for them.

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

The owner has answered the decisions R2.1
needs; every ledger row that names R2.1 is Done with evidence; `just gate5`, the whole app UI suite, the Rust tests and
`just lint` pass; the roadmap's status table, the tests READMEs and `docs/follow-ups.md` say what changed; and you have
told the owner what is done, what is left, and which step comes next (R2.2).
