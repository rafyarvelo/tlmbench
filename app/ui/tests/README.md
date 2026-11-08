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
| t_view | panes and the channel list, wheel and box zoom, Fit and Back, the view stopping at the recording's ends, hover readouts, cursors and Measure (tiles, a table in the channels' decimals, RMS, the stats channel picker), settings after a reload |
| t_live | the Connect dialog (a descriptor for sockets, a refused connection), a live tab following now (LIVE, REC time, window presets, no overview), a view you choose holding it, scrubbing back and Jump to live, the Sources panel's stream health, Stop, switching tabs, closing the source with its tab |
| t_open | Open…, ⌘O and drops, a file still being read (progress, Measure waiting for exact stats), the coarse label, the gap factor dialog |
| t_explorer | Open folder (folder → file → channel), select-first rows (a click selects and shows Open; double-click or Enter opens), the checkbox plotting in the selected pane, the filter (text, regex, a bad one marked), Expand all / Collapse all, Close all, the folder after a reload |
| t_playback | the transport: Play back opens it paused, play/pause and Space, the view following at 85% of the window and holding when you choose one, readouts following the playhead, speed, window presets, scrubbing, stopping at the end, the A–B loop, each tab keeping its playhead (paused), close |
| t_panes | the pane ⋯ menu and right-click: close (pane, others, below) with Undo, duplicate, split right into columns and down within one, add below; dragging a pane's number; time labels under each column; the layout after a reload |
| t_shell | the shared look (sidebar 300 px, UI font on buttons, 32 px pane headers, fixed plot height, no plot fill, `--danger`, focus ring), the rail and left sidebar (Ctrl+B), Settings (Basic/All, search, ⓘ, apply and save, Reset to defaults), mouse modes, the ⋯ menu, the status bar, Measure docking by hand and by itself, the note when a file can't be opened |
| t_controls | Lock (button, L, refused wheel/keys/Fit with an Unlock note, cursors still working, saved), ← → pan and + − ↑ ↓ zoom, T theme, the ⋯ menu's groups, Remove every channel / pane with Undo, Clear cursors always on the bar, Focus mode (Ctrl+., Esc, Exit focus) |
| t_sidebars | dragging a sidebar's inner edge (200 px to half the window, double-click resets, saved), collapsing Explorer and Measure's panels, Collapse all in ⋯, ⇄ trading sides, sizes and sides after a reload, Reset UI layout |
| t_channels | Explorer's categories (grouped by name prefix, counts, folding, Chart on a group), channel swatches (check, colour, "pane N"), the selection bar for one or several channels (Add to pane N, Chart, Table, Remove), Ctrl/Cmd-click, Shift-click, Ctrl+A and Esc, dragging channels onto a pane or empty space, the file right-click menu, Show only these, Close all files with Undo |
| t_start | the start page with no file (Open folder…, Open recording…, Connect live source…), recent recordings newest first, the rail's logo opening it over the tabs (Esc closes it), Show this page at launch |
| t_tips | the floating tooltip (after a pause, at once on keyboard focus, hidden when the pointer leaves), Short retiring a tip after three showings (saved), Full, Off, Reset tooltips |
| t_time | the menu on the time range (seven formats) changing the range, readouts and cursor tiles, Time decimals, Time labels on every chart, all saved |
| t_fileopen | Settings → When a file opens and Files opened for the first time (starter charts, tables, values, none; Remember), Forget layouts, Clicking a row (Act at once), Quiet interface |
| t_height | the grip under a chart (drag, side-by-side panes follow, ↓ steps, double-click resets, saved) |
| t_chips | a pane's chips: × to take out, drag to reorder or move (Alt copies), "· L" / "· R", "+N" and its list |
| t_narrow | the top bar under 900 px (Clear, Back, Fit, Play, Lock in ⋯), tabs scrolling with ‹ › |
| t_zones | dropping a pane on another's edges (beside, above, below) or middle (merge), the + between rows, Add pane ▾ kinds, Duplicate beside, the empty states |
| t_overview | the overview: what it shows (a pane or a pinned channel), its range, the Show on top switch, Box, Cursors and wheel gestures |
| t_leftovers | Settings under Live in the rail; the span in words; the wheel setting; Also lock the layout; Backspace; Ctrl+PageUp/Down, Ctrl+Shift+PageUp and Delete on tabs with Undo; the sidebars wrapping under 980 px; "Drag wider to zoom" |
| t_leftovers2 | the view button's icon, the values pane's "where" line and its height, three panes at most in a row, B over a table row, the transport in the main column |
| t_exmenus | "No file open" with a folder, a file's channels folding (chevron, ← →), Explorer's folder and ⋯ menus, files that can't be opened, the filter matching units and categories, Only this, Remove the folder |
| t_colors | channel colours (the least-used, not the channel number) and moving a channel to a category of your own |
| t_menus | the Menu component every menu uses: under its button (above when there's no room), inside the window on a right-click, the first or checked item focused, ↑ ↓ Home End, Esc giving focus back, a press outside closing it (Explorer's menus too), Enter picking |
| t_lines | plot lines drawn as quads cover clearly more pixels than one-pixel lines (forced, since software GL defaults to thin ones) |
| t_modes | a pane's view menu (chart, table, chart and table, values), value tiles at the pointer, else cursor A, else the end of the view, the view saved with the layout and kept by Duplicate |
| t_axes | a pane's units an axis each (left, right, room on the right of every plot); "Axis and scale…" in the ⋯ menu: range (fixed starting from what's shown, include zero, ignore outliers), decimals, label, log scale; kept with the pane and after a reload; Reset axes; none for a table pane |
| t_tables | table panes (Table, Chart and table): a row per sample in view, read a window at a time as it scrolls and moving with the view; a row click placing cursor A (Shift: B), double-click centring the view, Go to A; column widths and the table's height under a chart saved with the layout |

`node shots.js [dir]` writes the README's screenshots (`docs/images/`) from the built UI and the fake core.

`node parity/parity.mjs [scene ...] [--dpr 2] [--light]` (or `just app-parity`) compares the app with the prototype. It
opens both at the same viewport and device scale, puts each in the same scene (a menu open, a dialog, a hover state;
`parity/scenes.js`), and writes `out/parity/index.html`: the two screenshots side by side and the computed styles of
mapped elements that differ. It reports and never fails. The prototype runs offline with the real uPlot and TanStack
table-core (`parity/vendor/`) and the app's IBM Plex files.

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
