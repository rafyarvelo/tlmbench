# Feature matrix: design, prototype and app

Nov 7, 2026, on `main` at 523a185; the App column updated Nov 8 after step 4.1. Every v1 feature in `docs/design.md`, and every feature the prototype or the app has,
compared across the three. `docs/roadmap.md` turns the gaps into steps; the last column names the step that closes
each one. `docs/parity-audit.md` holds the finer, control-by-control comparison of the app's UI against the prototype.

**Design** says what v1 promises: **v1** (an Accepted or Modified decision), **Prop.** (still Proposed) or **—** (not in
the design). **Prototype** and **App**: ✓ done, ◐ partly, ✗ missing, **sim** simulated in the prototype (its data is
synthesized in the page, so there is no real file I/O), **n/a** doesn't apply. A ✓ in the App column means it works as
in the prototype and the design, not just that something exists.

Totals over the 204 rows below: the app has 108 ✓, 29 ◐ and 67 ✗. The prototype has 158 ✓, 6 ◐ and 12 ✗, and 27 rows
are simulated or don't apply to it. Its gaps are the parts only real software can have (multiple windows, plugins,
alignment estimators, the daemon) plus a few details (copy as CSV, bitfields).

## 1. Data, formats and the core

| Feature | Design | Prototype | App | Step |
| --- | --- | --- | --- | --- |
| CSV: delimiter, units, time column detection, text columns skipped | v1 | sim | ✓ | |
| CSV columnar cache, instant reopen | v1 | n/a | ✓ | |
| CSV files with `#` comment lines | Prop. | ✗ | ✓ (CLI and core) | |
| Parquet input | v1 | n/a | ✓ (core; the app opens it) | |
| `.tlm` reference format (descriptor in the header) | v1 | sim | ✓ | |
| Fixed-width binary with a separate descriptor | v1 | sim | ✓ (the descriptor form picks or writes one; Save to the app cache, Share to the folder) | |
| Packet files: sync, length, checksum, zstd/lz4/gzip, cached index | v1 | sim | ✓ (opened through the descriptor form, compressed payloads previewed by the core) | |
| Packet types: one recording per type (`flight.bin · gps`) | v1 | ✗ | ◐ (core, CLI and app, a tab per type until several recordings share a tab, 5.6) | 5.6 |
| Packets without a length field | v1 | ✓ | ✓ | |
| Descriptor `files = [...]` name patterns | v1 | ✓ | ◐ (core and CLI find descriptors in `.tlmbench/decoders/` and the app cache's `decoders/`; the app's open path uses it) | 4.3 |
| Bitfields in descriptors | v1 | ✗ | ✓ (core, CLI and the form's Bits column) | |
| Time bases: column, rate, counter, arrival, index | v1 | sim | ✓ (column, rate, counter with wraparound, index; arrival for live) | |
| Time going backwards flagged in the index | v1 | ✗ | ✗ (an error, as decided for now) | 5.10 |
| Min/max pyramid, resumable background build | v1 | n/a | ✓ | |
| M4 views, pixel-exact, gaps from the gap index | v1 | n/a | ✓ | |
| LRU chunk store | v1 | n/a | ✓ | |
| Cache cap and low-disk eviction; Settings shows path, size, Clear | v1 | n/a | ◐ (Settings → Cache: folder, size, Clear cache; Open folder reveals it in the file manager) | 4.1 |
| Scale and offset per channel (before the pyramid) | v1 | ✓ | ◐ (from descriptors only) | 5.2 |
| Missing values per channel (raw values, range, Missing when) | v1 | ✓ | ✓ (applied in the decoders' reads; the pyramid is rebuilt for them) | 5.2 |
| Derived channels computed in the core, cached with a pyramid | v1 | sim | ✓ (`Store::open_derived`, `Store::join`, `tlmbench derive`, the app's ƒ rows) | |
| WASM decoder plugins (wasmtime) | v1 | ✗ | ✗ | 5.9 |
| Native decoder plugins | v1 | ✗ | ✗ | 5.9 |
| Plugins listed in Settings | v1 | ✗ | ✗ | 5.9 |

## 2. Rendering and performance

| Feature | Design | Prototype | App | Step |
| --- | --- | --- | --- | --- |
| WebGL2 traces, one canvas, uniforms for pan and zoom | v1 | n/a (uPlot, Canvas 2D) | ✓ | |
| Lines as quads (1.25 px, same ink as the prototype) | v1 | ✓ | ✓ | |
| Binary IPC (no JSON for samples), stale replies dropped | v1 | n/a | ✓ | |
| Re-projecting the last buffer while the next arrives | v1 | n/a | ✓ | |
| Panes fill in left to right on a first open ("Reading 43%") | v1 | ✓ | ✓ (hatch differs: audit row "Building hatch") | 4.1 |
| "coarse" label after 0.3 s | v1 | n/a | ✓ | |
| Min/max envelope fill under a line | v1 | ✗ | ✓ (a setting, on; a visual check with a spiky channel on the Mac is left) | 5.4 |
| Markers when zoomed past ~6 px per sample | v1 | ✓ | ✓ |  |
| Draw every sample (setting) | v1 | ✓ | ✓ (core: `Viewport::raw_max`; CLI `--raw-max`) |  |
| Canvas 2D fallback renderer | v1 | n/a | ✗ (thin lines on software GL only) | 5.10 |
| Stack that scrolls when panes don't fit (GL scissored to the stack) | — | ✓ | ✓ |  |
| Gates 1–3 (100M points at 60 fps; 10 GB first paint; 4M samples/s live) | v1 | n/a | ✓ | |
| Benchmark tracking across commits | — | n/a | ✓ (`just bench-record` / `bench-compare`, local) | 4.1 |

## 3. Shell

| Feature | Design | Prototype | App | Step |
| --- | --- | --- | --- | --- |
| Rail: Explorer, Measure, Live, Settings | v1 | ✓ | ✓ | |
| Rail: Events, Rules & checks, Layouts | v1 | ✓ | ✓ | 5.3 |
| Tabs: file and workspace tabs, drag to reorder, keys, Undo on close | v1 | ✓ | ✓ | |
| Tab menu: header with name and ×, save status, "This tab shows" | v1 | ✓ | ✓ (one file per tab until 5.6) | 5.6 |
| Tab menu actions: Save layout, Save as, Revert, Share, Export file, Delete, Close… | v1 | ✓ | ✓ | |
| + menu: empty layout, your layouts, shared in this folder | v1 | ✓ | ✓ | |
| + menu: what each layout needs ("needs file 2"), slot picker | v1 | ✓ | ✗ | 5.6 |
| Time range (click to change the format), span in words | v1 | ✓ | ✓ | |
| Mouse modes, Clear, Back, Fit, Play back, Lock | v1 | ✓ | ✓ | |
| All panes as charts, tables or both (top bar) | v1 | ✓ | ✓ built (`t_allmodes`; panes of values keep their view) | 4.2 |
| Event markers toggle (top bar, E) | v1 | ✓ | ✓ | 5.2 |
| Remove every channel / every pane (top bar) | v1 | ✓ | ✓ (icons at 1300 px and wider, ⋯ otherwise) |  |
| ⋯ menu: View, All panes, Window groups | v1 | ✓ | ✓ | |
| ⋯ menu: Export group (data, image, HTML) | v1 | ✓ | ◐ (data and image; HTML waits) | 5.10 |
| Narrow window moves groups into ⋯ | v1 | ✓ | ✓ | |
| Menus: one style, grow from their button, keys, Esc, outside close | v1 | ✓ | ✓ | |
| Tooltips: data-tip, retire after 3 uses, Short / Full / Off | v1 | ✓ | ✓ | |
| IBM Plex fonts, tokens (sizes, radii) | v1 | ✓ | ✓ | |
| Icons with the prototype's drawings and sizes | — | ✓ | ✓ | |
| Motion: glides on a spring, menus grow, dialogs rise, reduced motion | v1 | ✓ | ✓ |  |
| Focus mode (⌘.) | v1 | ✓ | ✓ | |
| View lock with its settings (layout, cursors, table sort) | v1 | ✓ | ✓ built (`t_tablelock`) | 4.2 |
| Status bar: query and draw times, drawing level | v1 | ✓ | ✓ (and the core's memory, the chunk cache's share, CPU load) |  |
| Start page: open folder, open recording, connect, recent recordings | v1 | ✓ | ✓ | |
| Start page: recent sessions (one per folder), saved connections, layouts | v1 | ✓ | ◐ (sessions per folder, saved connections and the recording banner; no layouts) | 5.5 |
| Window title follows the tab | — | ✓ | ✓ |  |
| Drop a file on the window | v1 | ✓ | ◐ (a CSV without a saved profile asks first; several files in one tab wait for 5.6) | 4.3 |
| Settings: Basic and All, ⓘ explanations, Reset, Reset UI layout | v1 | ✓ | ✓ (rows for later steps' features come with them) |  |
| Settings: time formats Samples and Wall clock | v1 | ✓ | ✓ |  |
| Settings: event categories, channel display list, recipes and profiles | v1 | ✓ | ◐ (recipes in Explorer's ⋯ and folder menus; Channels list done; event categories wait) | 5.2 |
| Settings: Record live sessions to | v1 | ✓ | ✓ | |
| Sidebars: collapse, move, resize, swap sides | v1 | ✓ | ✓ | |
| Drag a sidebar panel to reorder it | v1 | ✓ | ✓ (and to the other sidebar) |  |

## 4. Explorer

| Feature | Design | Prototype | App | Step |
| --- | --- | --- | --- | --- |
| Folder tree, files a decoder recognizes, files that can't be opened | v1 | ✓ | ✓ | |
| Title row: add shown channels, Expand all, Collapse all | v1 | ✓ | ✓ | |
| Title row: New derived channel, Categories toggle | v1 | ✓ | ✓ | |
| Folder button with the folder's name and menu (recordings ▾) | v1 | ✓ | ✓ |  |
| Folder menu: Import all, recipes and profiles | v1 | ✓ | ◐ (Import all, Recipes…; profiles wait for the profiles tab) | 5.1 |
| Filter with regex, matching units and categories | v1 | ✓ | ✓ | |
| Open files strip: Show only these, Close all | v1 | ✓ | ✓ | |
| `.tlmbench` entry at the top (shared views count) | v1 | ✓ | ✓ | |
| Categories by name prefix, your own categories, counts | v1 | ✓ | ✓ | |
| Multi-select, selection bar (Add, Chart, Table, Remove, Derive…) | v1 | ✓ | ✓ | |
| Drag channels to panes | v1 | ✓ | ✓ | |
| File right-click: Open, Open only this, Close | v1 | ✓ | ✓ | |
| File right-click: descriptor, Make primary, Compare, channel prefix | v1 | ✓ | ◐ (Format descriptor…; the rest wait for 5.6) | 5.6 |
| Channel right-click: category, gap factor | v1 | ✓ | ✓ | |
| Channel right-click: display and limits, Derive…, Mark when…, Add check… | v1 | ✓ | ✓ | 5.2, 5.3 |
| Badges: ƒ derived, ⌀ missing count, ✓/✗ checks, red dots | v1 | ✓ | ✓ (a ✓ / ! / ✗ badge on tabs and Explorer files, a dot on failing channels in Explorer and on chips, a dot on the rail button) | 5.3 |
| Data rate shown on imported files; "primary" with several open | v1 | ✓ | ◐ (the rate chip on an open CSV's row, a click shows its import settings; primary waits for 5.6) | 4.3, 5.6 |
| Import a local folder (browser snapshot in the prototype) | v1 | ✓ | ✓ (Open folder reads the disk; Import all in the folder menu) | |

## 5. Pane stack and charts

| Feature | Design | Prototype | App | Step |
| --- | --- | --- | --- | --- |
| Panes synced on x; split right (3 a row) and down; drag to move | v1 | ✓ | ✓ | |
| Pane ⋯ menu: axis and scale, close, duplicate, split, add below | v1 | ✓ | ✓ | |
| Pane ⋯ menu: event markers in this pane | v1 | ✓ | ✓ | 5.2 |
| Chips: drag, ×, +N overflow, value on hover | v1 | ✓ | ✓ | |
| Chip click opens the channel editor (Display and Limits) | v1 | ✓ | ◐ (Display tab: name, colour, decimals, unit, line style; Limits, Value labels and Missing wait) | 5.2 |
| View menu: Chart, Table, Chart and table, Values | v1 | ✓ | ✓ | |
| View menu: Chart and values, Gauges, Checks, XY plot, Spectrum | v1 | ✓ | ✓ | 5.3, 5.4 |
| Axis and scale: a unit's axis each, range, include zero, decimals, label, log | v1 | ✓ | ✓ | |
| Ignore outliers (clip and mark) | v1 | ✓ | ✓ | |
| Y zoom: Shift + scroll, drag on the y-axis | v1 | ✓ | ✓ (sets the axis's fixed range; owner, Nov 8) |  |
| Fixed pane heights and a height grip, as the prototype | v1 | ✓ | ✓ |  |
| Limit lines and shading past warn and alarm | v1 | ✓ | ✓ | 5.2 |
| Event markers on charts (washes, notches, tooltips) | v1 | ✓ | ◐ (washes and notches; the pointer readout names the event; no tooltip) | 5.2 |
| Table filter matches on charts; Apply to charts with hatching and a chip | v1 | ✓ | ◐ (Apply to charts and its chip work, the core leaves the rows out; match marks and hatching are drawn from `table_marks`) | 4.2 |
| Table rows band on the charts | v1 | ✓ | ✓ built (`t_tablequery`) | 4.2 |
| Settings → Gaps in lines: break at gaps, or connect across them (dashed) | — (owner, Nov 8; needs a design entry) | ✗ | ✓ built (`t_gaps`; core `connect_gaps`, `query view --connect-gaps`) | 4.2 |
| Hover readout snapping to samples; legend values | v1 | ✓ | ✓ | |
| Overview: picker, range, window, gestures, top or bottom | v1 | ✓ | ✓ | |
| Overview drawing: min/max band per channel with a midline | v1 | ✓ | ✓ |  |
| Overview event ticks | v1 | ✓ | ✓ | 5.2 |
| Values view: tiles at the pointer, cursor A or the end of the view | v1 | ✓ | ✓ | |
| Values tiles coloured by limits | v1 | ✓ | ✓ | 5.2 |

## 6. Tables

| Feature | Design | Prototype | App | Step |
| --- | --- | --- | --- | --- |
| Raw rows in the view, paged from the core, moving with the charts | v1 | ✓ | ✓ | |
| Row click A (Shift B), double-click centres, Go to A/B | v1 | ✓ | ✓ | |
| Column widths and the table's height saved with the layout | v1 | ✓ | ✓ | |
| Slower channels hold, dimmed where unchanged; gaps flagged | v1 | ✓ | ✗ | 4.2 |
| Sort (multi-column) and filter in the core, with progress | v1 | ✓ | ✓ (`tlmbench query rows` too) | 4.2 |
| Filter row (`> 1.5`, `1..2`, `blank`, and/or), filter strip (In view / Whole file, ▲▼) | v1 | ✓ | ◐ (filter row, scope, ▲▼ and Apply to charts done; Save as rule waits for Event rules) | 4.2 |
| Save as rule (from the filter strip) | v1 | ✓ | ✗ | 5.3 |
| Clear sort and filters, Columns list, Autofit, header ▾ menu | v1 | ✓ | ✓ built (`t_tablequery`, `t_tablecols`) | 4.2 |
| Pinned columns, column selection, copy a selection as CSV | v1 | ◐ (no copy) | ✓ built (`t_tablecols`, `t_tablerows`: ⌘C and the row menu copy up to 100k rows as CSV) | 4.2 |
| Row hover, row right-click, cursor line in the table | v1 | ✓ | ✓ built (`t_tablerows`) | 4.2 |
| Status line (rows, rate, how slower channels show) | v1 | ✓ | ◐ (rows and the sample rate; slower channels wait for files with several rates, 4.3) | 4.2 |
| Event column and tinted rows inside events | v1 | ✓ | ◐ (tint, stripe and ▸ ◂ ● tags in the Time cell; a separate Event column and channel-cell tints wait for events that list channels) | 5.2, 5.3 |
| Keyboard (arrows, Page, Home/End) | v1 | ✓ | ✓ built (`t_tablerows`) | 4.2 |

## 7. Cursors, hover and Measure

| Feature | Design | Prototype | App | Step |
| --- | --- | --- | --- | --- |
| Cursors A and B (mode, keys over plots and rows), Esc clears | v1 | ✓ | ✓ | |
| Measure: values at A and B, Δt, 1/Δt, exact stats, channel picker | v1 | ✓ | ✓ | |
| Measure: Missing count and duration | v1 | ✓ | ✓ | 5.2 |
| Measure: Δy and frequency | v1 | ✓ | ✓ |  |

## 8. Time modes: Play and Live

| Feature | Design | Prototype | App | Step |
| --- | --- | --- | --- | --- |
| Transport: play/pause, scrubber, speed, window presets, Follow | v1 | ✓ | ✓ | |
| Loop between A and B; per-tab playhead restored paused | v1 | ✓ | ✓ | |
| Stop at events (any or chosen categories) | v1 | ✓ | ◐ (nothing, any, or one category; several at once wait) | 5.2 |
| Live sources: serial, UDP, TCP, tail a growing file | v1 | sim | ✓ | |
| Live source: file playback (demo, testing without hardware) | v1 | ✓ | ✓ (`replay://`) | |
| Ring buffer ingest, auto-recording to a session folder | v1 | n/a | ✓ | |
| Live tab is a file tab whose files grow; every view works | v1 | ✓ | ✓ | |
| Live strip: badge, REC time, scrubber, presets, Following, Sources | v1 | ✓ | ✓ (session name, PAUSED / STOPPED / ENDED) | |
| Sources panel: a card per source with health, Pause/Resume/Stop/Remove | v1 | ✓ | ✓ (a card per live tab) | |
| Connect dialog: the prototype's two-column layout, kinds, forms | v1 | ✓ | ✓ | |
| Connect dialog: name, record switch, decode preview, save the connection | v1 | ✓ | ✓ | |
| + Add source to a session; several sources on one clock | v1 | ✓ | ◐ (Add source joins the session folder and opens in its own tab; one tab with `·2` channels is 5.6) | 5.6 |
| Trigger (level or edge, pre/post, single or repeat) | v1 | ✓ | ✓ (in the core, on ingest; "Make a rule…" gives the same crossing as a rule) | |
| Session scrubber track with a faint overview trace | v1 | ✓ | ✓ | |
| Rules and checks running live | v1 | ✓ | ✓ (every 2 s over what is recorded) | |
| Backpressure: "dropping samples" warning with counts | v1 | ✓ | ✓ | |

## 9. Files, workspaces, sharing and export

| Feature | Design | Prototype | App | Step |
| --- | --- | --- | --- | --- |
| CSV import dialog (time column, rate, columns, types, categories) | v1 | ✓ | ◐ (built; Panes, prefix and derived channels wait for 5.1 and 5.6, one tab for several files for 5.6) | 4.3 |
| Import profiles keyed by the header (with recipes and a layout) | v1 | ✓ | ◐ (core and app: names, units, types, categories, time, rate, kept in the app cache; recipes and layout wait for 5.1) | 4.3, 5.1 |
| Descriptor form on an unclaimed binary file (preview, Find frames, TOML) | v1 | ✓ | ✓ | |
| Layouts by channel name in app data; Save, Save as, Revert, Delete | v1 | ✓ | ✓ | |
| Shared views in `.tlmbench/views/`; Share with this folder; Export file | v1 | ✓ | ✓ | |
| Share dialog (what gets written, read-only folders) | v1 | ✓ | ✗ | 5.7 |
| Layouts sidebar | v1 | ✓ | ✓ | |
| Built-in starter layouts (what a file opens with) | v1 | ✓ | ✓ (starter charts, charts and tables, one table, values, nothing: the prototype lists no built-in layouts) | |
| Session restore (tabs, files, time range, cursors), per folder | v1 | ✓ | ✓ (a session per folder, Reset session in Settings) | |
| Export data… (CSV, Parquet, Arrow, HDF5, MAT; view, cursors or all) | v1 | ✓ | ✓ (the core's `convert` writers) | |
| Save the view as an image (PNG, SVG) | v1 | ✓ | ✓ (panes scrolled out of the stack aren't drawn) | |
| Export to HTML (static viewer, Full and Light) | v1 | ✓ (simulated) | ✗ | 5.10 |

## 10. Derived channels

| Feature | Design | Prototype | App | Step |
| --- | --- | --- | --- | --- |
| Expression language: arithmetic, comparisons, `if`, quoted names, `a\|b`, labels | v1 | ✓ | ✓ (labels and limits come from settings) | |
| Math, sample-to-sample, rolling, one-number functions | v1 | ✓ | ✓ (a core test per function) | |
| Filters (moving average, Butterworth low/high/band-pass, median) | v1 | ✓ | ◐ (core and CLI; live data gets element-wise and windowed recipes; the causal form of the filters is left, see `docs/follow-ups.md`) | 5.1 |
| Cross-file inputs (`interp`) | v1 | ✓ | ✗ | 5.6 |
| Derive… dialog: checks as you type, preview, where it applies | v1 | ✓ | ✓ (this file, any file with the channels; a profile's waits for the profiles tab) | |
| A − B, A ÷ B, A × B, A + B with two selected | v1 | ✓ | ✓ | |
| Recipes library; Recipes and profiles manager; TOML import/export | v1 | ✓ | ◐ (the library in the app cache, the Recipes manager with TOML export and import; the import-profiles tab and Share to `.tlmbench/recipes/` wait) | 5.1 |
| Spectrum (FFT) pane view | v1 | ✓ | ✓ | |
| Autocomplete for channels, functions and value names | v1 | ✓ | ◐ (channels, recipes, functions; value names wait for 5.2) | 5.2 |

## 11. Events, rules and checks

| Feature | Design | Prototype | App | Step |
| --- | --- | --- | --- | --- |
| Events: name, start, duration, category, source; per recording in the cache | v1 | ✓ | ✗ | 5.2 |
| Events panel: list, add from cursors, inline edit, filters, grouping | v1 | ✓ | ✓ | 5.2 |
| Event categories (Settings; shared as `categories.toml`) | v1 | ✓ | ◐ (Settings: add, rename, recolour, Charts, delete; sharing waits) | 5.2, 5.7 |
| Value labels (enums) on channels | v1 | ✓ | ✓ (tiles, tables, chips and Measure show the text; expressions name them; axis ticks per value wait) | 5.2 |
| Channel limits (warn, alarm) with sensible defaults | v1 | ✓ | ✓ | 5.2 |
| Rules: an expression held for a minimum time, marking events | v1 | ✓ | ✓ (core engine, the editor's Rule rows, rule events in Events) | 5.3 |
| Checks: Expect, When, grace, settle, summary checks, severity | v1 | ✓ | ✓ (`tlmbench_store::checks`; "missing values fail" option not built) | 5.3 |
| Rules & checks panel; one editor with Lists and ƒ | v1 | ✓ | ✓ (the "files with this import profile" attachment is CLI only) | 5.3 |
| Check results: badges, dots, Check failed events, ▲▼ through violations | v1 | ✓ | ✓ | 5.3, 5.4 |
| Checks pane view | v1 | ✓ | ✓ | 5.3 |
| Folder run (files × groups), Report tab with trends | v1 | ◐ (a dialog) | ✓ (Explorer's folder, one level; results as CSV) | 5.3 |
| Check results cached in the app cache | v1 | ✓ | ✓ (`checks-results/`, by file contents, set version, recipes and limits) | 5.3 |

## 12. Pane views and widgets

| Feature | Design | Prototype | App | Step |
| --- | --- | --- | --- | --- |
| Chart and values (tiles under the chart, shared limits) | v1 | ✓ | ✓ | 5.4 |
| Gauges view: one widget per channel, picked from unit and name | v1 | ✓ | ✓ | 5.4 |
| Gauge, ring, bar, tank, sparkline, state light, compass, horizon, number | v1 | ✓ | ✓ | 5.4 |
| Commanded vs actual, bar group, flags, state timeline, XY dot | v1 | ✓ | ◐ (flags light bits named "bit n": names wait for descriptors that name bits in the app) | 5.4 |
| Pairs and groups by name; Separate / Join; sizes S, M, L, 2×2 | v1 | ✓ | ✓ | 5.4 |
| Min/max in view marks | v1 | ✓ | ✓ (from the core's exact statistics, so they wait for the pyramid) | 5.4 |
| Widgets update in place (typed updates, 100 widgets in < 2 ms) | v1 | ✓ (string patching) | ◐ (fields patched in place: 2.2 ms median for 100 in the headless browser; the release app's measurement and the half-per-frame turns past 4 ms are left) | 5.4 |
| Declarative SVG widgets with a live preview and sanitising | v1 | ✓ | ✓ (kept in settings until Share, 5.7) | 5.4 |
| Code widgets (sandboxed iframe, trust prompt) | v1 | ✓ | ◐ (inline `build`/`update` code, a frame served by the `tlmwidget` scheme with a policy of its own, the trust prompt per folder; the folder with `widget.toml` and inputs by role wait for Share, 5.7, and the scheme is untested in the real webviews) | 5.4 |
| XY plot: X channel, time colouring, 1:1, Alt-drag to time, local zoom | v1 | ✓ | ✓ (pairs by sample: channels of a file share their times) | 5.4 |
| Icons for categories and toolbar recipe buttons | v1 | ◐ | ✗ | 5.7 |

## 13. Several files and alignment

| Feature | Design | Prototype | App | Step |
| --- | --- | --- | --- | --- |
| Several files open at once in Explorer | v1 | ✓ | ✓ | |
| Several files in one tab, numbered slots (`accel_x·2`), short names | v1 | ✓ | ✗ (a tab shows one file) | 5.6 |
| Make primary, Channel prefix… | v1 | ✓ | ✗ | 5.6 |
| Align starts, Same clock, Manual offset | v1 | ✓ | ✗ | 5.6 |
| Time maps with knots; Sync points, Event matching | v1 | ✗ | ✗ | 5.6 |
| Cross-correlation, time-source channels (GPS, PPS) | v1 | ◐ | ✗ | 5.6 |
| WASM alignment estimators | v1 | ✗ | ✗ | 5.9 |
| Alignment section: residuals, quality, preview, nudge, lock | v1 | ◐ | ✗ | 5.6 |
| Open together, Compare runs | v1 | ✓ | ✗ | 5.6 |

## 14. Windows, sharing config, CLI and beyond the app

| Feature | Design | Prototype | App | Step |
| --- | --- | --- | --- | --- |
| Multiple windows sharing one core; tabs between windows; Link time view | v1 | ✗ | ✗ | 5.8 |
| Daemon with clients over a socket; browser tabs; per-session token | v1 | ✗ | ✗ | 5.8 |
| `.tlmbench/` for every object (events, checks, categories, recipes, profiles, decoders, widgets, icons, time maps) | v1 | ◐ (views, events, checks, categories) | ◐ (views only) | 5.7 |
| `tlmbench config export`; nearest `.tlmbench/` or `--config` | v1 | n/a | ✗ | 5.7 |
| CLI: info, build, query, record, convert | v1 | n/a | ✓ | |
| CLI: `check` with JUnit, CSV, JSON, HTML reports | v1 | n/a | ✓ (the HTML report's cells don't link to an exported view until 5.10) | 5.3 |
| CLI: `export html` | v1 | n/a | ✗ | 5.10 |
| Python bindings | v1 | n/a | ✗ | 5.10 |
| Browser-only mode (core in WASM) | v1 | n/a | ✗ | 5.10 |
