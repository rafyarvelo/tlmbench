You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to v1.0.0.
One agent works at a time: start from a clean `main`, leave a clean `main`. If another Claude session is running in this
repository, agree with it which files each of you touches before you edit.

This prompt is only a hand-off. **Where each step stands is the status table in `docs/release/release-plan.md`; what is
done is `docs/release/ledger.md`.** If this prompt and those disagree, they win, and you fix this prompt.

## First

1. Read `CLAUDE.md` (its hard requirements bind you), then `docs/release/release-plan.md` ("One plan, one status", the
   status table and the step you are on), the ledger's rows for that step, and `docs/release/owner-test-sheet.md`.
2. Skim the memory files in `/Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/` (`release-plan.md`,
   `disk-space.md`, `headless-tests-miss-real-window.md`, `viewer-architecture.md`, `r22-browser-only.md`).

## Your step

1. **Don't push docs-only commits on their own** (owner, Nov 15). `main` is about 17 commits ahead of `origin/main`
   (R3.1 and R3.2 code and tests among them): **ask the owner to push first thing**, then watch CI (`gh run watch`) until
   it is green with no rerun. A full run takes about 1.5 hours and a push cancels the one in progress, so don't push again
   until it ends unless a fix is needed.
2. **Triage the owner test sheet:** new Results and Notes (the new section T, then R, S and the older items) become ledger
   rows or fixes. The owner confirmed on Nov 15 and removed: the ⋯ menu, the time range, Explorer's quiet rows, Open with
   layout ▸, the start page, rates and filters.
3. **R2.2, browser-only mode, the rest** (release plan, status table): descriptors and packets, WASM decoder plugins,
   read-only folders, leaving out the post-v1 controls (D-brow-05, -08, -09, -10), then the end-of-R2.2 UI audit. The
   release plan gives the order. Several files, derived channels, events and checks in a browser tab are post-v1 (owner,
   Nov 15). Never touch an "App is better" row.
4. **Then R2.3 to R2.5** in the plan's order. R2.5 holds the core work the R3.2 rows are waiting on: the import dialog's
   sections and Derived list (S5.1-10, D-share-01), Connect's reconnect (D-live-11), the profile's starter layout and recipes
   (D-ui-42's layout), recipes reaching every file in a tab (D-derive-03) and every file's events (S5.6-12). **After R2.5,
   R3.3** (panes, charts, tables: D-ui-43, -46, -47, -48, -49 and the table's Event column) **and R3.4.** Sonnet suits R3.x;
   run the apple-design audit at the end of each sub-step (the R3.1/R3.2 audit was run by Sonnet, so an Opus pass is owed).

Built and committed, not pushed, Nov 15: R3.1 (D-ui-37, -39, -40, -41, -44, S4.4-04) and R3.2 (D-ui-36, -38, -42, -45,
D-derive-02; D-ui-01 and D-ui-10 Partial). The release app was rebuilt for the owner from that code; its leftover-slot-numbers
fix is on the sheet as T1.

## Open with the owner

- The re-test of the owner test sheet: section T (T1 to T5), R12, B4, S1 to S4 and the older items. When the owner confirms
  an item, remove it.
- Real-window looks no test can give (ledger and `ui-audits.md`): the ⋯ height at 900 px, the focus ring after a click in
  WebKit, Open with layout ▸ reading as a submenu, the start page narrow.

## Rules

- **Done means:** ledger rows moved with evidence you ran (a test by name, a file, or a command with its date),
  `python3 ci/ledger_check.py` passing with the counts line updated, the status table updated, and, when code or tests
  changed, ask the owner to push. After the push, watch CI (`gh run watch`) and fix what fails until it is green with no
  rerun. **Push only when CI has something to prove:** a docs-only commit waits for the next push of code or tests (owner,
  Nov 15); a full run takes about 1.5 hours and a push cancels the run in progress. Ledger IDs are unique:
  check the highest in use before adding one.
- A test for every behaviour change, the tests README's table, and a note in `docs/follow-ups.md` when behaviour differs
  from the design. New ideas go to `docs/post-v1.md`, never into v1 without the owner.
- Headless tests don't see the real window: anything only a real window shows is an Owner pass row with what to look at.
  Try the release app yourself before calling a step done (`screencapture -x` of the running window works for a look).
- The Mac has 8 GB of memory and a small disk. Never run two cargo builds or two UI suites at once; prefer workspace-wide
  cargo. A full `cargo nextest run --workspace` needs about 20 GB: check `df -h /`, `rm -rf target/debug` when short, and
  `CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0` keeps it near 8 GB. `cargo clean --manifest-path python/Cargo.toml` after
  a Python build. The owner has OK'd deleting caches and builds.
- Build order: `just viewer` before the CLI or the app, or they embed a stale viewer. `cargo tauri build` embeds
  `app/ui/dist` while it compiles: don't touch `app/ui/src` during it. Long builds go to the background; wait on a file or
  a process, not a sleep.
- UI tests: `node tests/t_x.js` or `tests/run.sh` from `app/ui`. `t_viewer` and `t_viewerctl` need `just viewer` and the
  release CLI; the real-core tests drive `target/release/tlmbench-app --daemon` and skip without it (`STRICT_SKIP=1` fails a
  skip). Screenshots for the README: `node tests/shots.js` after `npm run build`.
- The owner's settings: work on a copy of `~/Library/Application Support/io.github.rafyarvelo.tlmbench/settings.json`.
- Shell: macOS `sed -i ''`; never `git stash`; add files by name; don't push or rewrite history without the owner's
  go-ahead. End commits with the attribution line your session's instructions give.
- Replies to the owner: concise, lead with what changed and what's left. Never propose cutting scope.
