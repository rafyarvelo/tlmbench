# Roadmap to v1: the app

Nov 7, 2026. How the app gets from where it is to v1: everything `docs/design.md` promises, looking and behaving like
the prototype (`prototype/plotter.html`), and faster than it. `docs/feature-matrix.md` lists every feature with its
state in the design, the prototype and the app, and names the step below that closes each gap. This roadmap replaces
the ordered list in `docs/follow-ups.md` ("Phase 4 (in progress)"); the design's Phases 4 and 5 and their gates still
stand, and the steps here are how they get done.

## How the work runs

- **One agent at a time.** The development Mac (M1, 8 GB, about 30 GB free) can't hold two Rust builds and two
  Playwright runs at once: a second worktree needs its own `target/` (several GB) and the suites compete for memory.
  Each step is one agent's body of work, from a clean `main` to a clean `main`.
- **The prototype is the reference for look and behaviour; the design for what must be true.** When they disagree,
  the design wins and the difference is noted. Anything the app does differently on purpose goes in the
  accepted-differences list below, with the owner's OK.
- **Core first.** A feature with a data side (expressions, rules, checks, sort and filter, XY pairing, FFT) lands in
  the Rust core and the CLI first, tested headlessly, then gets its UI. The UI never computes over samples.
- **Parity is measured, not eyeballed.** Every step adds its scenes to `app/ui/tests/parity/scenes.js` and checks them
  with `just app-parity` (side-by-side screenshots and computed styles). New icons go through `parity/icons.mjs`.
- **Faster than the prototype.** Each step that adds a heavy view states a budget and measures it (see the budgets in
  each step). Benchmark tracking across commits is part of step 4.1.
- **Tests and docs with every change** (CLAUDE.md): a test per behaviour, the tests README table, the parity audit's
  rows and counts, the feature matrix's rows, and a note when the design doc needs changing.
- **Decisions belong to the owner.** A step lists the decisions it needs; the agent asks before building on them,
  with a short recommendation.

## The steps

Sizes are rough: S a day or two of agent work, M a few days, L a week or more, XL several weeks.

### Phase 4: formats and polish (Gate 4)

#### 4.1 Shell parity close-out (M)

The look and the shell, so that the app reads as the prototype before the big features land.

- Structural parity from the harness: Explorer's title row (Categories toggle; New derived channel waits for 5.1) and
  the folder button with its name and menu (recordings ▾); the top bar's Remove every channel and Remove every pane
  buttons; pane heights fixed as the prototype's, with the height grip; window title.
- The stack scrolls when panes don't fit, with the GL canvas scissored to the stack's rectangle (today plots shrink
  to 60 px, then draw over Add pane and the overview).
- Y zoom (Shift + scroll, drag on the y-axis); markers past ~6 px per sample; Measure's Δy and frequency.
- The Connect dialog laid out as the prototype's (kinds on the left, the form on the right, its sizes and copy). Its
  new fields (name, record switch, decode preview, saved connections) are 5.5.
- Dialogs rise over a scrim as the prototype's; Settings' Basic set and missing rows; a cache section in Settings
  (path, size, Open folder, Clear cache).
- Benchmark tracking across commits (`docs/follow-ups.md`).
- **Owner decisions (Nov 7), all built here:**
  - The overview draws each channel as a min/max band with a midline, as the prototype.
  - One session per folder (tabs, open files, time range, cursors), restored when the folder opens and listed under
    Recent sessions on the start page.
  - The Samples and Wall clock time formats, in the time-format menu and Settings (Wall clock only for files with an
    absolute start time).
  - Sidebar panels drag by their title to reorder or move to the other sidebar; saved in settings, restored by Reset
    UI layout.
  - Explorer selects several files (Cmd- and Shift-click) to Open (each in its own tab) or Close together; several
    files in one tab waits for 5.6.
  - The status bar shows the core's memory (and the chunk store's share) and CPU load, about once a second.
