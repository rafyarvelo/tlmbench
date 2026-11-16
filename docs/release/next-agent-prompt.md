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

1. **Don't push docs-only commits on their own** (owner, Nov 15). `main` up to `fd076bd` is pushed and CI-green (run 43761755149, no rerun); only later docs
   commits are unpushed and go up with your first push of code or tests. After that push, watch CI (`gh run watch`) until
   it is green with no rerun. A full run takes about 1.5 hours and a push cancels the one in progress, so don't push again
   until it ends unless a fix is needed.
2. **Triage the owner test sheet:** new Results and Notes (the new section T, then R, S and the older items) become ledger
   rows or fixes. The owner confirmed on Nov 15 and removed: the ⋯ menu, the time range, Explorer's quiet rows, Open with
   layout ▸, the start page, rates and filters.
3. **This prompt is written for Sonnet** (release plan, "Recommended model per step"). Do, in this order, the work that
   suits Sonnet, and **stop and report at the first design question or at the first step that needs Opus**; never decide
   a design question yourself. **Built Nov 15 by the previous Sonnet agent** (ledger rows and tests named there; pushed or not:
   see the status table): R2.2's folders and hidden controls, time going backwards in every format (S5.10-06), and R3.3's
   UI-only rows D-ui-43, -46, -47, -48, -49, -11, -13, -14, S4.2-09, S5.2-15 (and most of S5.2-13). What is left for Sonnet:
   1. **R3.3's table rows, the rest:** a click on an event tag in the table reveals the event in the Events panel (S5.2-13's
      last gap). The Event column and the dimming are built.
   2. **Drag outlines and edge drops** (D-ui-17, D-ui-18): add a parity scene holding a drag in progress
      (`app/ui/tests/parity/scenes.js`) and match the prototype's outline styles; the owner checks it in R5.
   3. **R2.4 Python:** wheels for each platform (`maturin build`, abi3, manylinux 2_28, macOS universal2 or two wheels,
      Windows x64), a PyPI trusted-publishing job, an sdist, docs generated from the docstrings, and the objects the ledger
      lists as missing (S5.7-07, S5.10-03: derive channels, add events, read `.tlmbench/` objects). Prove a wheel in a clean
      venv here (`python -m maturin build`, `pip install`, `python3 -m pytest python`); the other OSes are proved by CI after
      the push. If the Arrow C interface fights you, stop: Opus.
   After each: the ledger rows (evidence you ran), the tests README, the status table, the owner sheet, and the
   apple-design audit of the area in `ui-audits.md`, saying it was run by Sonnet (the Opus pass is owed).
4. **What needs Opus; don't start it** (judgement across many files, or a design call): R2.2's descriptors and packets
   (D-brow-08) and WASM decoder plugins (D-brow-09) and reading the folder's `.tlmbench/` in the browser (the store's config
   reader works on paths; it needs one over a file listing), then the end-of-R2.2 UI audit; R2.3's D-ingest-09 (a
   descriptor naming a plugin for one field: needs a field-level call in the plugin interface), D-ingest-10, D-ingest-11
   (live packets and plugins), S5.1-06 (causal filters on live data); **R2.5** (the core's Partial rows: the import
   dialog's sections and Derived list S5.1-10, D-share-01, Connect's reconnect D-live-11, the profile's starter layout,
   recipes and events across the files of a tab D-derive-03, S5.6-12) and the core-dependent rest of R3.3; **R3.4** and
   everything from R4 on except R7 and R9 (Sonnet) and R8 (Haiku). When your Sonnet work is done, report what the next Opus
   step is and why, and stop.

Built Nov 15. CI is green on `87322e1` (run 43791981674), but the Linux job failed once and passed on a rerun, with no rustc error text (it looks like the compiler being killed on the runner, and it doesn't reproduce locally). Watch for it recurring; if it does, make the Linux test step's debug build lighter. The Python wheels workflow (run 43791981655) is green on all three OSes.

## Open with the owner

- The re-test of the owner test sheet: sections T (T1 to T5) and U (U1 to U5), R12, B4, S1 to S4 and the older items. When the
  owner confirms an item, remove it.
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
