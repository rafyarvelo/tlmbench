# The app's UI tests

The built UI (`app/ui/dist`) in headless Chromium, with `fakecore.js` answering the Tauri commands the UI calls
(`launch`, `open`, `close`, `view`, `values_at`, `stats`, `rows`, `shared_views`, `share_view`, `export_view`, `list_folder`, `live_connect`, `live_state`, `live_stop`, the file
and folder dialogs and drag and drop) from synthetic
recordings. Tests check behaviour and state, not pixels; check visual changes with a screenshot.

`just app-test` builds the UI and runs every `t_*` here, three at a time; `just app-test t_view` runs one. Each prints
PASS or FAIL with its failed checks; full output goes to `out/<test>.txt` and a failing test's screenshot to
`out/fail_<name>.png`. Playwright and its Chromium come from `prototype/tests` (`just setup playwright`).

| Test | Covers |
| --- | --- |
| t_shared | shared views in the + menu (one that can't be read greyed), opening one (its file, layout and time range; read-only: Revert, Save as… makes your copy), Share with this folder… (file by path in the folder, time range, who), Export file… when the folder can't be written, a file outside the folder refused, the shared tab after a reload |
| t_workspaces | Save as… in a tab's menu (a name needed, unique; the tab then shows the layout, saved by channel name), unsaved changes marked without touching the file's layout, Save layout, Revert changes, ⌘S; the + menu (an empty layout, your layouts, opening one on the file in front with its missing channels named, Delete with Undo); a file tab sharing a file a workspace tab shows, the file kept open while a tab shows it; workspace tabs and their order after a reload |
| t_wire | the wire format: the fake core encodes the core's golden batch byte for byte, and `wire.ts` decodes it (Node, no browser) |
| t_tabs | a tab per open file, each keeping its panes, view and cursors; reordering by drag; middle-click, × and the tab menu (close, close others, close all) closing tabs and their files; the tabs after a reload |
| t_view | panes and the channel list, wheel and box zoom, Fit and Back, hover readouts, cursors and Measure, settings after a reload |
| t_live | the Connect dialog (a descriptor for sockets, a refused connection), a live tab following now (LIVE, REC time, window presets, no overview), a view you choose holding it, scrubbing back and Jump to live, the Sources panel's stream health, Stop, switching tabs, closing the source with its tab |
| t_open | Open…, ⌘O and drops, a file still being read (progress, Measure waiting for exact stats), the coarse label, the gap factor dialog |
| t_explorer | Open folder (folder → file → channel), select-first rows (a click selects and shows Open; double-click or Enter opens), the checkbox plotting in the selected pane, the filter (text, regex, a bad one marked), Expand all / Collapse all, Close all, the folder after a reload |
| t_playback | the transport: Play back opens it paused, play/pause and Space, the view following at 85% of the window and holding when you choose one, readouts following the playhead, speed, window presets, scrubbing, stopping at the end, the A–B loop, each tab keeping its playhead (paused), close |
| t_panes | the pane ⋯ menu and right-click: close (pane, others, below) with Undo, duplicate, split right into columns and down within one, add below; dragging a pane's number; time labels under each column; the layout after a reload |
| t_shell | the rail and left sidebar (Ctrl+B), Settings (Basic/All, search, ⓘ, apply and save, Reset to defaults), mouse modes, the ⋯ menu, the status bar, Measure docking by hand and by itself, the note when a file can't be opened |
| t_modes | a pane's view menu (chart, table, chart and table, values), value tiles at the pointer, else cursor A, else the end of the view, the view saved with the layout and kept by Duplicate |
| t_axes | a pane's units an axis each (left, right, room on the right of every plot); "Axis and scale…" in the ⋯ menu: range (fixed starting from what's shown, include zero, ignore outliers), decimals, label, log scale; kept with the pane and after a reload; Reset axes; none for a table pane |
| t_tables | table panes (Table, Chart and table): a row per sample in view, read a window at a time as it scrolls and moving with the view; a row click placing cursor A (Shift: B), double-click centring the view, Go to A; column widths and the table's height under a chart saved with the layout |

Each test uses `lib.js`: `run(name, async h => { ... }, opts)`. `h.check(label, condition, got)` records a result;
a test fails on any failed check, any error in the page, or an exception in the script. Helpers: `text`, `visible`,
`panes`, `paneCount`, `plotAt(pane, fraction)`, `until`, `settingsLS`, `fake(fn, arg)` (runs in the page, where
`window.__tlmFake` drives the fake core: `pick` and `pickFolder` are what the dialogs return, and `list_folder` lists
the `files` under a folder; `sources` are live endpoints whose recordings grow with the clock). Options: `files` (path → recording spec: `channels`, `units`, `samples`,
`rate`, `built: false` for a file still being read, `cold: true` for slow exact views), `launch` (the file opened at
start; null for none), `settings` (the first-launch settings), `viewport`, `colorScheme`.

The page's state is read through `window.__tlmApp` (`app.ts`): the view, cursors, hover time, build progress,
panes (their channels and `layout`: row, beside or under) and what each one has drawn.

The core's side of the wire format is pinned by `golden_batch` in `crates/core/src/wire.rs`; after a deliberate
format change, `TLMBENCH_BLESS=1 cargo test -p tlmbench-core golden_batch` rewrites the fixture, and `fakecore.js`
must follow.
