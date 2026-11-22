You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to v1.0.0.
One agent works at a time: start from a clean `main`, leave a clean `main`. If another Claude session is running in this
repository, agree with it which files each of you touches before you edit.

This prompt is only a hand-off. **Where each step stands is the status table in `docs/release/release-plan.md`; what is
done is `docs/release/ledger.md`.** If this prompt and those disagree, they win, and you fix this prompt.

Written Nov 22 after the owner's-test-notes run. What it built is in the status table's "Owner's test notes run" row; the
CI result of its push is in the CI row.

## First

1. Read `CLAUDE.md` (its hard requirements bind you), then `docs/release/release-plan.md` (the status table) and
   `docs/release/owner-test-sheet.md` (section Y is what the owner hasn't tried yet; fill in anything they wrote).
2. Skim the memory files in `/Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/`, above all the
   owner-decisions files (newest: `owner-decisions-nov22.md`).
3. Run the open-rows script: `python3 ci/ledger_check.py`, then list rows that aren't Done or Owner pass.
4. Ledger counts at the hand-off: Done 331, Partial 4, Missing 4, Diverged 1, Unverified 25, Owner pass 17 (382 rows).

## Built Nov 22 (all pushed): wait for the owner's sheet before changing them

- The owner's two blockers (D-ui-65, D-ui-66), X3 Your widgets as a list and a form (the owner chose the layout), X1 pins,
  X4 one Undo, W3 quiet pane headers, W2 results as a tree of files, X2 read-only events tagged, W4 a clock reset starts
  the source's next part (D-live-13), S5.3-15 the Report tab, S5.4-06 canvas traces, the two Nov 17 audit items.
- **Not checked in a real window by the agent**, the WKWebView drag above all (the blockers): the owner's sheet Y1 to Y8.
  If the owner reports a failure there, that comes first.

## Open questions for the owner (ask, with a recommendation; don't decide)

- **Quiet interface's default** (W3): the prototype's is on, the app's off. Recommendation: keep it off until the owner
  has tried Y5, then ask.
- **S4.3-12** (the import dialog's extra sections, parity P5): the prototype's Derived channels and Categories by prefix
  sections against the app's "Also for files with this header". Recommendation: App is better (the profile section covers
  both and applies to the next file with the header); ask before porting.

## Your rows

- **S5.6-13** (Missing): tables sort and filter by another file's columns, and XY across files (`group.rs` `xy_points`
  refuses two files). Core first (a held column for sort keys; XY by pairing on the shared timeline).
- **The rest of R3.4:** keyboard coverage against the prototype's list, every tooltip and empty state, the copy review
  (plain, short, sentence case). Then the UI freezes. Undo: a layout's rename and delete still undo only from their note
  (they aren't a file's edits); say so in the review or add them to the history.
- **D-gauge-04** waits for R6's measurement; **D-live-09** is the owner's (review decision 5).
- Then R4 (packaging and release automation, Opus), per the plan.

## What is decided, and what you must not decide

- **Owner decisions are the owner's.** Where the design says "Open" or two readings are reasonable, stop the row and ask.
- **Never propose cutting scope.** Post-v1 items go to `docs/post-v1.md`.
- **"App is better" rows are never ported back.**
- The design (`docs/design.md`) is the source of truth; a behaviour change needs a note in `docs/follow-ups.md` (the
  Nov 22 notes are under "The owner's test notes (Nov 22)").

## Rules

- **Done means, for each row:** built, a test for it (and the tests README's table), the ledger row moved with evidence and
  `python3 ci/ledger_check.py` passing with its counts line updated, `docs/follow-ups.md` where behaviour differs from the
  design, and an owner sheet item when only a real window shows it. Small commits, one row or topic each.
- **Push** once your rows are done and `just app-test`, `just proto-test`, `cargo nextest run --workspace`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` and `npm run typecheck` pass locally,
  plus `cargo clippy --target wasm32-unknown-unknown -- -D warnings` and `cargo fmt --check` in `crates/web` and
  `crates/web-parquet` (they include `decode` files as source: a new dependency in `decode` goes into their `Cargo.toml`
  and lock files too), and `python`. Then watch CI to green. A full run takes about 1.5 hours and bills about 545 minutes:
  don't push per row. **Don't change `ci/` or the CI image.**
- **Lessons (Nov 17):**
  - The UI tests' harness waits a fixed 600 ms for the page; under a cargo build every core is busy and tests fail at boot
    (`__tlmApp` undefined). Never run the UI suite while cargo builds; rerun such failures alone.
  - `node app/ui/tests/shots.js` with no argument writes into `docs/images` (tracked): pass an output folder.
  - The parity harness (`just app-parity`) found two real layout faults on its first run of new scenes: add a scene when
    you touch a panel.
  - Browser-only mode keeps a tab per packet type (several recordings in one browser tab are after v1); the app's slots
    path is skipped there (`ui.browserOnly`).
  - `[profile.test]` has `debug = false` (Windows' PDB limit). Known flakes under full-suite load: `t_browser`,
    `t_descriptor`, `t_windows` (pass alone); `t_browserfolder` failed once on a loaded Linux runner.
- **Never edit Rust sources while a cargo build is going** (tests that only run already-built binaries are fine). Start
  background commands with `</dev/null`. A change to `decode` or `store` means a 20 to 40 minute workspace test build.
- The Mac has 8 GB of memory and a small disk. Never run two cargo builds or two UI suites at once. Check `df -h /`;
  `rm -rf target/debug` is fine (the owner OK'd it). `CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0` keeps a workspace run
  near 8 GB.
- Build order: `just viewer` before the CLI or the app (and before the browser tests: `app/browser/dist`); `cargo build
  --release -p tlmbench` for the release CLI. UI tests: `./run.sh t_name` in `app/ui/tests`; the fake core is `fakecore.js`.
- In zsh write `"${var}:path"` when a colon follows a variable. macOS `sed -i ''`; never `git stash`; add files by name;
  don't rewrite history. End commits with the attribution line your session's instructions give.
- Replies to the owner: concise, lead with what changed and what's left.

- **Lessons (Nov 22):**
  - `tlmbench-plugin::estimator the_template_estimator_lines_two_files_up_through_the_store` failed once under the full
    workspace run and passed alone and on a full rerun: a load flake to watch, like the UI ones above.
  - Run cargo workspace-wide only (`-p` builds another feature set and fills the disk); clean `target/debug` when done.
  - A `git add` with a path that no longer exists (a file you `git mv`ed) aborts an `&&` chain: the commit then holds only
    what was staged before. Check `git show --stat HEAD` after a multi-file commit.
  - Browser-only tests run against `app/browser/dist`, built by `just viewer`: rebuild it after UI changes or a local
    pass proves nothing (CI builds it fresh). A wait whose result isn't checked hides a slow boot as a later failure.
  - The parity scenes `drag-edge*` report a 2 px and 6 px size gap in the drop label box; known noise, not behaviour.

## Report when you stop

1. Update the status table in `docs/release/release-plan.md` and push as above.
2. Rewrite this prompt for the next run.
3. Tell the owner: what changed (rows Done), the ledger counts, the questions waiting on them (each with your
   recommendation), the rows left, and the CI result. Then stop.