- **Core work:** Draw every sample (a request flag that skips reduction up to a budget); memory and load figures.
- **Done when** the harness's `main`, menu, hover and dialog scenes show no differences outside the accepted list, the
  suite passes, and the Connect dialog scene matches the prototype's layout.

#### 4.2 Tables complete (L)

- **Core:** sort (several columns) and filter over the whole column in the core, with progress, returning a row-id list
  the table pages through; filter syntax `> 1.5`, `1..2`, `blank`, and/or; In view and Whole file; match runs joined
  under 0.25 s.
- **UI:** filter row and strip (▲▼ steps, Apply to charts with hatching and a chip, marks on the charts), Clear sort
  and filters, Columns list, Autofit, header ▾ menu, pinned columns, column selection, copy as CSV, row hover and
  right-click, the cursor line, slower channels held and dimmed, the status line, keyboard; the rows on screen as a
  band on the charts; the lock's "table sort and filter still work" setting.
- **Gaps setting (owner, Nov 8):** a global switch in Settings, "Gaps in lines": break the line at a gap (the default,
  as now) or connect across it with a straight line. It applies to every gap the charts show: time jumps, missing
  values, and the rows Apply to charts leaves out. Connected stretches stay marked (a dashed segment) so a connected
  gap is never mistaken for data. The core takes it as a flag on the view request (`connect_gaps`), so it also works
  in `tlmbench query view` and the exports.
- **Top bar:** all panes as Charts, Tables or Both.
- **Budget:** a sort of 10M rows under 2 s on the M1 with progress; scrolling stays at 60 fps.
- **Done when** `t_tables` covers each item, the prototype's table scenes match in the harness, and the CLI can sort
  and filter (`tlmbench query` or a new `rows` command) for headless tests.

#### 4.3 Import and formats in the app (L)

- **CSV import dialog** on a first open (from Explorer, Open recording…, a drop on the window): time column and unit,
  rate (as recorded, resample, assume a rate, sample numbers), every column's name, unit, type (number, text, skip)
  and category; several headers grouped in one dialog; where the files go (own tabs, one tab, this tab, Explorer
  only). **Import profiles** keyed by the header, kept in the app cache (their recipes come in 5.1).
- **Core:** profiles applied at decode (types, skipped columns, time column and unit, rate), and the cache key includes
  them.
- **Descriptor form** (accepted Nov 7): the prototype's form on an unclaimed binary file, with the preview, Find
  frames and the TOML; descriptors found by their `files = [...]` patterns in the app cache and `.tlmbench/decoders/`
  (the CLI finds them the same way); packets without a length field; bitfields; counter time bases with wraparound.
- Packet types in the app: one recording per type (`flight.bin · gps`) in the same tab.
- Import a folder (Import all in the folder menu); the data rate on imported files.
- **Done when** every Gate 4 format opens in the app through its dialog or descriptor, `t_descriptor` and the import
  tests from the prototype are ported, and the harness's import and descriptor scenes match.

#### 4.4 Export, layouts and Gate 4 (M)

- **Export** (accepted Nov 7): the ⋯ menu's Export group; Export data… (CSV, Parquet, Arrow IPC, HDF5, MAT v7.3;
  the time view, between cursors or the whole recording; this tab's panes or every channel; wide or long; as
  recorded or resampled) through the `convert` writers; Save the view as an image (PNG, SVG).
- The Layouts sidebar; the `.tlmbench` entry at the top of Explorer; the tab menu's header, save status and "This tab
  shows"; starter layouts as the prototype's; session restore per folder (if decided in 4.1).
- `just gate4`: the format matrix (one recording in every format, identical per-channel checksums and views;
  `convert` round trips) and the 10 GB zstd packet file (cached reopen, whole view under 1 s).
- **Done when** Gate 4 passes and its result is written into the design's Roadmap section.

### Phase 5: the workbench (Gate 5, the v1 release)

#### 5.1 Expressions and derived channels (XL)

