You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to v1.0.0.
One agent works at a time: start from a clean `main`, leave a clean `main`. If another Claude session is running in this
repository, agree with it which files each of you touches before you edit.

This prompt is only a hand-off. **Where each step stands is the status table in `docs/release/release-plan.md`; what is
done is `docs/release/ledger.md`.** If this prompt and those disagree, they win, and you fix this prompt.

Written for **Opus** (release plan, "Recommended model per step"). Your job is the judgement work the plan gives to Opus,
and the Sonnet work that is left in the same steps, through **R3.4**. Stop after R3.4 (the UI freezes there). Do not start R4.

## First

1. Read `CLAUDE.md` (its hard requirements bind you), then `docs/release/release-plan.md` (the status table, and the sections
   R2.2 to R3.4), the ledger rows for each step (the query below), and `docs/release/owner-test-sheet.md` (sections T and U).
2. Skim the memory files in `/Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/` (`release-plan.md`,
   `disk-space.md`, `headless-tests-miss-real-window.md`, `viewer-architecture.md`, `r22-browser-only.md`,
   `owner-decisions-nov16.md`).
3. The open rows for this work. Run this from the repository root and treat its output as the list:
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

## Your step

Nov 16 (Opus) finished R2.2 and part of R2.3 and R2.5; the list below is what is left, in order. The ledger query above is the
full list; these are the notes that aren't in the ledger rows.

1. **Push and CI first.** Ask the owner for the push go-ahead (CLAUDE.md), push, and `gh run watch` until CI is green with no
   rerun. Nov 16's first push (`f262664`, run 43880338915) failed once on Linux in `t_live` ("Stop ends the recording"): a test
   race, fixed in the next commit. If `t_browser` times out under full-suite load (seen once locally, passes alone), say so.
2. **R2.3 (left):**
   - **D-ingest-09, the `payload` world (decided Nov 16, `docs/follow-ups.md`):** a third world in `crates/plugin/wit/tlmbench.wit`
     (and the copies under `plugins/*/wit`, which `just plugin-template` diffs); the host for it in `crates/plugin`; a descriptor
     `plugin` key on a `[[packet_types]]` entry (the plugin gets each payload after framing, checksum and decompression and returns
     rows for channels it declares) and on a field with a `size` (many records' bytes at once, one value each). A packet type
     decoded by a plugin isn't fixed-width records, so it needs a decode path into the cache of its own (as `plugin::open_with`
     builds one). A template plugin for the world, and tests with it.
   - **D-ingest-10:** hot reload is done (Nov 16). Left: the templates as a `cargo generate` template, with a test harness.
   - **D-ingest-11** (packets and plugins live, `crates/live`) and **S5.1-06** (filters causal on live data): not started.
3. **R2.5 (the ledger's R2.5 rows):** as the old prompt listed; the owner's v1 ideas joined it on Nov 16: **D-ui-57** (the import
   dialog's choice of how a file opens) and **D-ingest-12** (CSV text columns as labelled channels, ≤256 values, decided Nov 16).
   D-ui-58 (Open with its default app) is built (Owner pass).
4. **R3.3 (the rest)** as the ledger lists, plus **D-ui-59** (a code widget made from the app without the JSON; owner sheet G3).
5. **R3.4**, then the pre-freeze whole-app apple-design audit (both themes, the viewer, browser-only). **D-ui-52** (a sidebar
   panel's first move to the right drew wrong, sheet C2) is not reproduced: the owner is asked for a screenshot on the sheet. After
   R3.4 the UI is frozen.
6. **Each step closes with** the ledger rows moved with evidence, `python3 ci/ledger_check.py` and its counts line, the status
   table, the tests README, `docs/follow-ups.md` where behaviour differs from the design, and the owner sheet.

## What is decided, and what you must not decide

- **Owner decisions are the owner's.** Where the ledger says "owner decision" or the design says "Open", stop and ask. Give a
  one-paragraph recommendation with the trade-off and wait.
- **Never propose cutting scope.** Post-v1 items go to `docs/post-v1.md`, never into v1 without the owner.
- **"App is better" rows are never ported back.** Read `docs/release/parity-classes.md` before changing a parity scene.
- The design (`docs/design.md`) is the source of truth; a behaviour change needs a follow-up note so the design doc can be updated.

## Rules

- **Done means:** the rules in `CLAUDE.md` and the checks above. When code or tests changed, ask the owner to push; after the
  push, watch CI until it is green. **A Linux CI failure on the bindings' debug build (`tlmbench-decode`, no rustc error text)
  has happened once and passed on a rerun.** If it recurs, note it, and propose making the Linux test step's debug build
  lighter; that is a CI change and needs the owner's go-ahead.
- A test for every behaviour change, the tests README's table, and a note in `docs/follow-ups.md` when behaviour differs from the design.
- Headless tests don't see the real window: anything only a real window shows is an Owner pass row with what to look at.
  Try the release app yourself before calling a UI step done (`screencapture -x` of the running window works for a look).
- The Mac has 8 GB of memory and a small disk. Never run two cargo builds or two UI suites at once. Check `df -h /` before a
  build; `cargo clean --manifest-path python/Cargo.toml` clears the bindings' target folder (17 GB last time); `rm -rf target/debug`
  is fine when short. `CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0` keeps a workspace run near 8 GB. The owner has OK'd
  deleting caches and builds.
- A wait loop must not match its own command line (`pgrep -f` on the script's name matched itself last time). Wait on a file or
  on `gh run view` status, with `run_in_background`.
- Build order: `just viewer` before the CLI or the app. `cargo tauri build` embeds `app/ui/dist` while it compiles: don't touch
  `app/ui/src` during it.
- UI tests: `tests/run.sh` from `app/ui` (`./run.sh t_name` for one). `t_viewer` and `t_viewerctl` need `just viewer` and the
  release CLI. `t_descriptor` has been seen to flake under full-suite load (it passes alone); say so if it fails and passes on a rerun.
- The owner's settings: work on a copy of `~/Library/Application Support/io.github.rafyarvelo.tlmbench/settings.json`.
- Shell: macOS `sed -i ''`; never `git stash`; add files by name; don't push or rewrite history without the owner's go-ahead.
  End commits with the attribution line your session's instructions give.
- Replies to the owner: concise, lead with what changed and what's left. Never propose cutting scope.

## Open with the owner (first thing)

- The push go-ahead.
- The re-test of the owner sheet: section V (the Nov 16 build), C2's screenshot, C3, F10, then T, U, R12, B4 and S.
- Real-window looks no test can give (ledger Owner pass rows and `ui-audits.md`).

## Report when you stop (after R3.4)

Say what changed and what is left, in that order: the ledger counts, the status table's new row states, the owner decisions
you are waiting on (each with your recommendation), and the CI result. Say which Opus work was judgement and which was
Sonnet-sized, so the next step can be chosen with the owner. Then stop.

Written Nov 16 after the Opus session that finished R2.2 (descriptors, packets, WASM plugins by jco, the folder's
`.tlmbench/`, the end-of-R2.2 audit), the owner sheet's fixes (F10, C2's tabs, C3, T1's tab name, U2, the menu spacing, Open
with its default app), D-ui-18 and plugin hot reload. Owner decisions of Nov 16 are in the memory file `owner-decisions-nov16.md`
and `docs/follow-ups.md`.
