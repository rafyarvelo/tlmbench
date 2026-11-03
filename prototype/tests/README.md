# Prototype tests

Setup once: `just setup playwright` from the repo root (runs `npm ci` here and installs Playwright's headless Chromium).
The suite runs on macOS and Linux; on a Mac it needs GNU `timeout` (`brew install coreutils`). Ctrl-clicks use the
`ControlOrMeta` modifier, since Chromium on macOS turns Ctrl-click into a right-click.

`./run.sh` runs the suite: every `t_*.js` here (27 tests), three at a time, about 4 minutes in all. Timing tests
(`t_loading`, how fast rows appear; `t_perf_gauges`, how long 100+ widgets take to draw) run alone at the end so
other tests' load can't skew them.
It prints PASS or FAIL per test with the checks that failed, then a summary, and exits 1 on any failure.
`./run.sh t_live t_files` runs just those. Full output of each test is in `out/<test>.txt`.

| Test | Covers |
| --- | --- |
| t_core | panes and undo, views, cursors and Measure, events (+ Event menu) and the Rules button, layouts, limits, XY, lock, session restore |
| t_explorer | select-first rows, multi-select bar, title-row icons, filter, categories, expand/collapse, drag to a pane edge |
| t_files | tabs per file, open strip, another file's channels in expressions, Show only these, Compare, channel prefixes and numbers, Close all + undo |
| t_import | the CSV dialog, time and rate, When it opens, several files, profiles coming back, re-import, odd headers |
| t_folder | Open folder (read-only), import on open, Import all |
| t_descriptor | format descriptors: a binary file with none opens the form on its bytes; Find frames, the preview (hex by field, decoded rows, checksums), problems as the decoder words them, + Add field, the TOML, Share…, Open, and the descriptor claiming the file after a reload |
| t_derived | the expression engine, filters (lowpass, highpass, bandpass, median) and interp(), Derive…, recipes in import profiles, remove + undo, persistence, the Spectrum view |
| t_filters | the filter strip, Whole file, stepping, Apply to charts, Save as rule, missing values and Missing when, Ignore outliers |
| t_tables | sort, column select → chart, hide + Columns (a name scrolls to its column, and the table and list stay put when Columns closes), Autofit columns in view, box zoom and its dead zone, Back, Fit, focus mode |
| t_loading | rows on screen before data, columns filling in, rules waiting for data |
| t_live | live tabs as file tabs: follow/hold, the session scrubber, Explorer, tables, derived, sources, pause, close, reload |
| t_layouts | save as, New tab, delete + undo, shared views, closing tabs |
| t_settings | apply and save, Basic/All, search, time labels, theme |
| t_home | start page, Open folder, session row, Open only this |
| t_events | the Events panel: category chips (hide, only this, All, None; the charts follow them per tab, or not when unlinked), Expand all / Collapse all in the title row, the compact header, rule rows in the editor (group follows category, + Timing) |
| t_gauges | Gauges view: widget per channel, auto type, follows time, type and range menu, your own SVG widgets (sanitised), persistence |
| t_widgets | commanded vs actual, bar group, flags, state timeline, XY dot and GPS track, min/max marks; pairs and groups found by name, ungroup/separate, persistence; widget sizes; a shared code widget behind the trust prompt; dragging widget titles |
| t_rules | rules and checks in one editor: old rules moved to "Event rules", rule rows (lists: magnitude, outside, past the limits), on/off, kind switch, + Rule, rule events, editing a rule event, Mark when… |
| t_checks | check sets: verdicts, roll-up, info groups, value names, derived channels, badges (Explorer, tab, chips), Check events, stepping, the editor (lists, expressions, autocomplete), Add check…, Checks pane and layout, gauges at a violation, folder run, persistence |
| t_perf_gauges | 100+ widget cards: a moving frame inside 16 ms (7–12 here, depending on the machine), nothing rebuilt while values move, a still frame almost free, off-screen cards skipped and drawn on scroll |
| t_playback | transport: play/pause, Space, speed, window, scrub, follow/hold, gauges follow the playhead, stops at the end, A–B loop, Stop at events, each tab keeps its playhead |
| t_trigger | live triggers: arm on a level crossing, the window freezes around it, a Trigger event marks it |
| t_align | time alignment: Time channel (GPS time of week) and Cross-correlation |
| t_share | Share… as TOML (rule sets, events, categories, recipes), Export to HTML (Full or Light by budget), no sessions beside the data |
| t_colsel | table column selection: Shift-click ranges from the header or checkbox, Ctrl-click |
| t_colors | channel colours cycle evenly past eight channels |
| t_checksel | check rows: select and deselect, the tooltip, opening a check's channels |
| t_phone | phone layout: scroll position kept, time labels on every chart, long names on selected rows |

Each test uses `lib.js`: `T.run(name, async h => { ... }, opts)`. `h.check(label, condition, got)` records a
result; a test fails on any failed check, any error thrown in the page, or an exception in the script (a
screenshot goes to `fail_<name>.png`). Helpers: `tabs`, `panes`, `paneCount`, `text`, `drop` + `csv`, `menu`,
`pick`, `expandTree`, `until`, `settingsLS`. Options: `settings` (merged into the first-launch settings),
`phone: true`, `wait` (ms after load). The settings seed is written once per test (a sessionStorage flag), so a
reload finds what the page saved rather than the seed.

The charting and table libraries can't load here, so `stub.js` and `tsstub.mjs` stand in for them: tests
check behaviour and state, not pixels. Anything visual (shading, hatching, markers) needs a look in the
published prototype.
