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

1. **Don't push docs-only commits on their own** (owner, Nov 15). `main` is a few docs commits ahead of `origin/main`;
   they go up with your first push of code or tests. R0 is done (two clean runs Nov 15).
2. **Triage the owner test sheet:** new Results and Notes become ledger rows or fixes.
3. **R3.1, then R3.2** (release plan, R3; moved before R2.2 by the owner on Nov 15, so their re-tests meet the improved
app). The lists are the Port rows of `docs/release/parity-classes.md` and the owner's Nov 15 decisions listed in the
plan's R3 ("The owner's Nov 15 decisions, by sub-step"), each a ledger row. Never touch an "App is better" row. Sonnet
suits these; run the apple-design audit on each sub-step's area at its end (Opus if available). Two R3.2 items need core
work from R2.5 first (the profile's starter layout and recipes, Connect's reconnect): build around them and leave the
ledger row Partial with what is left. Then R2.2's remaining items, in the order the plan gives.

(The owner's Nov 15 report about Open together and Compare was confusing UI, not a bug: ledger D-ui-45 and D-ui-46.)

## Open with the owner

- The re-test on the owner test sheet (R12, B4, S1 to S4 and the older items). When the owner confirms an item, remove it.

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
