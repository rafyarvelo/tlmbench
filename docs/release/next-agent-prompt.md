You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to v1.0.0.
One agent works at a time: start from a clean `main`, leave a clean `main`. If another Claude session is running in this
repository, agree with it which files each of you touches before you edit.

This prompt is only a hand-off. **Where each step stands is the status table in `docs/release/release-plan.md`; what is
done is `docs/release/ledger.md`.** If this prompt and those disagree, they win, and you fix this prompt.

Written for **Opus** (after the Sonnet run of Nov 17, which built the well-specified rows of R2.5, R3.2 and R3.3; see the
status table, the commit "Import profiles carry check sets…" and `docs/follow-ups.md`). The Sonnet run's work and the work from
the owner's Nov 17 decisions were pushed on Nov 17 (the status table says with which CI run).

## First

1. Read `CLAUDE.md` (its hard requirements bind you), then `docs/release/release-plan.md` (status table and sections
   R2.5 to R3.4) and `docs/release/owner-test-sheet.md`.
2. Skim the memory files in `/Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/`.
3. Run the open-rows script from the ledger (rows still Partial, Missing or Diverged whose note names R2.x or R3.x).
4. Push (if the owner has not): the repo is public. `just app-test`, `just proto-test` (the prototype was not touched) and
   `cargo nextest run --workspace` passed locally at the Sonnet run's end (UI 118 of 118 after the fixes noted in its
   commit; Rust 298 passed, 1 skipped); `cargo clippy --workspace --all-targets -- -D warnings` and `npm run typecheck`
   were not run to the end, run them first. Then watch CI to green with no rerun. **Don't change `ci/` or the CI image.**

## Your rows

**The Opus list** (left untouched by the Sonnet run): S4.3-08 (packet types as slots of one tab), S5.6-12, S5.6-07 and
D-derive-03 (events, checks, rules and derived channels on every file of a tab through its time map), S5.3-06 (incremental
live checks), D-ingest-09's last part (payload plugins in browser-only mode), D-live-11 (Reconnect: needs the core's
reconnect first), S5.4-06 (canvas traces: owner decision first), D-gauge-04 (after R6's measurement), D-ui-10 and everything
else in R3.4 (the quiet-interface pass, the whole-app apple-design audit).

**Built Nov 17 from the owner's decisions** (memory `owner-decisions-nov17.md`): D-gauge-06, D-ui-12, S5.4-10, D-ui-09,
S5.5-02, and with them D-cli-06, D-cli-09 and S5.6-10. They need a real-window pass: the owner sheet has no entries yet for
the pinned strip and Settings › Icons, the Events box in the import dialog and the descriptor form, "Inputs and settings…",
dragging a channel onto a category, "Preview a frame" on a real UDP, TCP or serial source.

**Rows still open and well specified:**
- S4.1-19, S4.2-19 (parity classification: run `just app-parity`, read `docs/release/parity-classes.md` first, never port back
  an "App is better" row), S5.2-16's Events panel and channel editor scenes.
- S5.3-15: the design wants the folder report as a separate Report tab (saved with the set, reopened, re-run); today it is
  a dialog tab with the report kept in settings.
- S5.2-16: the per-tab event filters are kept with the session; the design also keeps them with a saved layout.

## What is decided, and what you must not decide

- **Owner decisions are the owner's**: memory `owner-decisions-nov14.md`, `owner-decisions-nov16.md`, `docs/follow-ups.md`.
  Where the design says "Open" or two readings are reasonable, stop the row and ask (recommendation and trade-off).
- **Never propose cutting scope.** Post-v1 items go to `docs/post-v1.md`.
- **"App is better" rows are never ported back.**
- The design (`docs/design.md`) is the source of truth; a behaviour change needs a note in `docs/follow-ups.md`.
- The Sonnet run chose, and the owner should confirm: the profile prefix rule's "by pattern" is a regular expression (first
  group, else the whole match); a profile's `panes` choice is saved with the profile (the design says the profile "says which
  panes the file opens with"); the folder-run report is kept in settings (this computer).

## Rules

- **Done means, for each row:** built, a test for it (and the tests README's table), the ledger row moved with evidence and
  `python3 ci/ledger_check.py` passing with its counts line updated, `docs/follow-ups.md` where behaviour differs from the
  design, and an owner sheet item when only a real window shows it. Small commits, one row or topic each (the Sonnet run's
  single big commit is the exception; don't copy it).
- **Never edit Rust sources while a cargo build or test run is going in the background**: the run compiles a half-edited
  tree. A background command that reads stdin (a stray `cat`) hangs forever: start background cargo with `</dev/null`.
- Headless tests don't see the real window: anything only a real window shows is an Owner pass row with what to look at.
  The Sonnet run did not open the release app; the owner sheet needs entries for: the import dialog's new section, the
  Recipes manager's profiles tab, the badge flash, the saved-connection rename, the folded "can't run" line.
- The Mac has 8 GB of memory and a small disk. Never run two cargo builds or two UI suites at once. Check `df -h /`.
  `CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0` keeps a workspace run near 8 GB. A workspace test build takes 20 to 40
  minutes after a change to `decode` or `store`.
- Build order: `just viewer` before the CLI or the app. UI tests: `./run.sh t_name` in `app/ui/tests`; `t_viewer`,
  `t_viewerctl`, `t_exportreal` need `just viewer` and the release CLI. Known flakes under load: `t_browser`, `t_descriptor`.
- In zsh write `"${var}:path"` when a colon follows a variable. macOS `sed -i ''`; never `git stash`; add files by name;
  don't rewrite history. End commits with the attribution line your session's instructions give.
- Replies to the owner: concise, lead with what changed and what's left.

## Report when you stop

1. Update the status table in `docs/release/release-plan.md` and push as above.
2. Rewrite this prompt for the next run.
3. Tell the owner: what changed (rows Done), the ledger counts, the questions waiting on them (each with your
   recommendation), the rows left, and the CI result. Then stop.
