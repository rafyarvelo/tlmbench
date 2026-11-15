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

**R2.2's remaining items** (release plan, R2.2, in the order it gives: descriptors and packets, WASM decoder plugins,
folders, leaving out the post-v1 controls; then the end-of-R2.2 UI audit). The plan's "Recommended model per step" says
which model suits each item. R3.0, the parity classification, is done (`docs/release/parity-classes.md`): parity work
later (R3.1 to R3.3) ports only its Port rows and never touches an "App is better" row.

## Open with the owner

- Approve the "App is better" rows B10 to B16 and the ⋯ menu trim (P3) in `parity-classes.md`.

- The re-test on the owner test sheet (R12, B4, S1 to S4 and the older items). When the owner confirms an item, remove it.
- Sheet R12's question: keep an unlocked viewer arrangement across reloads? Recommendation: no. Build nothing until answered.
- Release decisions 1 to 10 in the release plan (licence, platforms, signing…), needed by R4.

## Rules

- **Done means:** ledger rows moved with evidence you ran (a test by name, a file, or a command with its date),
  `python3 ci/ledger_check.py` passing with the counts line updated, the status table updated, and then ask the owner to
  push. After the push, watch CI (`gh run watch`) and fix what fails until it is green with no rerun. Ledger IDs are unique:
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
