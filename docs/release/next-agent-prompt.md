You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to v1.0.0.
One agent works at a time: start from a clean `main`, leave a clean `main`. If another Claude session is running in this
repository, agree with it which files each of you touches before you edit.

This prompt is only a hand-off. **Where each step stands is the status table in `docs/release/release-plan.md`; what is
done is `docs/release/ledger.md`.** If this prompt and those disagree, they win, and you fix this prompt.

Written Nov 17 (night) after the Opus run. What it built is in the status table's "Opus run" row; the CI result of its
push is in the CI row.

## First

1. Read `CLAUDE.md` (its hard requirements bind you), then `docs/release/release-plan.md` (the status table) and
   `docs/release/owner-test-sheet.md` (sections W and X are new and untested).
2. Skim the memory files in `/Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/`, above all
   `owner-decisions-nov16.md` and `owner-decisions-nov17.md`, and any newer owner-decisions file.
3. Run the open-rows script: `python3 ci/ledger_check.py`, then list rows that aren't Done or Owner pass.
4. Ledger counts at the hand-off: Done 326, Partial 5, Missing 4, Diverged 2, Unverified 25, Owner pass 17 (379 rows).

## Decided by the owner Nov 17 (night), all as recommended: build these

- **S5.4-06:** canvas for the sparkline, XY trail and cloud and the GPS track, as the design's Accepted decision says;
  dials stay SVG. Keep the widget API; HiDPI, theme colours and reduced motion by hand.
- **S5.3-15:** the folder report as a separate Report tab in the tab strip (v1, R3.3), saved with the check set, reopened
  and re-run; the dialog's report moves there.
- **Events panel top row:** the app's is approved as App is better (ledger A-12, parity B17). Don't port.
- **Audit items (`ui-audits.md`, Nov 17):** "Streaming" in the accent colour (the LIVE badge stays red); a zero line and
  the residual range on the Alignment's residuals; the Explorer header's icons stay as they are.

## The owner's test notes (Nov 17 night, release build `e484d77`; sheet section W)

- **W1 (packet types in one tab): passed.** W4, W5 and all of section X are not tested yet.
- **W2 (every file of a tab): passed.** The owner asks whether Rules & checks' "Results for" dropdown should be a tree of
  every file, so all files' results show at once, and is open to discussion. Ask, with a recommendation (mine: a tree with
  a row per file, each folding to its sets, the primary open, since a dropdown hides failures in the other files).
- **W3 (Quiet interface): the owner saw no change and asks whether to delete the setting.** The design lists Quiet
  interface among its settings ("Each of these is a setting (Clicking a row, Tooltips, Quiet interface)"), so deleting it
  is a design change: ask the owner, don't decide. Recommendation: keep it, but make its effect visible (the setting's
  hint says what it hides, and its effects are subtle on a single file at rest); the alternative is to make quiet the
  default (the prototype's default is on) and drop the switch from Basic.
- **W4 (Reconnect): the owner found Ctrl-C on the test sender "unrecoverable".** The likely cause was the test sender: it
  restarted its clock at 0, so after a reconnect every frame read as earlier than the last and the live view held at the
  last time (time going backwards is held, not reordered). The sender in `~/tlmbench-testdata/nov17/live/` now keeps
  the time of day and has `--drop 15`; the owner re-tests. If it still fails, find out what the strip showed. Open
  question either way: a real device that resets its clock when its link drops gives the same picture. Recommendation:
  on a reconnect, a recording whose time goes backwards starts a new part (`source-1-2.tlm`, as a rotated tailed file
  does) rather than holding; ask the owner.

## Your rows

- **S4.3-12** (the import dialog's extra sections, parity P5): what is left is the prototype's Derived channels and
  Categories by prefix sections against the app's "Also for files with this header". Classify with the owner or port.
- **S5.6-13** (new, Missing): tables sort and filter by another file's columns, and XY across files (`group.rs`
  `xy_points` refuses two files). Core first (a held column for sort keys; XY by pairing on the shared timeline).
- **The rest of R3.4:** keyboard coverage against the prototype's list, every tooltip and empty state, the copy review
  (plain, short, sentence case). Then the UI freezes.
- **D-gauge-04** waits for R6's measurement; **D-live-09** is the owner's (review decision 5).
- Then R4 (packaging and release automation, Opus), per the plan.

## What is decided, and what you must not decide

- **Owner decisions are the owner's.** Where the design says "Open" or two readings are reasonable, stop the row and ask.
- **Never propose cutting scope.** Post-v1 items go to `docs/post-v1.md`.
- **"App is better" rows are never ported back.**
- The design (`docs/design.md`) is the source of truth; a behaviour change needs a note in `docs/follow-ups.md`.

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

## Report when you stop

1. Update the status table in `docs/release/release-plan.md` and push as above.
2. Rewrite this prompt for the next run.
3. Tell the owner: what changed (rows Done), the ledger counts, the questions waiting on them (each with your
   recommendation), the rows left, and the CI result. Then stop.
