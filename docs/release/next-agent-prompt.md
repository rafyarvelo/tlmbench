You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to v1.0.0.
One agent works at a time: start from a clean `main`, leave a clean `main`. If another Claude session is running in this
repository, agree with it which files each of you touches before you edit.

This prompt is only a hand-off. **Where each step stands is the status table in `docs/release/release-plan.md`; what is
done is `docs/release/ledger.md`.** If this prompt and those disagree, they win, and you fix this prompt.

Written for **Opus** (after the Sonnet run of Nov 17, which built the well-specified rows of R2.5, R3.2 and R3.3; see the
status table, the commit "Import profiles carry check sets…" and `docs/follow-ups.md`). The Sonnet run's work is built
and tested locally but **not pushed**: if the owner has not pushed it, that comes first (below).

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

**Rows the Sonnet run stopped on a question. The owner accepted every recommendation below on Nov 17 (memory
`owner-decisions-nov17.md`): build them as written, don't ask again.** The same answer settles the three choices the Sonnet
run made (regular-expression prefix pattern, `panes` saved in the profile, folder-run report in settings):

- **D-gauge-06** (icons for categories, widget menus and toolbar buttons). Question: where does the "toolbar" with a
  person's recipe and layout buttons live? The app has no user toolbar. Recommendation: a "Pinned" strip in the top bar,
  shown only when something is pinned (Layouts rows and the Recipes manager get "Pin to toolbar"); the pin shows its icon,
  the name in its tip; icons come from a small Icons list in Settings (an SVG using `currentColor`, shared to
  `.tlmbench/icons/`). Trade-off: the top bar is the calmest part of the UI, so keep pins few (at most 8).
- **D-ui-12** (events from the file, with the channels they concern; the Events panel's file chips). Question: what in a
  file makes an event? Recommendation: a column or descriptor field marked "Events" (in the import dialog and the
  descriptor) makes an event per change, name = the new value or label, channels = that column's; format metadata (Parquet
  key/value, HDF5 attributes) after v1. The panel's "group by file" and file chips follow.
- **S5.4-10** (code widgets: inputs by role, parameters in `widget.toml`; X-win-07's frame scheme). Question: where does a
  person bind a role (`pitch`, `roll`, `series`) to a channel, and how does a parameter get its value? Recommendation: the
  widget menu gets an "Inputs…" section listing the manifest's roles, each a channel picker (default: the widget's own
  channel); parameters are small typed fields from the manifest (number, text, choice) saved per widget in the layout; a
  series is the last 64 samples at the reading time, sent with the value in the one postMessage per frame.
- **D-ui-09** (Explorer drops between rows and onto a category; categories multi-select; categories from the descriptor).
  Question: what do those two drops do? Reading A: onto a category row moves the dragged channels into it (the per-file
  `catOverride`); between rows reorders channels inside a pane's list. Reading B: between rows is a new pane between two
  panes. Also the descriptor has no `category` on a field (`Field` literals in about 20 places). Recommendation: A for the
  category drop; the existing pane-edge and gap drops stay; add `category` to descriptor fields with the import profile's key.
- **S5.5-02** (the Connect dialog previews decoded frames). Question: may the preview read a live stream? A file or replay
  source is safe. UDP must bind the port for about a second, TCP and serial open the device. Recommendation: yes, only on
  an explicit "Preview" click, a 1 s read, never on opening the dialog, saying "reads for a second"; shows the first decoded
  frame's values and the rate seen.

**Rows the Sonnet run did not start** (well specified, Sonnet or Opus): S4.1-19, S4.1-21, S4.3-12, S4.2-19 (the parity
scenes: run `just app-parity`, read `docs/release/parity-classes.md` first, never port back an "App is better" row), S5.2-16's
Events panel and channel editor scenes, S5.6-10 (port the multi-file tests: the Time channel from `gps_tow`; the prefix rule
is covered by `t_profileextras`), D-cli-09 (viewer tests driving Play, cursors and Measure: needs `just viewer` and the release CLI).

**Rows left Partial after the Sonnet run, to finish:** S5.3-15 (the design wants the folder report as a separate Report
tab; the dialog tab keeps the report with the set), D-cli-06 (an end-to-end test that an HTML export's windows follow a
set's `detail`: write it in `cli/tests`), S5.2-16 (saving the per-tab event filters with the layout and the session).

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
