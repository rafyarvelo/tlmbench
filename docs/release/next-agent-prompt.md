You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to v1.0.0.
One agent works at a time: start from a clean `main`, leave a clean `main`. If another Claude session is running in this
repository, agree with it which files each of you touches before you edit.

This prompt is only a hand-off. **Where each step stands is the status table in `docs/release/release-plan.md`; what is
done is `docs/release/ledger.md`.** If this prompt and those disagree, they win, and you fix this prompt.

Written for **Sonnet** (owner, Nov 17): you take every well-specified row left in R2.1, R2.5, R3.1, R3.2 and R3.3, and leave
the rows on the **Opus list** below untouched for the Opus run after you. You build from what the ledger rows and the
design say; you don't make design calls. **At the first design question, stop that row, write the question down (see
"Report"), and go on to the next row.** Don't start R3.4 or R4.

## First

1. Read `CLAUDE.md` (its hard requirements bind you), then `docs/release/release-plan.md` (the status table, and the sections
   R2.5 to R3.3), and `docs/release/owner-test-sheet.md`.
2. Skim the memory files in `/Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/` (`release-plan.md`,
   `disk-space.md`, `headless-tests-miss-real-window.md`, `viewer-architecture.md`, `owner-decisions-nov16.md`).
3. The open rows. Run this from the repository root and treat its output, minus the Opus list, as your list:
   ```
   python3 - <<'E'
   import re
   for l in open('docs/release/ledger.md'):
       if not l.startswith('| '): continue
       c = l.split('|')
       if len(c) < 7 or c[4].strip() not in ('Partial', 'Missing', 'Diverged'): continue
       m = re.search(r'\b(R2\.[1-5]|R3\.[0-4])\b', c[6])
       if m: print(c[1].strip(), c[4].strip(), m.group(1))
   E
   ```
   Read each row's whole line (what it asks, the evidence, the note) before you start it.

## Your rows (Nov 17), in this order

1. **R2.5, the small core rows:** D-ui-57 (the import dialog's choice of how a file opens, defaulting to Settings' "Files
   opened for the first time"), D-gauge-03 (a setting that turns code widgets off), D-check-01 (attach a set to an import
   profile in the app, not only the CLI), D-derive-01 (an import profile's own starter layout), S5.3-14 (Save as rule from
   a table filter; the prototype has it, `proto:t_filters`), D-share-01 (channel prefix rules in the import profile and
   dialog), D-cfg-04 (the one-click offer to update references to a renamed or moved recording), D-cfg-07 (a shared layout
   embeds the recipes it plots and offers to add missing ones), D-gauge-06 (icons for categories, widget menus and toolbar
   items), D-ui-12 (events from the file list the channels they concern; the Events panel's file chips), S5.1-10 (recipes
   applying to a profile: the import dialog's Derived section, "this profile" in Derive…), S4.3-11 (port the import and
   descriptor tests once their features exist), S5.4-10 (code widgets: inputs by role and parameters in `widget.toml`;
   if the frame scheme X-win-07 needs a decision, stop there).
2. **R3.2:** D-ui-01 (rename and edit saved connections), D-ui-09 (Explorer: drops on a pane's edge, between rows and onto
   a category; multi-select categories; categories from the descriptor), S5.5-02 (the Connect dialog previews decoded
   frames: a short read of the stream decoded by the descriptor), S4.1-21 and S4.3-12 (the Connect, import and descriptor
   scenes match the prototype in the harness).
3. **R3.3:** D-ui-59 (a code widget made in the app without the JSON), S5.1-12 (the import-profiles tab of the Recipes
   manager), S5.2-13's rest (a tag click reveals the event), S5.2-16 ("Charts follow these filters" per tab, and the
   scenes), S5.3-15 (folder run with sub-folders; the report saved with the set and reopened), S5.4-05 (widget text at
   most 10 updates a second), D-check-03 (one folded "N checks can't run here" line), D-check-04 (a failing live check
   flashes the tab badge and adds an event), S5.6-10 (port what is built of the multi-file tests), S4.2-19 (the table
   scenes).
4. **Older rows:** D-cli-06 (a default Light detail per rule-and-check set), D-cli-09 (viewer tests that drive Play,
   cursors and Measure), S4.1-19 (parity scene differences outside the accepted list; `docs/release/parity-classes.md`).

## The Opus list: don't start these

