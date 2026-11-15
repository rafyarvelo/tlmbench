You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to the v1
release. Step R2.1a (the owner's first test-sheet list) is done, and so are the owner's nine Nov 14 decisions, including
export tiers (7) and the menu bar (8). R2.2 (browser-only mode) is partly built. The owner re-tested on the build from
`18d127d`; everything they confirmed is off the test sheet, and **what is left there is your list**. **Your job: fix R12
(below), make B4 testable, then finish R2.2.** Nothing is pushed: pushing is the owner's call. One agent works at a time:
start from a clean `main`, leave a clean `main`. If another Claude session is running in this repository, agree with it which
files each of you touches, and when each runs cargo or a UI suite, before you edit.

## First, in this order

1. Read `CLAUDE.md`. Its hard requirements bind you: the app works from its cache alone on a read-only folder; nothing is
   written next to data except by Share and exports; exports are static files; everything in the design is v1 and you never
   propose cutting scope; the app never runs Python or other programs.
2. Read `docs/release/owner-test-sheet.md` (only open items now, in the owner's words), `docs/release/release-plan.md`,
   `docs/release/ledger.md` and `docs/release/ui-audits.md`. A feature is done only when its ledger row is Done with evidence
   `ci/ledger_check.py` can check.
3. Skim `docs/exports.md`, `docs/browser-only.md`, `docs/follow-ups.md` and the memory files in
   `/Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/` (`owner-decisions-nov14.md`,
   `viewer-architecture.md`, `disk-space.md`, `headless-tests-miss-real-window.md`, `release-plan.md`).

## Where things stand (Nov 15)

- Decisions 1 to 9 are built: ledger rows D-ui-22 to D-ui-32. Settings now has one reset, Reset app…, with "Things the app
  remembers" unticked to start (D-ui-33, sheet R3, confirmed by the owner).
- R2.2 so far: a CSV parses in a Worker, a reload brings files back, there's a static-host recipe, and there are no folder or
  live offers in a tab (D-brow-01 to D-brow-04). S5.10-04 is Partial: no `.tlm`, Parquet, packets, descriptors, plugins,
  derived channels, events, checks or several files in a tab.
- Not yet tested by the owner: S1 to S4 (the Nov 15 work), and B6, C2, C3, D1, D2, F4 to F10, G1 to G3, H1 to H5.

## 1. R12 failed: the exported view is half read-only (owner, Nov 15)

The owner's words (sheet R12): the exported view "isn't really 'read-only' since some of the panes 'duplicate' actions work,
but the dragging options don't work so it's awkward. … is it possible to just allow exported views to have the user be able
to move and drag channels like the app. Having the exported view start 'locked' and having the user click unlock is probably
a good idea. There doesn't have to be 100% parity in the browser version, but clean it up so it's consistent at least … Also
the Events tab shows the rules buttons but they don't work and there's no visual indication to the user as to why, either
hide them or disable and put some UI indication as to why."

What that asks for (the direction is the owner's; take it as decided):

- **Locked by default, with Unlock.** The viewer opens locked, as today's "Read-only view". One click unlocks it. Unlocked, a
  viewer can rearrange the view as in the app: drag channels between panes (with the copy key), reorder, split, duplicate,
  close and resize panes, and switch a pane's view. Lock puts it back. The header pill says which state the view is in. It
  keeps the export's provenance (file, date, version) in its tooltip.
- **Static files stay static.** Nothing is written into the export. If an unlocked arrangement is kept across reloads, it
  goes only in that browser's storage, wrapped in try/catch, and the view still works without it. Whether it is kept, and
  whether there is a "Back to the exported view" action, is a small open question: recommend one answer and ask the owner
  before you build it.
- **Every control either works or says why it can't.** Go through every control the viewer shows: pane headers and their
  ⋯, right-click menus, the rail and its panels (Events, Rules and checks, Layouts, Settings), the top bar, and the
  keyboard shortcuts. Each one should act, or be hidden, or be disabled with a short `data-tip` saying why ("Rules run in the
  app; an export shows their results"). The Events panel's rule buttons are the one the owner named. Write a test that lists
  every visible button in the viewer, locked and unlocked, and fails on any that does nothing without being disabled.
- **The same rule in browser-only mode.** `ui-audits.md` (Nov 15) left an open question: the rail's Live, Rules and checks,
  and Layouts icons there open panels that can't act yet. The owner's rule above answers it: what can't act is hidden, or
  disabled with the reason. Apply it as you go, and close that item in `ui-audits.md`.

Code: `app/ui/viewer.html`, `app/ui/src/viewer.ts` and `viewer-main.ts`, `crates/web` (the core in WASM), and the
`ui.viewer` checks across `app/ui/src` (`ui.svelte.ts` sets it). Tests: `t_viewer`, `t_browseronly` and a new one for the control sweep. Ledger: a new D-ui row for lock and
unlock, and another for "every control acts or says why". The next free ID is D-ui-34. Also update the viewer's S5.10 rows.
Run the apple-design audit on the viewer, locked and unlocked, light and dark, into `ui-audits.md`. Owe a design-doc note:
the viewer is no longer read-only, so decision 6's header changes.

## 2. B4: make "a file that is still building" visible

The owner wrote: "The file opened pretty much instantly, so I couldn't really tell…. Not verified, need a better test case."
Find out why. Clear cache may keep the open file's pyramid, a 100M-sample file may build too fast on this Mac, or the step
order may be wrong. Then give B4 steps that reliably show the hatching, the chart filling in, and the min/max band, for
example a bigger `just testdata` file or a way to drop one file's pyramid. Try them yourself in the release window, take a
screenshot, and rewrite B4 on the sheet. Ledger X-win-06.

## 3. Then the rest of R2.2 (browser-only mode), as `release-plan.md` says

Parquet, `.tlm`, packets, descriptors, plugins, derived channels, events, checks, several files in a tab, and folders where
the browser allows them (the prototype opens folders). Close S5.10-04. Run the apple-design audit at the end of R2.2 into
`ui-audits.md`.

## Hand the owner, when you report

- **A rebuilt release app** (`just viewer`, then `cd app/src-tauri && cargo tauri build --no-bundle`; about 12 minutes). Put
  its commit in the sheet's "App: release build from commit" line.
- **A re-test list.** R12 and B4 as you changed them, and new S items for what R2.2 adds, written like section S (what
  changed, Do, Expect, `Result:`, `Notes:`). Keep the sheet to open items only. When the owner confirms an item, remove it
  and the original items it covers.
- **Design-doc notes owed** (list them; don't edit the Claude Doc):
  - Earlier: BLAKE3 checksums (D-cli-08); checks as their own pass (S5.3-04); the check TOML (D-check-08); CSV `#` comments
    (D-ingest-08); redacted checks leave the export (D-cli-10); group routing in `crates/web`.
  - Nov 14: ⌘Z is undo (Backspace goes back a view); the copy key (Option or ⌘ on the Mac, Ctrl elsewhere); the default
    line width and style; per-pane Events (E is the default); the top bar's Export menu; pane actions on hover, giving way by
    width; Chart and values' grip at the pane's bottom (the prototype has it under the plot); "Show only these" as an
    Explorer header icon (the prototype has a pill in the Open strip); the folder watcher with Refresh; the export closes on
    start with progress under the top bar.
  - Nov 15: export tiers (Folder or One file, raw samples when they fit, the size limit field, `--single-limit`); the viewer
    draws the app's min/max band; the native menu bar from one command table; browser-only mode so far; one reset (Reset
    app… with Things the app remembers; Reset to defaults is gone); the viewer's lock and unlock (this step).

## Rules

- A row moves to Done only with evidence you ran in your session (a test by name, a file, or a command with its date). Run
  `python3 ci/ledger_check.py` before every commit. Ledger IDs are unique: check the highest one in use before you add a row.
- A test for every behaviour change, the tests README's table, and a design-doc note when behaviour differs from the doc.
- Headless tests don't see the real window. Anything only a real window shows is an Owner pass row with what to look at.
- The Mac has 8 GB of memory and a small disk. Never run two cargo builds or two UI suites at once. Prefer workspace-wide
  cargo. A full `cargo nextest run --workspace` debug build needs about 20 GB, so check `df -h /` first and
  `rm -rf target/debug` when short (the owner has OK'd deleting caches and builds). Release-mode tests reuse release
  artifacts. Run `cargo clean --manifest-path python/Cargo.toml` after a Python build.
- Build order: `just viewer` before the CLI or the app, or they embed a stale viewer. `cargo tauri build` runs
  `npm run build` and embeds `app/ui/dist` while it compiles the app crate: don't edit `app/ui/src` or rebuild the UI during
  that step.
- UI tests: `npx svelte-check --fail-on-warnings && npm run build` from `app/ui`, then `node tests/t_x.js` for one test or
  `tests/run.sh` for all. **Check that the build succeeded**: svelte-check warnings stop it, and the tests then run the old
  `dist`. Run `t_perf_gauges` alone at the end (`t_loading` is the prototype's, not the app's). `STRICT_SKIP=1 ./run.sh` turns
  a skipped test into a failure.
- Real-core tests (`t_exportreal`, `t_sharero`, `t_watchreal`, `t_resetreal`) drive `target/release/tlmbench-app
  --daemon` and skip without it. `t_viewer` needs `just viewer` and the release CLI.
- If you need the owner's settings, work on a copy of
  `~/Library/Application Support/io.github.rafyarvelo.tlmbench/settings.json`; never change the original.
- Shell: write scripts with the Write tool and run them; macOS `sed -i ''`; never `git stash`; add files by name; don't push
  or rewrite history unless the owner asks. End commits with the attribution line your session's instructions give.
- Concise replies to the owner: lead with what changed and what's left, no recaps. Never propose cutting scope.
