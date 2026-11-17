You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to v1.0.0.
One agent works at a time: start from a clean `main`, leave a clean `main`. If another Claude session is running in this
repository, agree with it which files each of you touches before you edit.

This prompt is only a hand-off. **Where each step stands is the status table in `docs/release/release-plan.md`; what is
done is `docs/release/ledger.md`.** If this prompt and those disagree, they win, and you fix this prompt.

Written for **Opus** (Nov 17, after the Sonnet run and the run that built the owner's Nov 17 decisions). Everything those
two runs built is pushed and **CI is green on `ada6c41` (run 44134441175)**. The one unpushed commit is a docs-only status
update; it goes up with your first code push.

## First

1. Read `CLAUDE.md` (its hard requirements bind you), then `docs/release/release-plan.md` (the status table, and R2.5 to
   R3.4) and `docs/release/owner-test-sheet.md`.
2. Skim the memory files in `/Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/`, above all
   `owner-decisions-nov16.md` and `owner-decisions-nov17.md`.
3. Run the open-rows script from the ledger (rows still Partial, Missing or Diverged whose note names R2.x or R3.x).
4. Ledger counts at the hand-off: Done 311, Partial 16, Missing 5, Diverged 3, Unverified 25, Owner pass 17.

## Your rows, in this order

**1. The Opus list** (judgement across many files; nobody has started these):
- S4.3-08: packet types as slots of one tab.
- S5.6-12, S5.6-07, D-derive-03: events, checks, rules and derived channels on every file of a tab through its time map.
  This also makes the Events panel's "group by file" and file chips mean something (today each tab shows its primary
  file's events).
- S5.3-06: incremental live checks.
- D-ingest-09's last part: payload plugins in browser-only mode.
- D-live-11: Reconnect (the core's reconnect first). It also closes most of what is left of S4.1-21 (the prototype's TCP
  form has it).
- S5.4-06 (canvas traces): **ask the owner first**; the design has an Accepted decision that the app's SVG sparklines
  diverge from. Give a recommendation and the trade-off.
- D-gauge-04: after R6's measurement.
- R3.4: D-ui-10 (chips and grips), the quiet-interface pass, the whole-app apple-design audit (`ui-audits.md`).

**2. Rows left Partial that are well specified:**
- S5.3-15: the design wants the folder report as a separate Report tab (saved with the set, reopened, re-run). Today it is
  a dialog tab and the report is kept in settings (`folderReports`).
- S5.2-16: the per-tab Events filters are kept with the session (`Session.events`); the design keeps them with a saved
  layout too. Also the Events panel and channel editor scenes in `app/ui/tests/parity/scenes.js`.
- S4.1-19, S4.2-19 (and what is left of S4.1-21, S4.3-12): parity classification. Run `just app-parity`, read
  `docs/release/parity-classes.md` first, never port back an "App is better" row. The Connect scene now opens on TCP in
  both pages; the import dialog's differences are mostly the prototype's extra sections (P5).

**3. The owner sheet.** Nothing built on Nov 17 has been seen in a real window. Add sheet items (what to look at) and, if
the owner is present, try the release app yourself (`headless-tests-miss-real-window.md`): Settings › Icons and the pinned
strip; the Events box in the import dialog and the descriptor form; "Inputs and settings…" on a code widget; dragging a
channel onto a category and Cmd/Ctrl-click on categories; "Preview a frame" on a real UDP, TCP or serial source; the import
dialog's "Also for files with this header"; the Recipes manager's Import profiles tab; the badge flash on a failing live
check; renaming a saved connection; the folded "N checks can't run here" line; Save as rule from a table filter.

## What is decided, and what you must not decide

- **Owner decisions are the owner's.** Settled Nov 17 (memory `owner-decisions-nov17.md`): the pinned strip and icons, events
  from a marked column or descriptor field, widget inputs by role and parameters, category drops, Connect's "Preview a
  frame" (one second, only on a click), the regular-expression prefix pattern, `panes` saved in the profile, the folder-run
  report kept in settings. Where the design says "Open" or two readings are reasonable, stop the row and ask.
- **Never propose cutting scope.** Post-v1 items go to `docs/post-v1.md`.
- **"App is better" rows are never ported back.**
- The design (`docs/design.md`) is the source of truth; a behaviour change needs a note in `docs/follow-ups.md`.

## Rules

- **Done means, for each row:** built, a test for it (and the tests README's table), the ledger row moved with evidence and
  `python3 ci/ledger_check.py` passing with its counts line updated, `docs/follow-ups.md` where behaviour differs from the
  design, and an owner sheet item when only a real window shows it. Small commits, one row or topic each.
- **Push** once your rows are done and `just app-test`, `just proto-test`, `cargo nextest run --workspace`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` and `npm run typecheck` pass locally.
  The crates outside the workspace lint on their own: `crates/web` and `crates/web-parquet` (`cargo clippy --target
  wasm32-unknown-unknown`) include `decode` files as source, so **a new dependency in `decode` must also go into their
  `Cargo.toml` and lock files** (the Nov 17 regex did), and `python`. Then watch CI to green. A full run takes about 1.5
  hours and bills about 545 minutes: don't push per row. **Don't change `ci/` or the CI image.**
- **CI lessons (Nov 17):** `[profile.test]` has `debug = false` because the Windows linker's PDB limit (LNK1140) was hit by
  the store, decode and app test binaries; keep an eye on it as test binaries grow. A setting saved on a timer can race
  `t_windows` ("applying it does not send it back"): save only on a real change. `t_browserfolder` (the folder's plugin,
  jco in the page) failed once on a loaded Linux runner and passed on the rerun; if it fails again, find the race. Known
  flakes under full-suite local load: `t_browser`, `t_descriptor`, `t_windows` (pass alone).
- **Never edit Rust sources while a cargo build or test run is going in the background**: the run compiles a half-edited
  tree. Start background commands with `</dev/null` (a stray `cat` hung one for 15 minutes). A change to `decode` or
  `store` means a 20 to 40 minute workspace test build.
- Headless tests don't see the real window: anything only a real window shows is an Owner pass row with what to look at.
- The Mac has 8 GB of memory and a small disk (about 15 GB free at the hand-off). Never run two cargo builds or two UI suites
  at once. Check `df -h /`; `rm -rf target/debug` is fine (the owner OK'd it). `CARGO_PROFILE_DEV_DEBUG=0
  CARGO_INCREMENTAL=0` keeps a workspace run near 8 GB.
- Build order: `just viewer` before the CLI or the app; `cargo build --release -p tlmbench` for the release CLI
  (`t_viewer`, `t_viewerctl`, `t_exportreal`). UI tests: `./run.sh t_name` in `app/ui/tests`; the fake core is
  `fakecore.js`.
- In zsh write `"${var}:path"` when a colon follows a variable. macOS `sed -i ''`; never `git stash`; add files by name;
  don't rewrite history. End commits with the attribution line your session's instructions give.
- Replies to the owner: concise, lead with what changed and what's left.

## Report when you stop

1. Update the status table in `docs/release/release-plan.md` and push as above.
2. Rewrite this prompt for the next run.
3. Tell the owner: what changed (rows Done), the ledger counts, the questions waiting on them (each with your
   recommendation), the rows left, and the CI result. Then stop.