- **Core:** a new crate (`tlmbench-expr`): the parser (names bare or quoted, `a|b` alternatives, labels in single
  quotes), type checks with errors that name the character, and chunked evaluation at the fastest input's rate
  holding the slower ones, carrying lookback across chunks. Function families: math, sample to sample, rolling (in
  samples or time), one-number, filters (moving average, Butterworth low/high/band-pass zero-phase on files, causal
  live; median). Derived channels get a pyramid and are cached by file and recipe hash. FFT for the Spectrum view.
- **Recipes** (one library), applying to this file, a profile, or any file with the channels; profiles pick recipes.
- **UI:** Derive… (right-click, the title row's ƒ button, the selection bar), A − B / A ÷ B / A × B / A + B with two
  selected, autocomplete, ƒ rows, Edit recipe… and Remove…, the Recipes and profiles manager, the Spectrum pane view.
- **CLI:** derived channels in `info`, `query` and `convert`.
- **Budget** (restated Nov 9): over a 100M-sample input, an element-wise derived channel evaluates at pyramid
  speed, and a stateful one (running totals, rolling windows, filters, median) within about 2× of it. `median`
  is fixed first; the plan is in `docs/follow-ups.md`.
- **Done when** every function in the design's table has a core test against a reference, the prototype's derived
  tests (`t_derived`) are ported, and the matrix's section 10 is all ✓.

#### 5.2 Channel display, limits and events (L)

- **Channel display and limits**, by channel name in settings: alias, colour, decimals, unit label, scale and offset
  (in the pyramid key), line width, dashed, steps; warn and alarm limits with defaults; value labels (enums). The
  channel editor (Display and Limits) from a chip or a right-click; Settings › Channels.
- **Missing values** (core): raw values, a valid range, Missing when (an expression, after 5.1); ⌀ counts; Measure's
  Missing count and duration.
- Limit lines and shading on charts; tiles coloured by limits.
- **Events:** the store (per recording in the app cache), markers and ranges from the cursors, the Events panel
  (filters, chips, grouping, inline edit), categories in Settings, markers on charts, ticks on the overview, the
  table's Event column and tinted rows, the top bar's E toggle and each pane's, Stop at in the transport.
- **Done when** the prototype's events and limits tests are ported and their harness scenes match.

#### 5.3 Rules and checks (XL)

- **Core:** the rule and check engine on the 5.1 expressions: Expect, When, grace, settle, summary functions (`pct`,
  `duration`, `occurrences`, `since`, min, max, mean, first, last), severity, roll-up; run in the same pass as the
  pyramid on first open and in idle time after; results cached by file contents and set version; incremental on live
  data. Check failed events; rule events regenerated when data, rules or limits change.
- **CLI:** `tlmbench check <folder> --set <name>` with JUnit, CSV, JSON and HTML reports.
- **UI:** the Rules & checks rail panel and its one editor (Lists and ƒ, live preview on the file in front), badges
  and dots (Explorer, tabs, chips), ▲▼ through violations, Mark when… and Add check…, + From limits, the Checks pane
  view, Save as rule from a table filter, folder run and the Report tab with trends.
- **Done when** `t_checks`, `t_checksel` and `t_rules` from the prototype are ported, a folder check of 20 files
  runs from the CLI, and its results match the app's.

#### 5.4 Pane views and widgets (XL)

- Chart and values; the Gauges view with every built-in widget (gauge, ring, bar, tank, sparkline, state light,
  compass, horizon, number, commanded vs actual, bar group, flags, state timeline, XY dot), pairs and groups by
  name, sizes S/M/L/2×2, min/max marks, the widget ⋯ menu.
- **Core:** one call per frame returning every widget's value, min/max in view and state-run starts.
- **Typed updates** (build and update per widget, text at most 10 times a second); traces on canvas.
- **XY plot:** core pairing by time onto a pixel grid; X channel picker, time colouring, 1:1 with the latitude
  correction, Alt-drag back to time, local zoom.
- Declarative SVG widgets (sanitised, live preview) and code widgets (sandboxed iframe, manifest, trust prompt).
- Min/max envelope fill on charts.
- **Budget:** 100 widgets in under 2 ms a frame (the prototype manages 76 in about 7 ms).
- **Done when** `t_gauges`, `t_perf_gauges` and the XY tests are ported and the budget is measured in the release app.

#### 5.5 Live complete (M)

- The Connect dialog's name, record switch, a preview of decoded frames, saved connections on the start page.
- The file playback source (demo and testing without hardware); + Add source; the live strip's session name and
  PAUSED / STOPPED; the Sources panel's cards with health and controls; the scrubber's faint overview trace.
- Trigger (level or edge on any channel, derived ones too; pre and post time; single or repeat; an event each time).
- Rules and checks running live (after 5.3); the "dropping samples" warning with counts.
- Settings: Record live sessions to.
- **Done when** the prototype's live tests are ported and `just gate3 app` still passes.

#### 5.6 Several files and alignment (L)

- Several files in one tab with numbered slots (`accel_x·2`) and short names; Make primary; Channel prefix…; the +
  menu's needs and slot picker.
- **Core:** time maps (knots, the inverse at query time); Align starts, Same clock, Manual, Sync points, Event
  matching, Cross-correlation, time-source channels. Cross-file derived inputs (`interp`).
- **UI:** the Alignment section (residuals, quality, preview, nudge, lock); Open together; Compare runs.
- **Done when** the prototype's multi-file tests are ported and two files with a known drift align to within a
  sample.

#### 5.7 Shared configuration (M)

- `.tlmbench/` for every object the design lists (events, checks, categories, recipes, profiles, decoders, widgets,
  icons, time maps) in the formats `examples/recordings/.tlmbench/` shows; the Share dialog; read-only folders offer
  Export instead; `tlmbench config export`; the nearest `.tlmbench/` or `--config` for the app, CLI and Python.

#### 5.8 Multiple windows (L)

- The core as a daemon with clients over a local socket (the same binary protocol); per-window state; shared files,
  live sessions and settings; drag a tab out to a new window, Move to new window; Link time view; sessions as a list
  of windows.

#### 5.9 Plugins (L)

- WASM decoder plugins (wasmtime, the WIT interface, a template crate), native plugins, WASM alignment estimators;
  Settings lists them with tier and version.

#### 5.10 HTML export, Python, browser mode and Gate 5 (XL)

- Export to HTML (Full and Light, the 250 MB budget, the viewer bundle) from the app and `tlmbench export html`; the
  checks summary in exports.
- Python bindings over the same core and `.tlmbench/` config.
- The browser-only mode (core in WASM, its cache in the browser's private storage).
- The Canvas 2D fallback renderer; time going backwards flagged in the index.
- `just gate5`: a folder check plus an HTML export of a 2 h flight within budget, opening in under 2 s from a static
  host. Passing it is the v1 release.

## Accepted differences from the prototype

Things the app does differently on purpose. Only the owner adds to this list.

- (Accepted Nov 7) The tab strip's ‹ › scroll buttons when tabs overflow (the prototype just scrolls).
- (Accepted Nov 7) The view menu's checked row is highlighted across its full width (the prototype's highlight hugs
  the text, which looks unintended).
- (Accepted Nov 7) Arrow keys, Home and End move between menu items (the prototype has none).
- Thin one-pixel lines on software GL (headless tests only), since quads draw slowly there.

## Status

| Step | State |
| --- | --- |
| 4.1 Shell parity close-out | done Nov 8; the owner's three decisions are built or settled (Open folder reveals only, local `just bench-record`/`bench-compare`, the building hatch kept), and the parity harness's remaining differences are in `docs/follow-ups.md` |
| 4.2 Tables complete | done except Save as rule (step 5), slower channels (4.3) and the 60 fps scroll measurement on the Mac. Built: the core's sort and filter (`tlmbench-core::table`, `tlmbench query rows`, 10M rows sort in 0.4–1.1 s) and in the app the header sort, the filter row, Clear sort and filters, the filter strip (In view / Whole file, ▲▼ through matches, Apply to charts with its chip) and saving them with the layout (`t_tablequery`, `t_tablefilter`); the Gaps in lines setting (core `connect_gaps`, `query view --connect-gaps`, dashed stretches, `t_gaps`; wire format v3 carries the dashed list), the core's mask of excluded samples makes them missing in views, stats and values (`tlmbench-core::mask`); the match marks and the hatching along the time axis (`table_marks`) are drawn; the rows band is drawn; the Columns list, Autofit, the header ▾ menu, Sample and Time pinned and column selection are built (`t_tablecols`); copy as CSV is built (up to 100k rows); left are Save as rule (waits for Event rules), row hover and right-click, the cursor line, slower channels, keyboard, the lock setting and the top-bar view switch |
| 4.3 Import and formats in the app | done Nov 8. Import profiles and the import dialog (core, app, `tlmbench inspect`, `--profile`); descriptors found by `files` patterns; the descriptor form on a binary file nothing claims (the core's preview of the first records, Find frames, TOML, Save to the app cache, Share to the folder, `t_descriptor`); bitfields (`bits = { shift, width }`); counter time bases with wraparound (fixed-width and packet files); packet types in the app as a tab per type (owner, Nov 8) until step 5.6; Import all; the data rate chip; the parity `import` and `descriptor` scenes. Left for later: slower channels held in tables (needs one tab with several rates, 5.6) |
| 4.4 Export, layouts and Gate 4 | done Nov 8. Export data… (the core's `convert` writers, from the ⋯ menu), Save the view as an image… (PNG, SVG), the Layouts sidebar, Explorer's `.tlmbench` row, the tab menu's header and status, sessions per folder (done in 4.1), `just gate4` (the format matrix in the CLI and the release app; the 10.3 GB zstd packet file reopens in 0.04 s from its cached pyramid) with the result in the design. HTML export is 5.10. Phase 4 is done |
| 5.1 Expressions and derived channels | done Nov 9, apart from what is listed in `docs/follow-ups.md`. Core and CLI: `tlmbench-expr` (parser, chunked evaluator for every function in the design's table with a test each, zero-phase filters with a read margin, one-number functions as passes, recipes in TOML that can use recipes, the FFT); `Store::open_derived` (a cache entry per recording and recipe set, parallel chunks for element-wise recipes) and `Store::join` (the recording's channels then the derived ones, the two pyramids side by side, nothing rebuilt); `tlmbench derive` and `--derived`, `--recipes`, `--expr` in `info`, `query` and `convert`; `just derived-bench`. App: Derive… (selection bar, the title row's ƒ, right-click) with templates, A − B and the like, the live check naming the character, a preview, autocomplete, scopes (this file, any file with the channels); ƒ rows; Edit recipe… and Remove from this file or every file with Undo; the Recipes manager (automatic on or off, duplicate, delete, TOML export and import); the Spectrum pane view (`t_derived`, `t_recipes`, `t_spectrum`). Budget: met for element-wise recipes (0.7 s for 100M samples against a 0.8 s pyramid build), not for stateful ones (1.0–1.8 s) |
| 5.2 Channel display, limits and events | under way (Nov 9). Built in the app: the channel editor (a chip click or Display… / Value labels… in the right-click menu) with Display (name shown, colour, decimals, unit label, line width, dashed, steps), Limits and Value labels tabs, all kept in settings by channel name; limit lines and shading on charts and tiles coloured by level; Settings › Channels with a reset per channel and for all; labels and limits reach expressions (`Store::set_expr_env`, in the derived cache key; `state == 'CRUISE'` autocompletes); `t_display`, `t_limits`, `t_labels`. Left: scale and offset and missing values in the core (see `docs/follow-ups.md`), the ⌀ badge and Measure's Missing count, then events (store, panel, markers, ticks, table column, E toggle, Stop at) |
| 5.3 – 5.10 | not started |

An agent finishing a step updates this table, the feature matrix's rows, the parity audit and the tests README, and
says what's left in `docs/follow-ups.md` if anything moved out of the step.
