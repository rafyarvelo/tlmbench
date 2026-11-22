You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to v1.0.0.
One agent works at a time: start from a clean `main`, leave a clean `main`. If another Claude session is running in this
repository, agree with it which files each of you touches before you edit.

This prompt is only a hand-off. **Where each step stands is the status table in `docs/release/release-plan.md`; what is
done is `docs/release/ledger.md`.** If this prompt and those disagree, they win, and you fix this prompt.

Written Nov 17 (night) after the Opus run; the owner's test notes added Nov 22. What it built is in the status table's "Opus run" row; the CI result of its
push is in the CI row.

## First

1. Read `CLAUDE.md` (its hard requirements bind you), then `docs/release/release-plan.md` (the status table) and
   `docs/release/owner-test-sheet.md` (the owner's results in sections W and X, and "Other things I noticed").
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

## The owner's test notes (Nov 17 to 22, release build `e484d77`; sheet sections W and X): handle these first

**Blocker (sheet, "Other things I noticed"):** dragging a chip from a pane of two channels and dropping it at the pane's
left or right edge copies the channel into the new pane instead of moving it (it stays in the original pane). Always.
Expected: a new pane with the channel, the channel gone from where it was (a copy only with Option/Alt, as the chip's
tooltip says). Fix first, with a test that drops at both edges; check `t_edgedrop`, `t_dnd` and the parity `drag-edge*`
scenes, and the real window (WKWebView drag differs from Chromium's).

**Second blocker (same section):** a layout of one chart on top, two charts side by side in the second row and one at
the bottom: closing one of the two side-by-side charts moves the other up into the top row instead of letting it widen
to fill its own row. Always. Expected: the remaining pane keeps its row and grows across it. Look at how removing a pane
handles `beside` (the pane after a removed one inherits its row; `stack.svelte.ts` and the pane-close paths), add a test
for closing either of the pair, and check it in the real window.

**Failed or needing work:**
- **X3 (code widgets): FAIL, "this code widget dialog currently sucks and needs major rework".** In the Gauges pane's widget
  dialog the "Your widgets" button is nearly cut off; after adding a widget it becomes unreachable (the dialog doesn't
  scroll); there is no way to edit a widget. Rework it (scrolling, edit, a clear list of your widgets); ask the owner
  about the layout before building if it changes the design.
- **X1 (icons and pins):** adding an SVG works, but the paste box is tiny and can't be resized; pinning doesn't work for
  shared layouts; the layout's right-click menu has no Unpin (only Settings does). Fix all three.
- **X4 (drag onto a category):** works, but Undo only reverses the most recent channel dragged, where moving channels
  between panes undoes several moves. The owner: "Undo should be consistent across the UI": category moves join the same
  undo history (⌘Z steps back through them one by one).
- **W3 (Quiet interface):** the owner compared with the prototype: the setting makes sense there but doesn't work right in
  the app. Compare both side by side (`prototype/plotter.html`, `.app.quiet` rules around line 206) and port what the
  prototype's quiet mode does that the app's doesn't. Keep the setting.

**Questions to discuss with the owner (don't decide):**
- **W2:** Rules & checks' "Results for" dropdown, or a tree of every file so all results show at once. Recommendation: a
  tree, a row per file folding to its sets, the primary open.
- **X2:** "What is a read-only event? How does that work?" Events made by the file (a marked column), by rules and by checks
  can't be edited or deleted in the Events panel, since they are recomputed from the data; only events you add can.
  Answer the owner and consider saying so in the panel (a tooltip or the row's tag).
- **W4 (Reconnect passed with the fixed sender):** the owner asks what should happen when a device's time goes backwards
  (a reset timer), whether a reconnect should still show the fields, and whether a live chart's X axis should come from the
  source's time at all or be a history buffer of samples. A design discussion: explain how it works now (live charts use
  the recording's time column; time going backwards is held at the last time, so a reset device freezes the view), give
  the options (keep source time and start a new part on a reset; arrival time for live; a sample-count axis) with a
  recommendation, and let the owner decide.

**Passed:** W1, W2, W4 (with the fixed sender), X2, X5. Not tested: W5, X6, X7, X8.

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
