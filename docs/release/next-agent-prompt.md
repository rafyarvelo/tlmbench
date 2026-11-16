You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to v1.0.0.
One agent works at a time: start from a clean `main`, leave a clean `main`. If another Claude session is running in this
repository, agree with it which files each of you touches before you edit.

This prompt is only a hand-off. **Where each step stands is the status table in `docs/release/release-plan.md`; what is
done is `docs/release/ledger.md`.** If this prompt and those disagree, they win, and you fix this prompt.

Written for **Opus** (release plan, "Recommended model per step"). Your job is the rest of R2.5, then R3.3's rest, D-ui-59
and R3.4 with the pre-freeze whole-app apple-design audit (both themes, the viewer, browser-only). Stop after R3.4 (the UI
freezes there). Do not start R4.

## First

1. Read `CLAUDE.md` (its hard requirements bind you), then `docs/release/release-plan.md` (the status table, and the sections
   R2.5 to R3.4), the ledger rows for each step (the query below), and `docs/release/owner-test-sheet.md`.
2. Skim the memory files in `/Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/` (`release-plan.md`,
   `disk-space.md`, `headless-tests-miss-real-window.md`, `viewer-architecture.md`, `owner-decisions-nov16.md`).
3. The open rows. Run this from the repository root and treat its output as the list:
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

## Where it stands (Nov 16 night)

The Nov 16 night Opus session built, and the owner re-tested V7 and T4 (pass):
- **R3.4's first rows:** D-ui-62 (one Lock, the prototype's padlock, in the app and exports, as the owner chose), D-ui-63
  (chips drag in Safari: nothing in the stack moves during `dragstart`), D-ui-64 (the flights layout's file name).
- **R2.3:** the payload world (D-ingest-09; left: payload plugins in browser-only mode, which refuses such a file today),
  `cargo generate` templates with a `cargo test` harness (D-ingest-10), causal filters live (S5.1-06), live packet streams
  with a session source per packet type from one connection (D-ingest-11).
- **R2.5, started:** D-store-11 (the cache's live folders counted, evicted, swept after a crash), D-check-02 (`missing =
  "fail"`), D-check-05 (`tlmbench check --watch`), D-ingest-12 (CSV text columns as labelled channels).

Left, by step (the ledger query is the list): **R2.5** (18 rows: S4.3-08 packet types as slots of one tab, S5.3-06
incremental live checks and S5.6-12 every file's events and checks are the large ones; D-ui-57, D-gauge-03, D-check-01,
D-derive-01 are small), **R2.3** D-ingest-09 in the page, **R3.2** (6), **R3.3** (12, with D-ui-59), **R3.4** D-ui-10 and
the audit. R2.1's D-cli-06 and D-cli-09 and R3.1's S4.1-19 are older Partial rows; read their notes.

## What is decided, and what you must not decide

- **Owner decisions are the owner's.** Where the ledger says "owner decision" or the design says "Open", stop and ask. Give a
  one-paragraph recommendation with the trade-off and wait. Decisions so far: memory `owner-decisions-nov14.md`,
  `owner-decisions-nov16.md`, and `docs/follow-ups.md`.
- **Never propose cutting scope.** Post-v1 items go to `docs/post-v1.md`, never into v1 without the owner.
- **"App is better" rows are never ported back.** Read `docs/release/parity-classes.md` before changing a parity scene.
- The design (`docs/design.md`) is the source of truth; a behaviour change needs a follow-up note so the design doc can be updated.

## Rules

- **Done means:** the rules in `CLAUDE.md`; ledger rows moved with evidence, `python3 ci/ledger_check.py` and its counts line,
  the status table, the tests README, `docs/follow-ups.md` where behaviour differs from the design, and the owner sheet.
- **Push:** the repo is public again (owner, Nov 16 late). Push once, when development and local testing are done (the
  whole UI suite and `cargo nextest run --workspace`), then watch CI until it is green. A full run bills about 545 minutes.
  A Linux CI failure on the bindings' debug build (`tlmbench-decode`, no rustc error text) has happened once and passed
  on a rerun; if it recurs, note it and propose a lighter debug build (a CI change: the owner's go-ahead).
- **Never edit Rust sources while a cargo build or test run is going in the background**: twice on Nov 16 a run compiled a
  half-edited tree and its result was void. Do UI or docs work meanwhile.
- A test for every behaviour change, the tests README's table, and a note in `docs/follow-ups.md` when behaviour differs.
- Headless tests don't see the real window: anything only a real window shows is an Owner pass row with what to look at.
  Try the release app yourself before calling a UI step done (`screencapture -x` of the running window works for a look).
- The Mac has 8 GB of memory and a small disk. Never run two cargo builds or two UI suites at once. Check `df -h /` before a
  build; `rm -rf target/debug` is fine (the owner OK'd deleting caches and builds). `CARGO_PROFILE_DEV_DEBUG=0
  CARGO_INCREMENTAL=0` keeps a workspace run near 8 GB. Run cargo workspace-wide, not with many `-p` subsets.
- Build order: `just viewer` before the CLI or the app. `cargo tauri build --bundles app` (in `app/src-tauri`) embeds
  `app/ui/dist` while it compiles: don't touch `app/ui/src` during it.
- UI tests: `./run.sh t_name` in `app/ui/tests`; `just app-test` for all. `t_viewer`, `t_viewerctl`, `t_exportreal` need
  `just viewer` and the release CLI; `t_browserplugin` needs `just plugin-template`. Known flakes under full-suite load
  (pass alone; say so if they fail): `t_browser`, `t_descriptor`.
- The owner's settings: work on a copy of `~/Library/Application Support/io.github.rafyarvelo.tlmbench/settings.json`.
- Shell: macOS `sed -i ''`; never `git stash`; add files by name; don't rewrite history. End commits with the attribution
  line your session's instructions give.
- Replies to the owner: concise, lead with what changed and what's left. Never propose cutting scope.

## Open with the owner (first thing)

- The rest of the sheet: V4 to V6 (browser-only), U, R12, B4, S (V7 and T4 passed Nov 16 night).
- Nothing else is waiting on the owner as of Nov 16 night.

## Report when you stop (after R3.4)

Say what changed and what is left, in that order: the ledger counts, the status table's new row states, the owner decisions
you are waiting on (each with your recommendation), and the CI result. Say which Opus work was judgement and which was
Sonnet-sized, so the next step can be chosen with the owner. Then stop.
