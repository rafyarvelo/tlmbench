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
   `disk-space.md`, `headless-tests-miss-real-window.md`, `viewer-architecture.md`, `r22-browser-only.md`).
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

1. **Push first, once you have something to prove.** `main` is one docs commit ahead of `origin/main` (`ef4ab5d`) and CI is
   green on `87322e1` (run 43791981674, the Linux job passed on one rerun; see the status table). Your first push of code or
   tests carries the docs with it. Ask the owner for the push go-ahead (CLAUDE.md: no push without it), then `gh run watch`
   until CI is green with no rerun. A full run takes about 1.5 hours and a push cancels the one in progress.
2. **Triage the owner test sheet** (sections T and U, then R, S and the older items). Each new Result or Note becomes a
   ledger row or a fix. The owner removed what they confirmed on Nov 15: the ⋯ menu, the time range, Explorer's quiet rows,
   Open with layout ▸, the start page, rates and filters. Section U is for the Nov 15 browser and R3.3 work.
3. **Finish the Sonnet leftovers first** (they are small; do them before the design work):
   - **D-ui-18, drag outlines (Diverged):** add a parity scene in `app/ui/tests/parity/scenes.js` that holds a drag in progress
     on both pages (synthetic drag events), compare the outlines of a dragged channel (Place on the left / right) and of the
     gaps (Drop here for a new pane) with the prototype's CSS, and match them. Then move the row to Done. The owner checks the
     real window in R5.
   - **S5.2-13 and S4.2-09:** the Event column, the tint and the dimming are built. The row stays Partial only for the
     "charts follow these filters" part (S5.2-16) if that is not built; check the row's own note and the owner sheet.
4. **R2.2 (finish):**
   - **D-brow-08, descriptors and packets in the browser (Missing):** split the pure part of `crates/decode/src/packets.rs`
     (framing, checksums, payload codecs) the way `parquet_core.rs` was split, so the browser's WASM core can include it; the
     zstd shim gains a streaming decoder; descriptors are kept in the browser and found by file name pattern; the descriptor form
     is offered on a binary file nothing claims. Pure Rust codecs only (no C compiler in the WASM build). Prove it with the
     Rust tests of the split part, and with `t_browseronly`-style UI tests for a packet file.
   - **D-brow-09, WASM decoder plugins in the browser (Missing):** run plugins in the browser's own WebAssembly engine with the
     app's sandbox limits (no file or network access, the same memory and time limits). Where a native plugin would be offered,
     the tab says it needs the app. This is the hardest item in the plan: a design call is likely (how the WIT interface maps
     to a browser component). Stop and report the design question to the owner before building if it is not settled by
     `docs/design.md`.
   - **D-brow-10, the folder's `.tlmbench/` (Partial):** read it in the browser for shared configuration. The store's config
     reader works on paths (`crates/store/src/config.rs`); it needs a reader over a file listing. Nothing is written to the folder.
   - **End-of-R2.2 UI audit:** browser-only light and dark, each new screen, recorded in `ui-audits.md`.
5. **R2.3 (finish):**
   - **D-ingest-09:** a descriptor can name a plugin for one field or packet type (a design call; the WIT interface and the
     descriptor's `plugin` key). Settle it against `docs/design.md` (Decoder plugins) and record the decision as a follow-up.
   - **D-ingest-10:** hot reload on rescan and the template's test harness (`plugins/template-decoder`).
   - **D-ingest-11:** the same decoder drives a file and a live stream, plugins included (`crates/live`).
   - **S5.1-06:** filters causal on live data (a one-pass form that carries state between batches).
6. **R2.5 (the core's Partial rows, the rest of the ledger's R2.5 list):** the import dialog's sections and the Derived list
   (S5.1-10, D-share-01, S4.3-11), Connect's reconnect (D-live-11, already Done for TCP and UDP; check the row), the profile's
   starter layout (D-ui-42's layout), recipes and events across every file of a tab (D-derive-01, -03, S5.6-07, S5.6-12, D-ui-12),
   Save as rule from a table filter (S5.3-14), the check's incremental run (S5.3-06), the widgets' inputs (S5.4-10), the
   gauges' setting and items (D-gauge-03, -06), check options (D-check-01, -02, -05), the store's cache cleanup (D-store-11),
   the configuration offers (D-cfg-04, -07), and the Python checks read from a shared folder (S5.7-07 is Done; check its neighbours).
   These are listed in the query output. Each is a ledger row moved with evidence.
7. **R3.3 (the rest, the core-dependent or judgement rows):** the import-profiles tab (S5.1-12), the check report that is saved
   and reopened (S5.3-15), the gauges' update rate (S5.4-05) and the trace-heavy widgets decision (S5.4-06, owner decision 4 in
   `review.md`), the check's flashing and folded lines (D-check-03, -04), the gauges' measurement item (D-gauge-04, after R6's
   measurement; mark it so), and the Time channel and prefix rule (S5.6-10, D-share-01 with R2.5). Take an owner decision where
   the ledger says one is open; never decide one yourself.
8. **R3.4 (what the audit can't see, the pre-freeze audit):** keyboard coverage against the prototype's list; every tooltip and
   empty state (`data-tip`: short, retire after three uses); copy review (plain, short, sentence case; the owner's tokens:
   font sizes 11, 11.5, 12, 13, 15, 26 px; radii 4, 8, 12, round); the design's motion rules and reduced motion; the quiet UI
   (controls appear on selection, hover or focus). Then the whole-app apple-design audit (both themes, the viewer, browser-only),
   recorded in `ui-audits.md`. **After R3.4 the UI is frozen:** only bug fixes touch `app/ui/src` from then on.
9. **Each step closes with:** the ledger rows moved with evidence you ran (a test by name, a file, or a command with its date),
   `python3 ci/ledger_check.py` passing with the counts line updated, the status table updated, the tests README's table for every
   behaviour change, a `docs/follow-ups.md` note where behaviour differs from the design, and the owner sheet updated.
   Ledger IDs are unique: check the highest in use before adding one.

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

## Open with the owner (first thing, before design work)

- The push go-ahead for `ef4ab5d` and the code since `fd076bd`.
- The re-test of the owner test sheet: sections T (T1 to T5) and U (U1 to U5), R12, B4, S1 to S4 and the older items. When
  the owner confirms an item, remove it.
- Real-window looks no test can give (ledger and `ui-audits.md`): the ⋯ height at 900 px, the focus ring after a click in
  WebKit, Open with layout ▸ reading as a submenu, the start page narrow, the browser's folder on Safari and Firefox, and
  whether the Events panel should leave the browser tab's rail.

## Report when you stop (after R3.4)

Say what changed and what is left, in that order: the ledger counts, the status table's new row states, the owner decisions
you are waiting on (each with your recommendation), and the CI result. Say which Opus work was judgement and which was
Sonnet-sized, so the next step can be chosen with the owner. Then stop.

Built Nov 15. CI is green on `87322e1` (run 43791981674); the Linux job failed once and passed on a rerun with no rustc error
text. The Python wheels workflow (run 43791981655) is green on all three OSes. Python: the bindings have derive, events, the
`.tlmbench/` objects, and Arrow through the C stream interface; `docs/python-api.md` is generated and checked in CI.