S4.3-08 (packet types as slots of one tab), S5.6-12, S5.6-07 and D-derive-03 (events, checks, rules and derived channels
on every file of a tab through its time map), S5.3-06 (incremental live checks), D-ingest-09's last part (payload plugins
in browser-only mode), D-live-11 (Reconnect: needs the core's reconnect first), S5.4-06 (canvas traces: an owner decision
first), D-gauge-04 (after R6's measurement), D-ui-10 and everything else in R3.4 (the quiet-interface pass and the
whole-app apple-design audit). If one of your rows turns out to need one of these, stop it and say so.

## What is decided, and what you must not decide

- **Owner decisions are the owner's.** Where a row says "owner decision", the design says "Open", or two readings of the
  design are both reasonable, stop the row. Write the question with a one-paragraph recommendation and its trade-off.
  Decisions so far: memory `owner-decisions-nov14.md` and `owner-decisions-nov16.md`, and `docs/follow-ups.md`.
- **Never propose cutting scope.** Post-v1 items go to `docs/post-v1.md`, never into v1 without the owner.
- **"App is better" rows are never ported back.** Read `docs/release/parity-classes.md` before changing a parity scene.
- The design (`docs/design.md`) is the source of truth; a behaviour change needs a note in `docs/follow-ups.md`.

## Rules

- **Done means, for each row:** built, a test for it (and the tests README's table), the ledger row moved with evidence and
  `python3 ci/ledger_check.py` passing with its counts line updated, `docs/follow-ups.md` where behaviour differs from the
  design, and an owner sheet item when only a real window shows it. Small commits, one row or topic each.
- **Push:** the repo is public. Push once, when your rows are done and the whole UI suite (`just app-test`), the prototype
  suite (`just proto-test`) and `cargo nextest run --workspace` pass locally; then watch CI until it is green. A full run
  bills about 545 minutes, so don't push for each row. **Don't change `ci/` or the CI image** (on Nov 16 adding a tool to
  `ci/setup.sh` made the arm64 image build compile it from source for hours; the run had to be cancelled).
- **Never edit Rust sources while a cargo build or test run is going in the background**: twice on Nov 16 a run compiled a
  half-edited tree and its result was void. Do UI or docs work meanwhile, or wait.
- Headless tests don't see the real window: anything only a real window shows is an Owner pass row with what to look at.
- The Mac has 8 GB of memory and a small disk. Never run two cargo builds or two UI suites at once. Check `df -h /` before a
  build; `rm -rf target/debug` and `cargo clean --manifest-path python/Cargo.toml` are fine (the owner OK'd it).
  `CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0` keeps a workspace run near 8 GB. Run cargo workspace-wide.
- Build order: `just viewer` before the CLI or the app. `cargo tauri build --bundles app` (in `app/src-tauri`) embeds
  `app/ui/dist` while it compiles: don't touch `app/ui/src` during it.
- UI tests: `./run.sh t_name` in `app/ui/tests`. `t_viewer`, `t_viewerctl`, `t_exportreal` need `just viewer` and the release
  CLI (`cargo build --release -p tlmbench`); `t_browserplugin` needs `just plugin-template`. Tests reach the page through
  hooks on `window` (`__tlmApp`, `__tlmFake`); add to a hook rather than reaching into top-level code. Known flakes under
  full-suite load (pass alone; say so if they fail): `t_browser`, `t_descriptor`.
- In zsh, `$var:x` is a modifier: write `"${var}:path"` when a colon follows a variable (on Nov 16 `$base:ci/...` emptied
  two files).
- The owner's settings: work on a copy of `~/Library/Application Support/io.github.rafyarvelo.tlmbench/settings.json`.
- Shell: macOS `sed -i ''`; never `git stash`; add files by name; don't rewrite history. End commits with the attribution
  line your session's instructions give.
- Replies to the owner: concise, lead with what changed and what's left. Never propose cutting scope.

## Report when you stop

Stop when every row on your list is Done or stopped on a question. Then:
1. Update the status table in `docs/release/release-plan.md` (R2.5, R3.2, R3.3 rows) and push as above.
2. Rewrite this prompt for the Opus run: the Opus list above, plus every row you stopped, each with its question.
3. Tell the owner, in this order: what changed (rows Done), the ledger counts, the questions waiting on them (each with
   your recommendation), the rows left for Opus, and the CI result. Then stop.
