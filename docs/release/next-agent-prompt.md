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

Written Nov 16 evening for **Opus 5.5** (recommended: the first items are a Safari-only bug no headless test reproduces, and a
lock redesign across the app and the viewer; R2.3 has the plugin `payload` world, a WIT and host change). The ledger query above is
the full list; these are the notes the ledger rows don't hold. **CI is green on `6bda02c` (run 43908728143, no rerun).** `main` is
one docs commit ahead or more: **don't push until the owner says the repo is public again** (it is private for Nov 16 to save
Actions minutes; a full run bills about 545 minutes, mostly macOS at 10x). When it is, push code and docs together, once.

1. **The owner's open findings (sheet V7 and T4), first:**
   - **D-ui-62, one Lock (owner, Nov 16 evening, superseding the morning's Edit layout button):** keep the prototype's single
     Lock icon, in the app and in exports alike, locking both the time view and moving channels and panes. Read the prototype's
     `#b-lock` and `lock-layout` (`prototype/plotter.html`), propose the exact behaviour to the owner in one paragraph (what
     each click does in the app and the viewer, what L does, what the viewer opens as), then build it. It replaces `#b-vlock`
     (Edit layout) and the app's time-only lock; `t_viewer`, `t_viewerctl`, `t_controls` and the tips "Click Edit layout to …"
     change with it.
   - **D-ui-63, chips don't drag in Safari** (Explorer rows do; Chrome is fine). The Nov 16 `fitChips` change didn't fix it.
     Playwright's WebKit crashes on this Mac, so test in Safari itself (export flight-1 as One file, open it in Safari) or on CI
     (`TLM_BROWSER=webkit` in `t_viewerctl.js`). Candidates: `-webkit-user-drag: element` on `.chip`; the chip is an inline
     span inside a header with `overflow: hidden`; the stack's `pointerdown` (`app.ts`, `stackEl`) selects the pane on press.
     The app's own window uses the pointer-driven drag (`dnd.ts`), so only browsers and exports are affected.
   - **D-ui-64:** `just testdata` doesn't give the flights folder the layout "Altitude and battery" the sheet's T4 asks for
     (`tools/testdata/make.py`; shared views live in `.tlmbench/views/`).
2. **R2.3 (left):** D-ingest-09, the `payload` world (decided Nov 16, `docs/follow-ups.md`: a packet type's payload or a field's
   bytes to a plugin; a plugin-decoded packet type needs a decode path into the cache of its own, as `plugin::open_with`);
   D-ingest-10's `cargo generate` template and harness (hot reload is done); D-ingest-11 (packets and plugins live); S5.1-06
   (causal filters live).
3. **R2.5:** the ledger's R2.5 rows, with the owner's v1 ideas D-ui-57 (the import dialog's open-as choice) and D-ingest-12 (CSV
   text columns as labelled channels, ≤256 values, decided Nov 16).
4. **R3.3 (the rest)** and **D-ui-59** (a code widget made without the JSON). Then **R3.4** and the pre-freeze whole-app
   apple-design audit (both themes, the viewer, browser-only). After R3.4 the UI is frozen.
5. **Each step closes with** the ledger rows moved with evidence, `python3 ci/ledger_check.py` and its counts line, the status
   table, the tests README, `docs/follow-ups.md` where behaviour differs from the design, and the owner sheet. Run the whole UI
   suite before every push: a menu or label change breaks tests that list items exactly (`t_channels` did on Nov 16).

Known flakes under full-suite load (pass alone; say so if they fail): `t_browser`, `t_descriptor`. Browser-only tests need
`just viewer` first; `t_browserplugin` needs `just plugin-template` (or the Rust tests, which build the plugins).

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

- Whether the repo is public again (then the push go-ahead for what `main` holds).
- D-ui-62's behaviour, before building it (above).
- The rest of the sheet: V4 to V6 (browser-only), V7 once D-ui-62 and D-ui-63 are done, T4 once D-ui-64 is, then U, R12, B4, S.

## Report when you stop (after R3.4)

Say what changed and what is left, in that order: the ledger counts, the status table's new row states, the owner decisions
you are waiting on (each with your recommendation), and the CI result. Say which Opus work was judgement and which was
Sonnet-sized, so the next step can be chosen with the owner. Then stop.

Written Nov 16 evening. The Nov 16 Opus session finished R2.2 (descriptors and packets, WASM plugins by jco, the folder's
`.tlmbench/`, the end-of-R2.2 audit), fixed the owner sheet's findings (F10, C2, C3, V1, V2, export names), D-ui-18, plugin hot
reload and Open with its default app; CI green on `6bda02c`. Owner decisions of Nov 16: the memory file
`owner-decisions-nov16.md` and `docs/follow-ups.md`.
