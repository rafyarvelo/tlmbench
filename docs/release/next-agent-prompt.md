You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to the v1
release. Step R2.1 (exports) is done. Since then, `73af52c..39a3fc3` fixed the first findings from the owner's real-window
pass (the empty-stack button, the table grip, edge drops, the last channel closing its pane, four columns per row, "Drop
here for a new pane"). **The owner has now filled in part of `docs/release/owner-test-sheet.md` (Nov 12 to 14). Your job is
the list below, before R2.2.** It is already triaged and ordered, so don't re-triage it from scratch. Check it against the
sheet, then go.

One agent works at a time: start from a clean `main` (the sheet and this list are in `8c6ef01`) and leave a clean `main`.
Nothing is pushed: pushing is the owner's call.

## First, in this order

1. Read `CLAUDE.md`. Its hard requirements bind you: the app works from its cache alone on a read-only folder; nothing is
   written next to data except by Share and exports; exports are static files; everything in the design is v1 and you never
   propose cutting scope; the app never runs Python or other programs.
2. Read `docs/release/owner-test-sheet.md` in full (the owner's own words win over the summaries here), then
   `docs/release/release-plan.md` and `docs/release/ledger.md`. A feature is done only when its row is Done with evidence
   `ci/ledger_check.py` can check.
3. Skim `docs/release/r2-1-sparse-store.md`, `docs/exports.md`, `docs/follow-ups.md` and the memory files in
   `/Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/` (`viewer-architecture.md`, `disk-space.md`,
   `headless-tests-miss-real-window.md`, `release-plan.md`).
4. **Send the owner the decisions in section 2 as one message**, each with a recommendation, in a single AskUserQuestion
   if you can (4 questions per call, so two calls at most). Then start section 1 straight away: nothing in it waits on an answer.

## 1. Fix now: bugs and gaps with no open decision (in this order)

Order: shared causes first (one fix clears several items), then blockers and hard-requirement areas, then the rest. Reproduce
each one first, with the release build in daemon mode (`target/release/tlmbench-app --daemon --ui app/ui/dist`, as
`app/ui/tests/t_exportreal.js` does) or the real window. Each one gets a test, a small commit citing the sheet ID, and its
ledger row updated in the same commit.

| # | Sheet | What the owner saw | Where to start | Ledger |
| --- | --- | --- | --- | --- |
| 1 | E2, F1 | Rules and checks, Export to HTML (and likely others) open stuck in the top-left corner | Already found: `.dlg` in `app/ui/src/style.css:699` is `position: fixed` with no `top`/`left`. Each dialog centres itself in its own scoped CSS (`ImportDialog.svelte` sets `left: 50%`; `.recdlg` in `DeriveDialog.svelte` is scoped, so `HtmlExportDialog` and `CheckEditor` get nothing). Centre every `.dlg` in one shared rule, as the prototype does, and remove the per-dialog copies. Test: every dialog's box is centred horizontally and inside the viewport (Playwright can measure `getBoundingClientRect`) | S4.1-10 |
| 2 | A6 | Share on a read-only folder: "Couldn't share", no Export offered | `app/ui/src/app.ts:2516` passes `'Export file…'` to `note()`, so find out why the owner didn't see it (another error path? the note's action not shown?). The design (Files, "Read-only folders") says Share… reports that it can't write and offers Export file… *instead*: check whether the folder is writable when the Share dialog opens and offer Export there, not only after a failure. Test with a `chmod a-w` folder, and keep `just ro-test` green | S5.7-04, X-win-09 |
| 3 | Other #1 (blocker) | Table columns can't be dragged onto another pane | The prototype's `.tth[draggable]` (`prototype/plotter.html:865`): a column header drags like a channel chip | add a row (D-ui) |
| 4 | A4 | Dropping a folder from Finder: "Is a directory (os error 21)" | A dropped folder does what Open folder… does. `main.rs:3141` already checks `is_dir` on one path, so follow the drop path | X-win-01 |
| 5 | F1 | The export dialog doesn't close straight away; the progress is weak plain text | Close on start, show progress in the app's own style (a status chip or the top-bar note with a progress bar, not a text line), and end with "Exported to …" with Show in Finder | S5.10-01 |
| 6 | F2 | The "needs a web server" note doesn't give the command | Give the exact command for the real folder (`python3 -m http.server --directory '<folder>'`) with a copy button, and say that "One file" opens anywhere | D-cli-05 |
| 7 | B5 | Chart and values / chart and table: the resize handle is mid-pane under the chart, not at the pane's bottom; XY's header is broken and XY doesn't resize; the gauges' ⋯ menu doesn't match the prototype | Compare each with the prototype's pane (`just app-parity` scenes, then screenshots) | R3.3 rows |
| 8 | E2 | Can't edit the colours of check categories | The design (Events) gives categories a name, colour and shown or hidden, edited in Settings; the feature matrix says "event categories wait" | find the categories row |
| 9 | E1 | Rules and checks is hard to understand | One plain line per rule and check kind, saying what it does, in the editor and its empty state. Copy only: show the owner the wording in the PR summary | R3.4 copy |
| 10 | A2 | "Show only these" appears only once a file is open | The owner asked for it as an always-visible icon in the Explorer header (beside Expand all and Collapse all). That settles where it goes. The empty state is still the open proposal in `docs/follow-ups.md` ("Showing only open files" + Show all): build it too, it doesn't conflict | the Explorer row |
| 11 | Ideas, v1 = yes | No folder watcher or refresh in Explorer | Already in the design: D-ui-02 is Missing. Build the watcher, and a refresh in the Explorer's folder menu for network shares where events don't arrive | D-ui-02 |

Not bugs, but fix the sheet itself while you are here: **B1** says "⌘Z to go back a view", but neither the design nor the
prototype binds ⌘Z to that (Backspace and Previous view are the bindings, and they work); see decision 3. **B4**: the 100M file
opened instantly because its pyramid was already in the cache, so the step should say to Clear cache first (or ship a larger
file in `just testdata big`). **A3**: add how to forget one import profile (`~/Library/Caches/tlmbench/profiles/<key>.json`)
until decision 1 is built. **A6**: say how to make a read-only folder (Get Info › Sharing & Permissions › Read only, or
`chmod -R a-w`).

## 2. Ask the owner (one message, recommendations included), then build what they choose

| # | Sheet | Question | Recommendation |
| --- | --- | --- | --- |
| 1 | A3 | A way to reset everything: Clear cache keeps import profiles, so the import dialog never comes back | Settings › Cache keeps Clear cache, with a tooltip that says it keeps settings and profiles. Add **Reset app…**: a confirm dialog that lists what goes (settings, import profiles, recipes, sets, layouts, your events, widgets), each with a checkbox, all ticked. Pull R3.3's profiles tab forward so one profile can be deleted |
| 2 | C1 | Copy on drag: the owner expects Ctrl or ⌘; the prototype (and its tooltip) uses Alt/Option | Option on the Mac (Finder's convention) and Ctrl on Windows and Linux, with a "+" on the drag cursor. Also accept ⌘ on the Mac, as the owner expects |
| 3 | B1 | ⌘Z | ⌘Z and ⇧⌘Z undo and redo the last change, as on any Mac app (the app already has undo in its notes); Backspace stays "back a view". Note it for the design doc |
| 4 | B3 | A global default line style | Settings › Charts › Default line: width (thin, normal, heavy) and style, used by every channel that has no style of its own |
| 5 | E1 | Events on a single pane | An Events toggle among the pane's hover controls; the global switch sets the default for new panes |
| 6 | F1 | Export is hard to find; the viewer doesn't say it's read-only | A one-click Export button in the top bar (a menu: HTML, data, image), keeping the ⋯ item. The viewer's header says "Read-only view" with the provenance (file, date, app version), which also closes the known provenance-footer Partial |
| 7 | F2, F3 | Export tiers, and how a browser should work (the biggest item; see the note below) | (a) The dialog offers **Folder** (any size, needs a web server or host) and **One file** (opens anywhere), each with its size. (b) One file ships every raw sample when they fit, and Light only when they don't; the 25 MB cap becomes a field with 25 MB as the default. (c) The viewer's coarse drawing matches the app's at the same zoom (the min/max band, not a thinned line); check it with side-by-side screenshots. (d) "Open folders and CSVs in a browser like the prototype" is R2.2, next after this list |
| 8 | Ideas, v1 = yes | The Mac menu bar: File › Open… and every app action | A full native menu bar (File, Edit, View, Pane, Window, Help) built from the app's command table, so the menus and shortcuts can't drift. Add ledger rows; note it for the design doc |
| 9 | Ideas, v1 = yes | Pane actions on hover instead of behind ⋯ | Show as many of the prototype's pane-header actions as fit, in the prototype's order, on hover or focus, and overflow the rest to ⋯ by width. It keeps the calm UI rule (nothing shows until hover) |

**Note for decision 7: explain this to the owner plainly.** The prototype works in a browser because it makes up its own data
in the page. The production equivalent is **browser-only mode** (R2.2): the app's UI over the core in WASM, opening folders
and CSV, Parquet and `.tlm` files from the browser with derived channels, events and checks. An **export** is a different
thing: a static snapshot of chosen data that anyone can open. Both are v1. What the owner expected in F2 is R2.2, which runs
right after this list.

## 3. Then

1. Build the release app (`just viewer`, then `cd app/src-tauri && cargo tauri build --no-bundle`, about 10 minutes) and run
   the UI audit in `release-plan.md` ("UI audits with the apple-design skill", the first pass) on the screens this list
   touched. Fix what it finds that agrees with the prototype and CLAUDE.md's tokens; take any conflict to the owner.
2. Hand the owner a short re-test list: every FAIL above that you fixed, plus the sections they haven't run yet (**B6, C2,
   C3, D1, D2, F4 to F10, G1 to G3, H1 to H5**). Fill in the sheet's "App: release build from commit" line.
3. Report what changed and what is left, then go on to **R2.2** unless the owner says otherwise.

## Still open from R2.1 (unchanged; fold them in when they touch the same code)

- **Owner pass rows from R2.1**: the one-file export opened from a disk path (F3 is done and works), widget cards in a viewer
  (F8), a check report's links (F7), the export dialog's three raw-window fields (F4), a two-file export (F6), and Light and
  trimmed exports drawing labelled coarse (F5).
- **Left Partial:** a default `--detail` per set (D-cli-06); no viewer test drives Play, cursors or Measure (D-cli-09); the
  viewer's footer doesn't show the provenance (decision 6 covers it); a time range for a tab of several files is refused by the
  app's Export; `tlmbench query view` shows no gap breaks until the pyramid is built. All in `docs/follow-ups.md`.
- **Python tests not run** since R2.1 (they need a `maturin` build). Run them when you touch Python or the export manifest.
- **Design-doc notes owed** (list them for the owner; don't edit the Claude Doc): BLAKE3 checksums (D-cli-08); checks as their
  own pass (S5.3-04); the check TOML (D-check-08); CSV `#` comments (D-ingest-08); redacted checks leave the export
  (D-cli-10); group routing in `crates/web`. Add whatever this list's decisions change (⌘Z, the copy modifier, export tiers,
  the menu bar, per-pane events, Reset app).

## Rules

- A row moves to Done only with evidence you ran in your session (a test by name, a file, or a command with its date). Run
  `python3 ci/ledger_check.py` before every commit.
- A test for every behaviour change, the tests README's table, and a design-doc note when behaviour differs from the doc.
- Headless tests don't see the real window. Anything only a real window shows is an Owner pass row with what to look at.
- The Mac has 8 GB of memory and a small disk. Never run two cargo builds or two UI suites at once; workspace-wide cargo only.
  Check `df -h /` before big builds and `rm -rf target/debug` when under 5 GB free. `cargo clean --manifest-path
  python/Cargo.toml` after a Python build. The owner has OK'd deleting caches and builds.
- If you need the owner's settings, work on a copy of
  `~/Library/Application Support/io.github.rafyarvelo.tlmbench/settings.json`; never change the original.
- Build order: `just viewer` before the CLI or the app, or they embed a stale viewer. The viewer is the app's UI: a change to
  shared UI code changes both, so run the app's tests, not only `t_viewer`.
- UI tests run one at a time with `node tests/t_x.js` from `app/ui`, after `npx svelte-check --fail-on-warnings && npm run build`;
  `t_perf_gauges` runs alone at the end. `STRICT_SKIP=1 ./run.sh` turns a skipped test into a failure (the release gate).
- Shell: write scripts with the Write tool and run them; macOS `sed -i ''`; never `git stash`; add files by name; don't push
  or rewrite history unless the owner asks. End commits with the attribution line your session's instructions give.
- Concise replies to the owner: lead with what changed and what's left, no recaps. Never propose cutting scope.
