# Parity audit: the app against the prototype

Audited Nov 2, 2026 on `main` (814feec). Shared styles and sizes fixed Nov 3 (see "Fixed since the audit"); the rows below are as audited unless they say fixed. The reference is `prototype/plotter.html`. The app is `app/ui/` (Svelte 5 and
TypeScript), built with `npm run build` and run against the fake core (`app/ui/tests/fakecore.js`) with three
recordings in a folder `/data`. Both were screenshotted at 1440×900 in dark theme, once in light theme and once at
760×800. The prototype was loaded with the real uPlot and TanStack from the CDN, so its pixels are true. Every menu,
popover and dialog reachable from the main view was opened in both. Sizes below are computed styles read from the
page.

## Lead

The app has the prototype's skeleton: the rail, the slim top bar with tabs, Explorer, the pane stack with its ⋯ and
view menus, Measure, the overview, Play and Live. Most of what fills that skeleton is missing or simplified. That
includes controls the design puts in the Phase 3 shell port: the lock, focus mode, the time-format menu, keyboard
pan and zoom, the clear-panes buttons, sidebar panels that collapse, move and resize, the start page, multi-select
and drag in Explorer, channel categories, the full Settings list, and the tooltip system. Three things make the
app look and feel most unlike the prototype:

- The sizes don't match. The sidebar widths are swapped (left 260 px against 300 px, Measure 300 px against
  260 px). Rows, chips and pane headers are smaller. Plot areas have a filled background. Panes stretch to fill the
  window instead of keeping the prototype's fixed heights.
- The app has no motion, no focus rings and no custom tooltips. Its buttons and inputs don't inherit the UI font,
  so they render at 13.33 px.
- Measure, the values tiles and the tables show raw float precision and a reduced set of facts.

Status counts over the tables below, including the keyboard table (a row per control or behaviour): **Ported 107 ·
Partial 46 · Different 35 · Missing 90** (278 rows; counts as of Nov 3). Of the missing rows, 24 belong to Phase 3 (the shell port
that is already due), 31 to Phase 4 and 35 to Phase 5. A row tagged with several phases counts under its first.

Status key: **Ported** works as in the prototype. **Partial** exists with parts missing. **Different** exists but
looks or behaves otherwise; this includes app-only items. **Missing** is absent from the app. Phase is the roadmap
phase in `docs/design.md` that owns the item. The design's Phase 3 decision ("the app takes the prototype's UI at
the start of Phase 3: the shell, top bar, tabs, Explorer, the pane stack with its menus, the time modes and
transport, settings") puts every shell control in Phase 3. Where the design doesn't say, the Notes column says so.

Prototype references are `plotter.html` line numbers. App references are paths under `app/ui/src/`.

---

## 1. Shell

| Item | Prototype | App | Status | Phase | Notes |
| --- | --- | --- | --- | --- | --- |
| Rail views | Logo, Explorer, Events, Rules & checks, Layouts, Measure, Live, Settings (`6547-6558`) | Logo, Explorer, Measure, Live, Settings (`shell/Rail.svelte:21-28`) | Partial | 3 (Events, checks: 5; Layouts: 4) | Events, Rules & checks and Layouts are missing. |
| Rail logo | Button that opens the start page (`6551`, `6599`) | Button that opens the start page (`shell/Rail.svelte`) | Ported | 3 | Fixed Nov 3. |
| Settings position in rail | Under Live, with the other views; a spacer below (`6560`) | Pinned to the bottom of the rail (`shell/Rail.svelte:26-27`) | Different | 3 | Compare `proto_main_dark.png` with `app_main_dark.png`. |
| Rail Live button | Brings the live tab forward if there is one, else opens Connect. A dot shows while streaming (`6600-6604`) | Toggles the Connect dialog (`shell/Rail.svelte:25`) | Partial | 3 | No streaming dot, no jump to the live tab. |
| Rail toggles | Clicking the active view hides the sidebar (`6654-6666`) | Same (`shell/Rail.svelte:8`) | Ported | 3 | |
| Rail active marker | Accent-soft fill and a 2 px accent bar at the left edge | Same (`shell/Rail.svelte:75-88`) | Ported | 3 | |
| Ctrl+B / Ctrl+Alt+B | Left sidebar / Measure (`6796-6798`) | Same (`App.svelte:22-27`) | Ported | 3 | |
| Measure auto-open | Opens when both cursors are placed, closes on clear unless opened by hand (`6672-6682`) | Same (`ui.svelte.ts:46`, `shell/Rail.svelte:10-18`) | Ported | 3 | |
| Top bar order | Tabs · range · modes, Clear, Back, Fit, Play · all-panes view, Lock, Events · clear channels, remove panes · ⋯ (`6568-6588`) | Tabs · range · modes, Clear (only with cursors), Back, Fit, Play, ⋯ (`shell/TopBar.svelte:85-165`) | Partial | 3 (all-panes view: 4; Events: 5) | Groups 2 and 3 are missing; see the rows below. |
| Time range text | Mono, 3 decimals (`00:00:00.000`), a button that opens the time-format menu (`6590-6597`) | Mono, a button that opens the time-format menu; decimals follow the zoom | Partial | 3 | Fixed Nov 3: the button and menu; the decimals are auto (3 in the prototype). |
| Span text | Human duration: "2.00 h visible", "6.00 min visible" | Clock: "00:03:20.0 visible", "10.000 s visible" | Different | 3 | |
| Pointer time | "pointer 01:13:08.5" in the range bar | "00:02:00.6" with no label (`shell/TopBar.svelte:152`) | Different | 3 | |
| Mouse modes | Box zoom, Pan, Cursors as icons (`1558-1562`) | Same (`shell/TopBar.svelte:154-158`) | Ported | 3 | |
| Clear cursors button | Always on the bar (`1563`) | Always on the bar, disabled with no cursors | Ported | 3 | Fixed Nov 3. |
| Back | Backspace; no forward (`1565`, `6974`) | Alt+← and Alt+→ forward (`shell/TopBar.svelte:162`, `app.ts:585-600`) | Different | 3 | The app follows the design's table (Alt+←/→); the prototype uses Backspace. |
| Fit | F and the button, glides (`1564`, `5488`) | Same (`app.ts:580`) | Ported | 3 | |
| Play back button | Toggles the transport (`6579`) | Same (`shell/TopBar.svelte:164`) | Ported | 3 | |
| All panes as charts, tables or both | Segmented control (`1568-1572`, `setAllModes 5058`) | — | Missing | 4 | The follow-ups list the pane view switch under Phase 4. |
| Lock button | Lock icon, turns warn-coloured when on; L (`1555`, `5495-5560`) | Lock icon, on the bar after Back/Fit/Play; pressed state; L (`TopBar.svelte`, `app.ts` `locked()`) | Ported | 3 | Fixed Nov 3. |
| Event markers toggle | Flag icon, E (`1573`, `toggleEvents 4172`) | — | Missing | 5 | |
| Remove every channel / Remove every pane | Two icons, each with Undo (`6582-6586`, `clearPanes 6956`) | In ⋯ → All panes, each with Undo (not icons on the bar) | Different | 3 | Fixed Nov 3 in ⋯, where the quiet-UI rule puts rare actions; the prototype also has bar icons. |
| ⋯ menu | Grouped as View (Lock, Event markers, Clear cursors), All panes (three views, New pane, Remove every channel, Remove every pane), Export (Export data…, Save the view as an image…, Export to HTML…) and Window (Collapse all sidebar panels, Switch theme T, Focus mode Ctrl+., Settings…). Items already on the bar are left out (`6686-6727`) | Open recording…, View (Lock, Clear cursors, Overview above), All panes (New pane, Remove every channel, Remove every pane), Window (Switch theme, Focus mode, Show performance details, Settings…) | Partial | 3 (Export: 4; HTML: 5) | Fixed Nov 3 except: Export group (4; HTML: 5), Event markers (5), the three pane views (4), Collapse all sidebar panels. |
| Narrow top bar | Bar icons move into ⋯ when they don't fit (`6714-6716`) | No adaptation | Missing | 3 | See `proto_narrow.png` and `app_narrow.png`. |
| Note under the top bar | Warn dot, text, Undo, ×, goes after 12 s (`3542-3547`) | Same (`App.svelte:40-56`, `stack.svelte.ts:252`) | Ported | 3 | |
| Transport and live strip placement | Inside the main column, above the panes (`1625-1626`) | Full width under the top bar, above both sidebars (`App.svelte:38-39`) | Different | 3 | See `proto_play.png` and `app_play.png`. |
| Sidebar panels | Each section is a panel with a title bar: click to collapse, drag to reorder or move to the other side, ⇄ to send it across (`6258-6371`) | Explorer and each Measure panel collapse from their titles (saved); ⇄ swaps Explorer's and Measure's sides | Partial | 3 | Fixed Nov 3 except dragging a panel to reorder it or move it alone. |
| Collapse all sidebar panels | ⋯ → Window (`6283-6294`) | ⋯ → Window (Expand all when all are collapsed) | Ported | 3 | Fixed Nov 3. |
| Sidebar resize | Drag the inner edge (200 px to half the window, keeping 420 px of plots); double-click resets; saved (`6747-6779`) | Drag the inner edge, 200 px to half the window keeping 420 px of plots; double-click resets; saved (`App.svelte` `grip`) | Ported | 3 | Fixed Nov 3, for both sidebars. |
| Reset UI layout | Settings button (`6297-6303`) | Settings → Layout → Reset UI layout | Ported | 3 | Fixed Nov 3. |
| Status bar | State dot and text, memory, load, query, draw, points (`1662-1669`) | One string: points, kind, round trip (`App.svelte:76-78`) | Partial | 3 | |
| Start page | Open folder…, Open recording…, Connect live source…, recent sessions, recent recordings, live sources with saved connections, layouts in the folder, "Show this page at launch" (`7039-7212`) | Open folder…, Open recording…, Connect live source…, the last folder, recent recordings, "Show this page at launch" (`shell/StartPage.svelte`); with no file it fills the stack, else it opens over the tabs | Partial | 3 | Fixed Nov 3 except recent sessions, live sources with saved connections and layouts in the folder. |
| No file open | In the stack: "No file open", a line of help, Start page button (`3773`) | Centred "Open a recording / Drop a CSV or .tlm file here, or press ⌘O" (`App.svelte:69-72`), plus a note in Explorer | Different | 3 | The app's mode buttons stay enabled with no file (`app_empty.png`). |
| No panes in a tab | "No channels plotted", a line of help, a large + Add pane (`3777-3784`) | Only the "+ Add pane" bar (`App.svelte:65`, `app.ts:929-931`) | Different | 3 | |
| Focus mode | Ctrl/Cmd+. hides everything but the panes and range; Esc or "Exit focus" leaves (`6389-6397`) | Ctrl/Cmd+. or ⋯ → Focus mode hides the rail, tabs, buttons, sidebars and status bar; Esc or Exit focus leaves | Ported | 3 | Fixed Nov 3. |
| Lock toast | "The view is locked. Unlock" when a gesture is refused (`1769`, `lockedHint 5526`) | The note under the top bar: "The view is locked." with Unlock | Ported | 3 | Fixed Nov 3. It's the note, not a separate toast. |
| Theme toggle | T, and the ⋯ item (`5565`) | T and ⋯ → Switch theme | Ported | 3 | Fixed Nov 3. |
| Window title | "• TelemetryBench" while a layout has unsaved changes (`3578`) | Not set | Missing | 4 | |
| Scrim behind dialogs | rgba(3,5,8,.45), fades in over 220 ms (`233`) | Same (`style.css` `.scrim`) | Ported | 3 | Fixed Nov 3. |
| Narrow window | Under 980 px the sidebars wrap onto their own full-width rows (`85`) | Sidebar stays 260 px beside the panes | Different | 3 | `proto_narrow.png`, `app_narrow.png`. |
| Drop a file on the window | Opens the CSV import dialog | Opens the first dropped path (`app.ts:2031-2035`) | Different | 4 | The app has no import step; see section 8. |

## 2. Explorer

| Item | Prototype | App | Status | Phase | Notes |
| --- | --- | --- | --- | --- | --- |
| Panel title row | Collapse chevron and "EXPLORER", then icons: add the shown channels to the selected pane, new derived channel, categories on/off, a separator, Expand all, Collapse all, ⇄ (`6467-6503`) | "EXPLORER", then Open folder, Open recording, Expand all, Collapse all (`shell/Explorer.svelte:72-78`) | Different | 3 (derive, categories: 5) | The app has file-opening icons where the prototype has channel actions. |
| Folder menu | "recordings ▾" button: Open folder…, per folder Import all N files… and Remove from Explorer, Expand all folders, Collapse all folders, Import profiles and recipes…, Close all files (`6512-6517`) | The folder icon opens the picker (`shell/Explorer.svelte:74`) | Partial | 3 (import, recipes: 5) | `proto_folder_menu.png`. |
| Filter | Beside the folder menu, regex `.*` toggle inside, placeholder "Filter" (`6524-6530`) | Full width under the title, regex toggle inside, "Filter files and channels" (`shell/Explorer.svelte:79-82`) | Ported | 3 | |
| What the filter matches | Path, or channel name, unit or group (`5715`) | File name, or channel name (`shell/Explorer.svelte:41-42`) | Partial | 3 | Units and groups aren't searched. |
| Explorer ⋯ | Add all N channels to pane N, Open them in a new chart, Open them in a new table, Recipes and profiles… (`6518-6522`) | — | Missing | 3 (recipes: 5) | `proto_explorer_more.png`. |
| Open-files strip | "OPEN · n", Show only these (toggle), Close all, file chips with × (`2284-2297`) | "OPEN · n", Show only these (setting), Close all, chips with × | Ported | 3 | Fixed Nov 3. |
| Close all files with Undo | Undo restores the files and tabs (`closeTabs 3260`) | Undo reopens the files (not layout tabs) and brings the front one back (`app.ts` `closeAll`) | Ported | 3 | Fixed Nov 3. |
| Folder tree | Folders expand and collapse, kept (`2303`) | Same, kept in settings (`shell/Explorer.svelte:44-50`) | Ported | 3 | |
| `.tlmbench` entry | First row, with an "N shared views" badge (`2303-2330`) | — | Missing | 4 | In the follow-ups' Phase 4 list. |
| File badges | "no descriptor" on binary files nothing decodes (`2323`), a check badge "✗ 2" (5), LIVE on live files | None | Missing | 4 (checks: 5) | |
| File size | Not shown (the design: "Rows show only what matters: the data rate of imported files") | Size in MB on every file (`shell/Explorer.svelte:144`) | Different | 3 | |
| Show files that can't be opened | Switch with "(N hidden)" (`1599`, `2307`) | — | Missing | 3 | |
| Selected file row | Select first: actions beside the name (open, open only this) (`2335`) | Select first: an "Open" button (`shell/Explorer.svelte:140-142`) | Partial | 3 | |
| Open a file | Double-click or Enter | Same (`shell/Explorer.svelte:136-137`) | Ported | 3 | |
| File right-click menu | Open, Open only this, Channel prefix…, Format descriptor…, Compare with…, Close all files (`9416-9445`) | Open, Close this file, Close all files (`Explorer.svelte`) | Partial | 3 (descriptor: 4; prefix, compare: 5) | Fixed Nov 3 except Open only this, Channel prefix… (5), Format descriptor… (4), Compare with… (5). |
| Multi-select | Ctrl/Cmd-click adds, Shift-click a range, Ctrl+A all, Esc clears; files, channels or categories (`5826-5863`, `5899-5903`) | Ctrl/Cmd-click adds, Shift-click a range, Ctrl+A all channels, Esc clears; channels only | Partial | 3 | Fixed Nov 3 for channels; files and categories aren't multi-selectable. |
| Selection bar | At Explorer's foot for several rows: Add to pane N, Chart, Table, Remove, Derive… (`5887-5898`) | At Explorer's foot: Add to pane N, Chart, Table, Remove (also for one selected channel) | Partial | 3 (Derive: 5) | Fixed Nov 3 except Derive… (5). |
| Channel tree | File → category → channel; categories show "plotted / total" (`5779-5786`) | File → category → channel; categories (the start of the name before the first `_` or `.`, shared by two or more channels, else "other") show "plotted / total" and fold shut (saved) | Partial | 3 | Fixed Nov 3 with name-prefix categories, which the owner approved as the default; moving a channel to another category and New category… (right-click) are missing. |
| Category hover actions | Add the group to the pane, open in a new table, open in a new chart (`5783-5786`) | Add, Chart and Table on hover (`Explorer.svelte`) | Ported | 3 | Fixed Nov 3. |
| Channel checkbox | 14 px swatch: filled with the channel's colour when plotted anywhere, a check when in the selected pane; "pane 1" says where else (`5790-5798`) | 14 px swatch filled with the channel's colour when plotted anywhere, a check in the selected pane; "pane N" says where else | Ported | 3 | Fixed Nov 3. |
| Channel row text | Plotted channels in semibold; data rate on hover | Plotted channels in semibold; unit at the right; no data rate on hover | Partial | 3 | Semibold fixed Nov 3. |
| Channel row actions | Remove from every pane, add to the pane, new chart, new table (`5795-5798`) | The selection bar's Add to pane N, Chart, Table and Remove act on the selected channel | Ported | 3 | Fixed Nov 3. They sit in the bar at Explorer's foot, not beside the row (quieter). |
| Click and double-click | Click selects, checkbox plots, double-click plots (`5938-5940`) | Same (`shell/Explorer.svelte:158-163`) | Ported | 3 | |
| Drag a channel | To a pane, a pane's edge, between rows or onto empty space; a selection drags together; onto a category header to move it (`5941-5950`, `startDrag 5687`) | Rows drag (a picked set together) onto a pane, or onto empty space below the panes for a new pane | Partial | 3 | Fixed Nov 3 except pane edges, between rows and category headers. |
| Channel right-click | Move ACCEL_X to [category], New category…, Missing values and display…, Value labels…, Derive…, Mark when…, Add check… (`9416-9440`) | One item: Gap factor… (`app.ts:1871-1881`) | Different | 3 (display: 5; derive, mark, check: 5) | `proto_chan_menu.png`. The design's gap-factor decision puts Gap factor in this menu. |
| Gap factor dialog | — | Per channel, default 4×, Reset (`app.ts:1883-1929`) | Different | 2 | App only; it's in the design's Storage decisions but not in the prototype. |
| Derived and missing badges | ƒ glyph, "⌀ 312" (`5793`) | — | Missing | 5 | |
| Panes bar | "Panes + New / Clear / Remove all", shown with Show tips (`1596`) | — | Missing | 3 | |
| Add all to a target | "Add all N to (pane, new table, new chart, new chart + table)" (`1589-1595`, `fillAddTarget 5722`) | — | Missing | 3 | In the quiet UI, reached from Explorer's ⋯. |
| Empty Explorer | — | "No recording open. Open a folder or a recording, or drop a CSV or .tlm file on the window." (`shell/Explorer.svelte:121-123`) | Different | 3 | App only; the prototype always has a folder. |

## 3. Pane stack

| Item | Prototype | App | Status | Phase | Notes |
| --- | --- | --- | --- | --- | --- |
| Rows, columns, stacks | A row holds up to 3 columns (`MAX_PER_ROW 2719`) | Same model, up to 4 (`stack.svelte.ts:39`) | Different | 3 | |
| Pane height | Fixed by row count: 150, 125 or 105 px plots (`paneHeight 3758`) | Same: 150 px up to 3 rows, 125 for 4, 105 for more; they shrink (not scroll) when the window is too short; table and values panes fill (`PaneStack.svelte`) | Ported | 3 | Fixed Nov 3. The chart height grip is still missing. |
| Chart height grip | Drag under a chart to resize, side-by-side panes follow, double-click resets, arrows step (`3838`, `5183-5200`) | — | Missing | 3 | |
| Pane number | Drag handle; a grip shows on hover; accent when selected; tooltip (`3864-3871`) | Muted number, draggable (`shell/PaneStack.svelte:110-120`) | Partial | 3 | |
| Moving a pane | Onto a pane's left or right edge (beside), top or bottom (new row), middle (merge), or a gap between rows; Alt copies (`movePane 2898`, `bindPaneDrop 3914`, `makeGap 3981`) | Drop on a pane: it moves before that pane as its own row (`stack.svelte.ts:178-186`) | Partial | 3 | No edges, merge, gaps or copy. |
| Insert between rows | "+" in the gap between rows (`3791`) | — | Missing | 3 | |
| Selection | Click a header selects; accent bar at the left | Same | Ported | 3 | |
| Header right-click | Opens the pane menu (`3875`) | Same, except on chips (`shell/PaneStack.svelte:69-74`) | Ported | 3 | |
| Chips | Swatch, name, unit, "· L"/"· R" when two units share a pane, value while pointing, × on hover, drag to move or reorder (Alt copies), click opens the channel editor (`3822-3858`) | Colour dash, name, unit, value while pointing (`shell/PaneStack.svelte:121-125`) | Partial | 3 (editor: 5) | No ×, drag, reorder, side tag or click. |
| Chip overflow | A "+N" button lists every channel with live values when the header overflows (`5064-5203`, `openChanPop 5130`) | No overflow handling in the code | Missing | 3 | Not seen with 2 channels a pane; read from the code. |
| Coarse marker | Drawing level per chart with Show performance details | "coarse" pill while the exact view is on its way (`shell/PaneStack.svelte:127-129`) | Different | 2 | App only, for the pyramid's coarse-to-exact view. |
| View button | Current view's icon and a chevron (`3832`) | A generic layout icon (`shell/PaneStack.svelte:130-146`) | Different | 3 | |
| View menu | Chart, Table, Chart and table, Chart and values, Values, Gauges, Checks, XY plot, Spectrum, each with an icon and a hint (`11020-11034`) | Chart, Table, Chart and table, Values, no icons (`shell/PaneStack.svelte:33-38`, `186-201`) | Partial | 3 (cv, gauges, checks, XY, spectrum: 5) | The check mark was unsized and drew about 150 px wide; fixed Nov 3 (14 px). |
| ⋯ pane menu | Event markers in this pane, Axis and scale…, Close others, Close panes below, Close pane, Duplicate, Split right, Split down, Add a pane below (`2824-2848`) | Same without Event markers (`shell/PaneStack.svelte:43-53`) | Partial | 5 | Only the Phase 5 item is missing. |
| Close with Undo | Close pane, others and below are undoable (`2797-2812`) | Same (`stack.svelte.ts:159`) | Ported | 3 | |
| Duplicate | Below, or beside with Shift-click; Ctrl/Cmd+D (`2872`, `6790`) | Below, or beside with Ctrl/Cmd+Shift+D (the menu: below only) | Partial | 3 | Ctrl/Cmd+D added Nov 3; the menu still has no beside. |
| Split right, split down | `2977`, `2743` | `stack.svelte.ts:118-136` | Ported | 3 | |
| Add pane bar | "+ Add pane" and a ▾ menu of types (Chart, Table, Chart + table, Chart + values, Values); Ctrl+Shift+N adds one below the selected pane (`2852-2869`) | "+ Add pane" only (`App.svelte:65`) | Partial | 3 | |
| Empty pane | "Empty pane"; "Drag a channel here, or select this pane and click a channel." (`3816`) | "Tick channels on the left to plot them here" (`shell/PaneStack.svelte:124`) | Different | 3 | |
| Chart view | uPlot per pane | WebGL, one canvas for all panes (`app.ts:619`) | Ported | 1 | Different engine by design. |
| Chart and table | Chart above, table below, table height grip | Same (`shell/PaneStack.svelte:175-176`, `shell/Table.svelte:165-187`) | Ported | 4 | |
| Values view | Tiles with "at cursor A · 00:32:57.3" (or pointer or end of view) above them, decimals from the channel, "no limits", "Click to set limits"; the pane shrinks to fit (`3835`, `tileHtml`) | Tiles with name, value and unit (`shell/PaneStack.svelte:159-171`); values at raw precision (`0.340868`), no "where" line; the pane keeps its full height | Partial | 3 (limits: 5) | `proto_pane_value.png`, `app_pane_values.png`. |
| Chart and values | `3839` | — | Missing | 5 | |
| Gauges | `8067-8508` | — | Missing | 5 | |
| Checks view | `11465` | — | Missing | 5 | |
| XY plot | X channel picker, 1:1, Fit (`10786-10892`) | — | Missing | 5 | The design lists XY plots in the plot stack without a phase. |
| Spectrum | `10893-11037` | — | Missing | 5 | Part of derived channels in the design. |
| Overview title | "Overview of [selected pane · 2 channels ▾]": follow the pane or pin any channel (`5973-5990`) | "Overview of pane N" (`app.ts:933-944`) | Partial | 3 | No channel picker. |
| Overview drawing | Each channel's min/max band scaled to its own range plus a midline; an event strip below (`5990-6050`) | The pane's traces as lines (`app.ts:872`) | Different | 3 (events: 5) | |
| Overview gestures | Follows the mouse mode: Box drags a new view, a click centres; Pan drags the window; Cursors places A and B; the wheel zooms (`6051-6142`) | Drag the window, or click to move it there (`app.ts:1813-1843`) | Partial | 3 | No box, cursors or wheel. |
| Overview time range | "00:00:00 – 02:00:00" at the right (`1635`) | — | Missing | 3 | |
| Show on top | A switch in the overview header, and a setting (`1635`, `placeOverview 6732`) | A text link that flips between "Show on top" and "Show at the bottom", a setting and a ⋯ item (`app.ts:940`) | Different | 3 | |

## 4. Charts

| Item | Prototype | App | Status | Phase | Notes |
| --- | --- | --- | --- | --- | --- |
| Y axes | A unit's axis on the left, a second unit on the right | Same (`axes.ts`) | Ported | 2 | |
| Time labels | Bottom chart, or every chart (Settings › Time labels) (`1721`) | Settings → Time labels: bottom chart only or every chart | Partial | 3 | Fixed Nov 3 except Automatic (every chart on a phone). |
| Axis and scale… | Left and right axis: range (fit, ignore outliers, include zero, fixed), decimals, label, log; Reset axes, Done (`10750`) | Same fields (`shell/AxisDialog.svelte`) | Ported | 4 | Title "Pane 1 axes" against "Pane 1 settings · axes". Reset axes is a bordered button and Done is filled (fixed Nov 3). |
| Ignore outliers clipping | Spikes clipped and marked | Same (`app.ts:741`) | Ported | 4 | |
| Cursors | Click in Cursors mode (A, then B), drag either | Same (`app.ts:1711-1736`, `nearCursor 1684`) | Ported | 2 | |
| A and B keys | Over a table row (`4810`) | Over a plot (`app.ts:1972`) | Different | 3 | Each has half of what the design asks for (plot and table row). |
| Cursor flags | A and B flags at the top of each pane | Same (`app.ts:817`) | Ported | 2 | |
| Esc clears cursors | `6944` | `app.ts:1967` | Ported | 2 | |
| Measure: cursors | Tiles: A, B, Δt (highlighted), 1/Δt; a table of Channel, A, B, Δ (`1642-1650`, `fillInspector 6153`) | Tiles A, B, Δt (highlighted), 1/Δt; a table of Channel, A, B, Δ in each channel's decimals (`app.ts` `refreshMeasure`) | Ported | 3 | Fixed Nov 3. |
| Measure: stats | Own panel with a channel picker: Samples, Min, Max, Mean, RMS, Std dev, Missing (count and duration, warn colour), Computed in; over the visible range when no cursors are placed (`1651-1657`, `6153-6195`) | Own panel with a channel picker: Samples, Min, Max, Mean, RMS, Std dev, Computed in; between the cursors, else the visible range | Partial | 3 | Fixed Nov 3 except Missing (count and duration): the core doesn't report it. |
| Measure without cursors | Cursor tiles show "—"; stats cover the visible range | Tiles show "—" with a hint; stats cover the visible range and follow the view | Ported | 3 | Fixed Nov 3. |
| Measure clear button | — | × in Measure's title (`app.ts:1009`) | Different | 3 | App only. |
| Measure panels | Cursors and Stats as two collapsible panels with titles (`1642`, `1651`) | Cursors and Stats as two collapsible panels (saved) | Ported | 3 | Fixed Nov 3. |
| Hover readout | Values in chips while pointing; dots on the traces | Same (`app.ts:844`, `updateChips 945`) | Ported | 2 | |
| Event markers | Flags and shaded ranges on every chart, a tooltip on hover (`showEvTip 4146`), per-pane toggle | — | Missing | 5 | |
| Limit lines | Dashed warn and alarm levels from channel limits | — | Missing | 5 | |
| Gaps | Missing values become gaps everywhere (channel settings) | Lines break at time jumps over the gap factor (`app.ts:219`) | Different | 2 (missing values: 5) | |
| Table filter on charts | Matches shaded with a tick; Apply to charts hides rows with hatching and a chip (`5255-5365`) | — | Missing | 4 | |
| Table rows band | The rows a table shows are a shaded band on the charts (`tableWins 4308`) | — | Missing | 4 | In the follow-ups. |
| Box zoom | ≥ 4 px zooms; the box turns solid when release will zoom; a box too small fades with "Drag wider to zoom" the first 3 times (`5460-5476`) | ≥ 4 px zooms with a glide (`app.ts:1771-1779`) | Partial | 3 | No armed state, fade or hint. |
| Maximum zoom note | "Maximum zoom" at the pointer (`zoomNote 5384`) | — | Missing | 3 | |
| Pan | Pan mode, Shift- or middle-drag, ← → keys (`6936-6937`) | Pan mode, Shift- or middle-drag, ← → keys (`app.ts:1718`, `panBy`) | Ported | 3 | Fixed Nov 3. |
| Wheel zoom | At the pointer, speed from settings; off makes Ctrl+wheel zoom (`5407-5450`) | At the pointer, speed from settings; horizontal wheel pans (`app.ts:1788-1810`) | Partial | 3 | No "Scroll wheel zooms" setting. |
| Keyboard zoom | + − ↑ ↓ (`6938-6939`) | ← → pan 15%, + − ↑ ↓ zoom 0.7× / 1.43× (`app.ts` `panBy`, `zoomBy`) | Ported | 3 | Fixed Nov 3. |
| Double-click fits | Setting dblFit | Same (`app.ts:1783`) | Ported | 3 | |
| Glide | Fit, box zoom, Back glide; reduced motion respected (`glideView 5208`) | Same (`app.ts:556-578`) | Ported | 3 | |
| Draw every sample | Setting: Automatic, up to 100k, up to 1M, Always (`1720`) | Decided by the core | Missing | 3 | |
| Time format and decimals | Ten formats and fixed decimals (`1722-1723`, `fmtTime 2523`) | Seven formats (Automatic, Clock, Seconds, Milliseconds, Minutes and seconds, Hours, Days) and fixed decimals (`axes.ts` `timeLabel`) | Partial | 3 | Fixed Nov 3 except Weeks, Years, Samples and Wall clock. |
| Channel colours | The least-used palette colour, first among its pane (`assignColor 2748`) | Fixed by channel index: `--trace-(ch % 8 + 1)` (`app.ts:175`) | Different | 3 | Two channels in one pane can share a colour in the app. |
| Building hatch | — | Hatching and progress while the pyramid builds (`app.ts:789`) | Different | 2 | App only. |
| Channel editor | Display (name shown, colour, decimals, unit label, scale, offset, missing values, valid range, Missing when), Limits, Value labels (`10643`) | — | Missing | 5 | The design doesn't phase display settings; they sit with missing values. |
| Y zoom | — | — | Missing | 3 | Neither has it; the design's table lists Shift+scroll and dragging the y axis. |

## 5. Tables

| Item | Prototype | App | Status | Phase | Notes |
| --- | --- | --- | --- | --- | --- |
| Rows in view | Samples in the time view, virtualised; rows follow the fastest channel (`4298-4310`) | Same, read a window at a time from the core (`shell/Table.svelte:94-120`) | Ported | 4 | |
| Status line | "7,200,001 rows in view · one per accel_x sample (1 kHz) · slower channels show their latest sample, dimmed when unchanged" | "200,000 rows in view" (`shell/Table.svelte:190`) | Partial | 4 | |
| Slower channels dimmed | Last sample held, dimmed where unchanged | Can't tell: the fake recordings have one rate, and `Table.svelte` has no dimming | Missing | 4 | Read from the code, not seen. |
| Value format | The channel's decimals (`0.030`) | Each channel's decimals, learned from the first axis range that drew it (or the rows a table read): four digits at its largest magnitude | Ported | 3 | Fixed Nov 3. The prototype's decimals come from its data; the core has no per-channel decimals yet (the channel editor, Phase 5, sets them). |
| Event column | Event tags; click reveals the event (`4857`) | — | Missing | 5 | |
| Sort | Header click (setting) or the ▾ menu: ascending, descending, clear; several columns | — | Missing | 4 | In the follow-ups (sort in Rust). |
| Filter row | An input under each header ("e.g. > 1.5"); and/or, ranges, blank (`4310-4340`) | — | Missing | 4 | |
| Filter strip | In view / Whole file, ▲ ▼ through matches, Save as rule, Apply to charts, Clear (`4518-4600`) | — | Missing | 4 (Save as rule: 5) | |
| Clear sort and filters | Button (`4518`) | — | Missing | 4 | |
| Columns | Popover: show or hide each column, Show all, Hide all, click a name to scroll to it (`openColumnsPop 4453`) | — | Missing | 4 | |
| Autofit | Fit the columns in view, Fit all to contents, Fit all to headers, Reset widths | — | Missing | 4 | |
| Column resize | Drag; widths kept | Same, kept with the layout; double-click resets one column (`shell/Table.svelte:205-226`) | Ported | 4 | |
| Header ▾ menu | Sort ascending, Sort descending, Clear sort, Autofit this column, Hide this column, Select this column | — | Missing | 4 | |
| Column selection | Click selects (Shift range, Ctrl add); a bar to Hide, Open in a new table, Plot in a new chart | — | Missing | 4 | |
| Pinned columns | Sample and Time stay put while scrolling sideways (`pinLeft 4685`) | Sticky header only (`shell/Table.svelte:300-303`) | Missing | 4 | |
| Cursor line | A and B times and Δt in the table bar (`4518`) | — | Missing | 4 | |
| Go to A / Go to B | `4518` | `shell/Table.svelte:192-193` | Ported | 4 | |
| Row click | Cursors mode: A, then B, then alternating; right-click menu; double-click centres (`4808-4822`) | Click places A, Shift-click B, double-click centres (`shell/Table.svelte:150-156`) | Different | 4 | The app follows the design ("A, Shift: B"). |
| Row right-click | Set cursor A here, Set cursor B here, Zoom the charts to this row, Clear cursors (`4816-4820`) | — | Missing | 4 | |
| Row hover | Crosshair follows on the charts (`syncCrosshair`, `4822`) | — | Missing | 4 | |
| Rows inside events tinted | Setting eventsTables (`1695`) | — | Missing | 5 | |
| Table height in Chart and table | Grip; double-click resets | Same (`shell/Table.svelte:165-187`) | Ported | 4 | |
| Keyboard | ↑ ↓ PageUp PageDown move (`4428`) | The scroll area takes focus and scrolls | Partial | 4 | |
| Copy as CSV | — | — | Missing | 4 | Neither has it; the design and follow-ups ask for it. |

## 6. Time modes

| Item | Prototype | App | Status | Phase | Notes |
| --- | --- | --- | --- | --- | --- |
| Analyze | Default mode | Same | Ported | 3 | |
| Space plays and pauses | `8628` | `app.ts:1958-1962` | Ported | 3 | |
| Transport layout | Row 1: play, time, speed, windows, Following, A–B loop, Stop at…, ×; row 2: a full-width scrubber (`8585-8640`) | One row: play, time, scrubber, end time, speed, windows, Following, Loop A–B, × (`shell/Transport.svelte`) | Different | 3 | |
| Playhead time | "00:32:57.3 / 02:00:00" | "53.784 s" with the end time apart ("00:03:20.0") | Different | 3 | |
| Speed | ¼× to 1 day/s; the default plays the file in ≤ 3 min (`8514`, `8522`) | Same list and rule (`play.svelte.ts:5`, `app.ts:445`) | Ported | 3 | |
| Window presets | 10 s, 1 min, 10 min, 1 h | Same (`play.svelte.ts:7`) | Ported | 3 | |
| Follow | Following / a view you choose holds it | Same | Ported | 3 | |
| A–B loop | "A–B loop" | "Loop A–B" | Ported | 3 | Label differs. |
| Stop at… | Pause at any event or chosen categories (`openStopAt 8578`) | — | Missing | 5 | In the follow-ups. |
| Close transport | The view stays | Same | Ported | 3 | |
| Live strip | LIVE/PAUSED/STOPPED/ENDED badge, session name, REC time with "holding X behind live", scrubber, −time, windows 10 s/30 s/1 min/5 min, Following/Jump to live, Trigger, "N sources" (`buildLiveStrip 7635`) | LIVE/ENDED, REC time, scrubber, −time, windows, Following/Jump to live, Sources (`shell/LiveStrip.svelte:15-40`) | Partial | 3 | No session name, PAUSED or STOPPED, Trigger or source count. |
| Sources | Popover: a card per source with a state pill, Pause/Resume, Stop/Remove/Reconnect; + Add source; Stream health (total rate, UI updates, sample → pixel, history in memory, dropped, CRC errors); Recording (file, size) (`7662-7800`) | Panel under the strip: source, rate, recorded, dropped, missing, buffer, recording file, error, Stop recording (`shell/LiveStrip.svelte:41-56`) | Partial | 3 | + Add source is deferred in the follow-ups. |
| Trigger | Oscilloscope trigger on a level or edge; single or repeat; Disarm (`7543-7634`) | — | Missing | 3 | The design calls triggers v1 without a phase. |
| Connect dialog | Two columns: source kinds with descriptions (File playback, Serial port, UDP, TCP, Tail a file), then Name, per-kind fields, Record while streaming, Save recordings to, Save this connection on the start page; footer "Opens in a new live tab" (`7238-7348`) | One column: Serial / TCP / UDP / Growing file tabs, the fields, a descriptor, "Recorded to the Recordings folder" (`shell/Connect.svelte`) | Partial | 3 | `proto_connect.png`, `app_connect.png`. The app has no name, recording switch, folder or saved connections. |
| Live tab marker | Pulsing dot on a live tab (`991`) | — | Missing | 3 | |
| Live rail dot | `6604` | — | Missing | 3 | |

## 7. Settings

The prototype's Settings panel (`1672-1740`, tidied by `tidySettings 6400`) and the app's (`shell/Settings.svelte`).

| Item | Prototype | App | Status | Phase | Notes |
| --- | --- | --- | --- | --- | --- |
| Basic and All | Segment under the search (`6412-6414`) | Segment above the search (`shell/Settings.svelte:34-38`) | Ported | 3 | Order is swapped. |
| Search | Over names, explanations and choices; Esc clears (`6415-6443`) | Over labels, hints and options (`shell/Settings.svelte:25-29`) | Ported | 3 | |
| ⓘ explanations | `i` button per row (`6407-6409`) | ⓘ per row (`shell/Settings.svelte:45`) | Ported | 3 | |
| Switch style | Toggle switches | Switches (`input[role=switch]`) | Ported | 3 | Fixed Nov 3. |
| Basic set | Theme, Time format, When a file opens, Files opened for the first time, Channels from several files, Clicking a row, Tooltips, Quiet interface, Mouse mode, Group channels, Left sidebar, Measure panel (`6399`) | Mouse mode, Theme, Overview above the panes, Open Measure… (`shell/Settings.svelte:16-22`) | Different | 3 | |
| Footer | "Saved in this browser as you change them", Reset to defaults | "Saved as you change them", Reset to defaults (`shell/Settings.svelte:63`) | Ported | 3 | |
| Lock the time view | `1676` | `shell/Settings.svelte` | Ported | 3 | Fixed Nov 3. |
| Cursors still work while locked | `1677` | `shell/Settings.svelte` | Ported | 3 | Fixed Nov 3. |
| Also lock the layout | `1678` | — | Missing | 3 | |
| Table sort and filter still work while locked | `1679` | — | Missing | 4 | |
| Click a column header to sort | `1683` | — | Missing | 4 | |
| Mouse mode | `1687` | `shell/Settings.svelte:16` | Ported | 3 | |
| Double-click fits the whole recording | `1688` | `shell/Settings.svelte:18` | Ported | 3 | |
| Zoom speed | `1689` | `shell/Settings.svelte:17` | Ported | 3 | |
| Scroll wheel zooms the charts | `1690` | — | Missing | 3 | |
| Panes mark the events of | `1694` | — | Missing | 5 | |
| Tint table rows inside events | `1695` | — | Missing | 5 | |
| Event categories | Name, colour, show, delete; + Add category (`1696-1698`, `renderCatRows 5568`) | — | Missing | 5 | |
| When a file opens | Remember its last layout, Starter charts, Starter charts + tables, One table, One values pane, Nothing plotted (`1702`) | Settings → When a file opens: Remember, Starter charts, Starter charts + tables, One table, One values pane, Nothing plotted | Ported | 3 | Fixed Nov 3. The default for a first-time file is Starter charts (the prototype's is one table); `fileOpenNew` changes it. |
| Files opened for the first time | `1703` | Settings → Files opened for the first time | Ported | 3 | Fixed Nov 3. |
| Forget remembered file layouts | `1704` | Settings → Layout → Forget layouts | Ported | 3 | Fixed Nov 3. |
| Derived channels and import profiles: Manage… | `1705` | — | Missing | 5 | |
| Channels (display settings list) | `1709`, `renderChannelSettings 10720` | — | Missing | 5 | |
| Show the start page at launch | `1713` | Settings → Show the start page at launch; over the restored tabs at launch | Ported | 3 | Fixed Nov 3. |
| Record live sessions to | `1714` | — | Missing | 3 | The app records to a fixed Recordings folder. |
| Session status, Reset session | `1715` | — | Missing | 3 | |
| Theme | `1719` | `shell/Settings.svelte:19` | Ported | 3 | |
| Draw every sample | `1720` | — | Missing | 3 | |
| Time labels | `1721` | Settings → Time labels | Ported | 3 | Fixed Nov 3 (Automatic for phones isn't offered). |
| Time format | `1722` | Settings → Time format, and the menu on the time range | Partial | 3 | Fixed Nov 3; see the row above for the formats missing. |
| Time decimals | `1723` | Settings → Time decimals | Ported | 3 | Fixed Nov 3. |
| Group channels into categories | `1724` | Settings → Group channels into categories (Basic) | Ported | 3 | Fixed Nov 3. |
| Overview above the panes | `1725` | `shell/Settings.svelte:20` | Ported | 3 | |
| Left sidebar | `1726` | Not a setting; the rail toggles it (`settings.leftPanel`) | Partial | 3 | |
| Measure panel | `1727` | Not a setting; the rail toggles it (`settings.measureOpen`) | Partial | 3 | |
| Open Measure when both cursors are placed | `1728` | `shell/Settings.svelte:21` | Ported | 3 | |
| Channels from several files | `1729` | — | Missing | 5 | |
| Clicking a row | `1730` | Settings → Clicking a row: Select first, Act at once (Explorer) | Ported | 3 | Fixed Nov 3. |
| New pane from a dragged channel | `1731` | — | Missing | 3 | |
| Tooltips: Short and learn, Full, Off | `1732` | Settings → Tooltips (`tips.ts`) | Ported | 3 | Fixed Nov 3. |
| Learned tooltips: Reset | `1733` | Settings → Reset tooltips | Ported | 3 | Fixed Nov 3. |
| Quiet interface | `1734` | Settings → Quiet interface: hides "pane N" badges, file sizes and category counts | Partial | 3 | Fixed Nov 3 for Explorer; chips, file numbers and grips aren't affected. |
| Show tips and extra controls | `1735` | — | Missing | 3 | |
| Show performance details | `1736` | `shell/Settings.svelte:22` | Ported | 3 | |
| UI layout: Reset UI layout | `1737` | Settings → Layout | Ported | 3 | Fixed Nov 3. |

## 8. Layouts, sharing, export, descriptor, derived, rules, events, widgets

| Item | Prototype | App | Status | Phase | Notes |
| --- | --- | --- | --- | --- | --- |
| Tab contents | File or layout icon, name, slot badges, check badge, dirty dot, lock on read-only, live dot, ⌄, × (`renderTabs 3301`) | File or layout icon, name, dirty dot, ⌄, × (`shell/TopBar.svelte:87-133`) | Partial | 4 (slots, checks: 5) | |
| Tab width | Up to 220 px; 206 px for "synthetic_flight.bin" | 146 px for "flight.tlm" | Ported | 3 | Same styles; the names differ. |
| Tab drag to reorder | `moveTab 3291` | `shell/TopBar.svelte:100-108` | Ported | 3 | |
| Tab keys | Ctrl+PageUp/PageDown switch, Ctrl+Shift+PageUp/PageDown move, Delete closes, Enter/Space shows (`3340-3348`, `3644`) | Enter/Space shows (`shell/TopBar.svelte:97`) | Partial | 3 | |
| Middle-click closes a tab | `3308` | `shell/TopBar.svelte:98` | Ported | 3 | |
| Tab overflow | ‹ › scroll buttons when tabs don't fit | Names truncate ("f…") | Missing | 3 | `proto_narrow.png`, `app_narrow.png`. |
| Tab menu header | Name and × (`1743`) | None | Missing | 4 | `proto_tab_menu.png`, `app_tab_menu.png`. |
| Tab menu status | What the tab is (starter layout, saved, unsaved, shared, built-in, live) (`updateWsStatus 3535-3567`) | None | Missing | 4 | |
| This tab shows | The tab's files as numbered slots (`3537`) | None | Missing | 5 | |
| Save layout ⌘S | Always listed, disabled when there's nothing to save (`1749`) | Listed on layout tabs only (`shell/TopBar.svelte:182-184`) | Different | 4 | |
| Save as… | Name form (`1750`, `openNameForm 3589`) | Name form (`shell/TopBar.svelte:168-180`) | Ported | 4 | |
| Revert changes | `1751` | `shell/TopBar.svelte:194-196` | Ported | 4 | |
| Share with this folder… | Writes `.tlmview.json` with slots, alignment, time range (`1752`, `shareView 3231`) | Writes `.tlmbench/views/` with the layout, file and time range; Update shared copy… (`shell/TopBar.svelte:197-204`, `app.ts:1542`) | Ported | 4 | |
| Export file… | — | Tab menu item (`shell/TopBar.svelte:205`, `app.ts:1567`) | Different | 4 | App only. |
| Delete layout | With Undo (`deleteLayout 3441`) | With Undo (`app.ts:1448-1458`) | Ported | 4 | |
| Close tab, others, all | Undoable; live sources stop (`closeTabs 3260`) | Not undoable (`app.ts:1338`) | Partial | 3 | |
| + menu | "New tab" header and ×; Empty layout…; your layouts with delete; shared layouts with "1 file · ready · a.chen" or "needs file 2" in warn; Starters (`openAddMenu 3420-3460`) | Empty layout…; your layouts ("N panes", delete); shared layouts ("by …"); a note (`shell/TopBar.svelte:219-275`) | Partial | 4 (needs and slots: 5) | `proto_add_menu.png`, `app_add_menu.png`. |
| Built-in starter layouts | Starters group | — | Missing | 4 | |
| Slot picker | Which file fills each slot before a layout opens (`askForSlot 3218`, `openLayout 3465`) | — | Missing | 5 | |
| Layouts sidebar | Your layouts, shared, starters; New empty layout…, Save current as… (`renderLayoutList 6609`) | — | Missing | 4 | In the follow-ups. |
| Shared view opens its file and range | `applySharedFiles 3204` | `app.ts:1510-1525` | Ported | 4 | |
| Session restore | Per folder, with Reset session (`6981-7038`) | Tabs, layouts and the front tab restored at launch (`app.ts:2115-2135`) | Partial | 3 | No per-folder sessions, no reset. |
| Share dialog | Previews the TOML files written for events, categories, recipes, checks (`shareDialog 10490`) | — | Missing | 5 | |
| Export data… | Format, time, channels, wide or long, resample, time format, size and files, the matching command (`openExportData 10436`) | — | Missing | 4 | Proposed decision. |
| Save the view as an image… | Every pane or the selected one; PNG 1× or 2×, SVG (`openExportImage 10468`) | — | Missing | 4 | Proposed decision. |
| Export to HTML… | `openExportHtml 10395` | — | Missing | 5 | |
| Descriptor form | Framing, byte order, header, time, fields table, hex preview, decoded rows, Find frames, TOML beside it (`openDescDlg 9621-9820`) | — | Missing | 4 | Proposed decision, awaiting review. |
| CSV import dialog | Time column and unit, data rate, columns (name, unit, type, category), profiles, several files grouped (`8961-9472`) | CSVs open straight away | Missing | 4 (profiles: 5) | The design doesn't phase the import step. |
| Import a local folder | Import all, Import selected (`importLocal 9813`) | — | Missing | 4 | |
| Open together, Compare | `openFilesTogether 3126`, `compareFiles 2221` | — | Missing | 5 | |
| Channel prefix… | `9439` | — | Missing | 5 | |
| Derive… | Expression editor with recipes (`openDerive 10193`) | — | Missing | 5 | |
| Recipes and profiles manager | `openRecipeManager 10281` | — | Missing | 5 | |
| Events panel | + Event (marker at A, range A–B), Share…, Rules, Categories, filter, grouping, category chips, list with start and duration, inline edit, Expand/Collapse all (`1609-1613`, `renderEventList 6819`) | — | Missing | 5 | `proto_rail_events.png`. |
| Rule events | `11038` | — | Missing | 5 | |
| Rules & checks panel | Sets on the files in front, progress, results (`renderChecksPanel 11451`) | — | Missing | 5 | |
| Check editor | `openCheckEditor 11562` | — | Missing | 5 | |
| Folder runs | `openFolderRun 11705` | — | Missing | 5 | |
| Jump to violations | `openAtViolation 11777` | — | Missing | 5 | |
| Widgets | Gauge cards, widget menu, your widgets editor, code widgets with a trust prompt, limits popover (`8067-8508`, `8641-8736`) | — | Missing | 5 | |

## Fixed since the audit

Nov 3, 2026, in `app/ui/src/style.css` and the shell components; `t_shell` checks the sizes, the font and the ring.
Look and feel items 1, 2, 6 (the ring), 7, 9, 12, 13, 14, 18, 19, 31, 33, 35, 36, 39, 40, 42 and 43 (menus, dialogs, scrim
and pressed controls, all off under reduced motion) are fixed in the app: the sidebars are 300 px (left) and 260 px
(Measure); buttons, inputs and selects inherit the UI font; `:focus-visible` shows the accent ring; `--good`,
`--danger` and `--on-accent` exist in both themes and the fallbacks are gone; the plot fill is gone; pane headers are
32 px with 26 px chips; tree rows are 30 px at 13 px with plotted names in semibold; open-file chips are 24 px; the
status bar is 28 px. Still open in those items: the sidebar's drag-to-resize (1), the filter input's height and radius
(6), chip width, spacing, × and drag (14, 15), and the tooltip system (44).

The same day: Lock (button, L, the three-state note with Unlock, and the two lock settings), the keyboard (← → pan, + − ↑ ↓
zoom, T theme, L lock, Ctrl/Cmd+. focus mode, Ctrl/Cmd+D duplicate, Ctrl/Cmd+Shift+N new pane), Focus mode with
Exit focus, Clear cursors always on the bar, and the ⋯ menu's groups with Remove every channel and Remove every pane
(Undo). `t_controls` covers them. "Also lock the layout" isn't done, and the lock doesn't yet cover the table
sort and filter (Phase 4).

Measure, then the sidebars (Nov 3): Measure has the prototype's two collapsible panels: Cursors (tiles A, B, Δt, 1/Δt and
a Channel, A, B, Δ table) and Stats (a channel picker, Samples, Min, Max, Mean, RMS, Std dev, Computed in), over the
visible range when no cursors are placed. Values in chips, tiles, Measure and tables use each channel's decimals. The
sidebars resize by their inner edges, panels collapse (saved), ⋯ → Collapse all sidebar panels, ⇄ swaps Explorer's and
Measure's sides, and Settings → Reset UI layout puts it all back. `t_view`, `t_tables` and `t_sidebars` cover them.
Look and feel items 3 (padding aside), 4 (no ⇄ per panel yet), 27 and 29 are fixed. Not done: dragging a panel to
reorder it, Missing in Stats, and the sidebar padding.

Explorer's channels (Nov 3): the swatch checkbox, multi-select with the selection bar, dragging channels onto panes, the
file right-click menu, Show only these and Undo on Close all files. `t_channels` covers them. Channel categories group by name prefix (Settings → Group channels into categories). Not done: moving a channel between categories, "Show files that can't be opened", Explorer's
⋯ and folder menus, Open only this, dragging onto pane edges.

Start page and switches (Nov 3): the start page (with no file, and from the rail's logo over the tabs), recent
recordings, "Show this page at launch", and switches in Settings and on the page. `t_start` covers them. The empty
state's mode buttons still stay enabled with no file.

Tooltips (Nov 3): one floating tip for every `data-tip` (700 ms, at once on keyboard focus, warm for 1.5 s), Short (first
clause and shortcut, retired after three showings), Full or Off, and Reset tooltips. `t_tips` covers it. Look and feel
item 44 is fixed except that the native `title` is no longer set at all.

Time format (Nov 3): Settings → Time format, Time decimals and Time labels, and the menu on the time range (Automatic,
Clock, Seconds, Milliseconds, Minutes and seconds, Hours, Days). `t_time` covers it.

File-open settings (Nov 3): When a file opens, Files opened for the first time, Forget layouts, Clicking a row (Act at
once) and Quiet interface. `t_fileopen` covers them.

## 9. Look and feel

The audit's screenshots were kept in a scratch folder and are gone; `app/ui/tests/shots.js` regenerates the app's (below, `docs/images/`). The pairs were named `proto_<view>.png` and `app_<view>.png`. The pairs are:

| View | Prototype | App |
| --- | --- | --- |
| Main, dark | `.../shots/proto_main_dark.png` | `.../shots/app_main_dark.png` |
| Main, light | `.../shots/proto_main_light.png` | `.../shots/app_main_light.png` |
| 760×800 | `.../shots/proto_narrow.png` | `.../shots/app_narrow.png` |
| Cursors and Measure | `.../shots/proto_cursors_measure.png` | `.../shots/app_cursors_measure.png` |
| ⋯ menu | `.../shots/proto_more_menu.png` | `.../shots/app_more_menu.png` |
| Tab menu | `.../shots/proto_tab_menu.png` | `.../shots/app_tab_menu.png` |
| + menu | `.../shots/proto_add_menu.png` | `.../shots/app_add_menu.png` |
| Pane hover | `.../shots/proto_pane_hover.png` | `.../shots/app_pane_hover.png` |
| Pane ⋯ menu | `.../shots/proto_pane_menu.png` | `.../shots/app_pane_menu.png` |
| Axis and scale | `.../shots/proto_axis_dialog.png` | `.../shots/app_axis_dialog.png` |
| View menu | `.../shots/proto_view_menu.png` | `.../shots/app_view_menu.png` |
| Chip click / right-click | `.../shots/proto_chip_editor.png` | `.../shots/app_chip_menu.png` |
| Channel right-click | `.../shots/proto_chan_menu.png` | `.../shots/app_chan_menu.png` |
| Selected channel | `.../shots/proto_chan_selected.png` | `.../shots/app_chan_selected.png` |
| Selected file | `.../shots/proto_file_selected.png` | `.../shots/app_file_selected.png` |
| Chart and table | `.../shots/proto_pane_both.png` | `.../shots/app_pane_both.png` |
| Table | `.../shots/proto_pane_table.png` | `.../shots/app_pane_table.png` |
| Values | `.../shots/proto_pane_value.png` | `.../shots/app_pane_values.png` |
| Play | `.../shots/proto_play.png` | `.../shots/app_play.png` |
| Settings, all | `.../shots/proto_settings_all.png` | `.../shots/app_settings_all.png` |
| Connect | `.../shots/proto_connect.png` | `.../shots/app_connect.png` |
| Status bar, overview on top | `.../shots/proto_perf_ovtop.png` | `.../shots/app_perf_ovtop.png` |
| No file / start page | `.../shots/proto_start.png` | `.../shots/app_empty.png` |

Prototype-only views: `proto_file_menu.png`, `proto_folder_menu.png`, `proto_explorer_more.png`, `proto_chan_multi.png`,
`proto_pane_rclick.png`, `proto_pane_cv.png`, `proto_pane_gauge.png`, `proto_pane_xy.png`, `proto_pane_spectrum.png`,
`proto_pane_checks.png`, `proto_rail_events.png`, `proto_rail_checks.png`, `proto_rail_layouts.png`,
`proto_rail_settings.png`, `proto_focus.png`, `proto_locked.png`.

**Same in both:** every colour token of the dark and light themes that both define, the UI and data fonts (IBM
Plex Sans and Mono), body 13 px, the rail (52 px wide, 38 px buttons, radius 8, 4 px gap), the top bar (44 px high,
6 px gap), tab padding (0 6 0 14 px, 13 px), the selected tab's underline, the note under the top bar, menu item
style, and the axis tick font.

**Different**, prototype first, then app:

1. Left sidebar width: 300 px, resizable 200–560 px · 260 px fixed.
2. Measure width: 260 px · 300 px.
3. Sidebar padding: 6 px 6 px 16 px, with panels inside · 0, content padded per block.
4. Panel title: chevron and caps title in a 25 px row, plus icons and ⇄ · caps title (11 px, muted) in a 40 px row, no chevron.
5. Explorer header: two rows (title icons; folder button, filter and ⋯) · two rows (title icons; full-width filter).
6. Filter input: 30 px high, 13 px, `--fg` text, radius 8 · 28 px, 13.33 px, pure white text (`#fff`), its frame drawn by the label.
7. Tree rows: 30 px, 13 px · 28 px, 12 px.
8. Tree indent: channels sit under a vertical guide line per level · no guide lines.
9. Plotted channel names: semibold · regular.
10. Checkbox: 14 px rounded swatch filled with the channel colour, white check when in the selected pane · filled box only, no check.
11. File rows: no size · size in MB, monospace, right-aligned.
12. Open-file chip: 24 px high, 13 px, radius 4 · 22 px high, 12 px, radius 8.
13. Pane header: 32 px high, padding 4 4 2 8, gap 4×6 px · 28 px, padding 0 8 0 10, gap 14 px.
14. Chips: 26 px high, 13 px, radius 4, padding 2 2 2 6 · 17 px high, 12 px, no padding or radius.
15. Chip width and spacing: 114 px per chip, with room kept for the value and ×, 6 px apart · 80 px, 14 px apart.
16. Pane number: accent and a grip when selected · muted, 11 px.
17. Pane ⋯ and view buttons: 26×26 px · 26×22 px; view button shows the view's icon and a chevron · a generic icon.
18. Plot background: none; the plot sits on `--bg` · filled with `--plot-bg` (`#11161e` dark, `#fff` light).
19. Pane height: fixed (150 px plots, 192 px panes for 3 rows) · stretches (240 px panes for 3 rows).
20. Y-axis ticks: 3 labels on a 150 px plot · 5 labels on a 200 px plot.
21. Time span text: "2.00 h visible" · "00:03:20.0 visible".
22. Range text: 3 decimals, a button with padding 1 4 and radius 4 · auto decimals, plain text.
23. Top bar right side: three groups with separators and an accent fill on pressed toggles · one group.
24. Add pane bar: + icon, text, and a ▾ split at the right · "+ Add pane" text in a dashed box.
25. Overview: 89 px (header 34 px with a select, range and switch) · 73 px (header 19 px with a link).
26. Overview traces: muted min/max band per channel with an event strip · coloured lines.
27. Measure: 2×2 tiles (radius 8, Δt tile highlighted) and a table · key-value rows.
28. Values tiles (both 26 px values in 166–169 px tiles, radius 8): unit beside the value, a colour dash before the name, "no limits", "Click to set limits", a "where" line above the tiles · unit under the value, a coloured left border, nothing else.
29. Table values: channel decimals · raw float precision.
30. Settings: switches · native checkboxes.
31. View menu: icon per item; check mark 14 px · no icons; the check mark SVG has no size and renders about 150 px wide (`app_view_menu.png`).
32. Menus: group headings (11 px caps, muted) and a header with × on the tab and + menus · no group headings in ⋯; no headers.
33. Axis dialog buttons: Reset axes as a bordered button, Done filled with the accent · Reset axes as a link, Done outlined.
34. Connect dialog: 680 px, two columns, kind cards with descriptions · 440 px, one column, segmented kinds.
35. Scrim: rgba(3,5,8,.45) with a fade · rgba(0,0,0,.25), no fade.
36. Status bar: 28 px, padding 6 16, gap 18 · 24 px, padding 0 14, gap 16.
37. Light theme overview: band in `--fg-muted` · lines that look lighter than the pane traces (`app_main_light.png`).
38. Narrow window: sidebars wrap into full-width rows · the sidebar keeps 260 px.
39. Tokens: the prototype defines `--good`, `--danger` and `--on-accent` · the app leaves all three out and uses `var(--danger, #ff6b78)` fallbacks (`shell/LiveStrip.svelte:71`, `shell/Connect.svelte:126`, `shell/Explorer.svelte:206`); the app adds `--cursor`, which the prototype also has.
40. Font sizes on screen: 11, 11.5, 12, 13, 15 px · 11, 11.5, 12, 13 and 13.33 px. Buttons, selects and inputs in the app don't inherit the UI font, so they render at the browser's 13.33 px (rail buttons, `.tb`, the filter, Settings selects). 13.33 px isn't a token.
41. Radii on screen: 4, 8 and 12 px · 4 and 8 px.
42. Focus ring: `:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px }` everywhere (`48`) · browser default, with `outline: none` on the filter and tree rows (`shell/Explorer.svelte:202`, `302`, `style.css:589`).
43. Motion: menus grow in (`popin`, 170 ms), dialogs rise (`dlgin`, 220 ms), the scrim fades, pressed controls shrink to .97 and darken, event and table flashes, all off under reduced motion (`225-232`, `929`) · only the view glide (which respects reduced motion) and a 0.12 s chevron turn.
44. Tooltips: one floating tip (12 px, radius 8, inverted colours, shadow) after 700 ms, at once on keyboard focus, warm for 1.5 s; Short mode shows only icon buttons, first sentence and shortcut, and retires after 3 uses; Full and Off (`bindTips 6200-6256`) · the native `title`, copied from `data-tip` (`app.ts:2038-2040`), always shown, never retired.
45. Icon set: the app's icons are copied from the prototype (`icons.ts`). Missing in the app: lock, events flag, chart/table/both view icons, split, duplicate, add-all, derive, categories, check shield, layouts, clear channels, remove panes.

### Keyboard shortcuts

| Key | Prototype | App | Status |
| --- | --- | --- | --- |
| B, P, C | Modes | Modes | Ported |
| F | Fit | Fit | Ported |
| Esc | Clear cursors, close menus, leave focus mode | Clear cursors, close menus, leave focus mode | Ported |
| Backspace | Back | — | Different (the app uses Alt+←) |
| Alt+← / Alt+→ | — | Back / forward | Different (the app follows the design) |
| ← → | Pan 15% | Pan 15% | Ported |
| + − ↑ ↓ | Zoom 0.7× / 1.43× | Zoom 0.7× / 1.43× | Ported |
| L | Lock | Lock | Ported |
| T | Theme | Theme | Ported |
| E | Event markers | — | Missing |
| A, B | Over a table row | Over a plot | Different |
| Space | Play / pause | Play / pause | Ported |
| Ctrl+B, Ctrl+Alt+B | Sidebars | Sidebars | Ported |
| Ctrl/Cmd+. | Focus mode | Focus mode | Ported |
| Ctrl/Cmd+D | Duplicate pane (Shift: beside) | Duplicate pane (Shift: beside) | Ported |
| Ctrl/Cmd+Shift+N | New pane below the selected one | New pane below the selected one | Ported |
| Ctrl/Cmd+S | Save layout | Save layout (a file tab asks for a name) | Ported |
| Ctrl+PageUp / PageDown | Switch tabs | — | Missing |
| Ctrl+Shift+PageUp / PageDown | Move the tab | — | Missing |
| Delete on a tab | Close it | — | Missing |
| Ctrl/Cmd+A in Explorer | Select all rows of a kind | — | Missing |
| Enter in Explorer | Open the file; with several rows, the primary action | Open the file, toggle the channel | Partial |
| ⌘O | — | Open recording… | Different (app only) |

---

## What to port first (recommendations)

The owner decides; this is the order I'd suggest.

**Make the shell look and feel like the prototype (Phases 1–3, already due):**

1. Shared styles. Make buttons, inputs and selects inherit the font, and add the global `:focus-visible` ring.
   Add `--good`, `--danger` and `--on-accent`. Add the menu, dialog, scrim and press animations with
   reduced-motion support. Fix the unsized check mark in the view menu.
2. Sizes and spacing. Swap the sidebar widths (left 300, Measure 260) and add the resize edges. Give panes the
   prototype's fixed heights and the chart height grip. Drop the plot fill. Make pane headers 32 px with 26 px
   chips and tree rows 30 px at 13 px. Show plotted channel names in semibold.
3. The tooltip system: the floating `data-tip` tip that retires after three uses, with Settings › Tooltips and its
   Reset.
4. The top bar's missing groups: Lock (L, the toast, the three lock settings), Remove every channel and Remove
   every pane with Undo, Clear cursors always present, the ⋯ menu's groups (Theme T, Focus mode Ctrl+.,
   Collapse all sidebar panels), and items moving into ⋯ on narrow windows. Then the time-format menu on the range
   with the Time format, Time decimals and Time labels settings, and the span written as a duration.
5. Keyboard: ← → pan, + − ↑ ↓ zoom, Ctrl+D, Ctrl+Shift+N, tab switching and moving, Delete on a tab. Decide
   Backspace against Alt+← (the design says Alt+←/→).
6. Sidebar panels: collapsible titles, drag to reorder or move sides, ⇄, Reset UI layout.
7. Explorer: channel categories with counts and hover actions, the swatch-and-check checkbox with "pane N",
   channel row actions, multi-select with the selection bar, dragging channels onto panes, pane edges and gaps,
   the file right-click menu, Show only these, Show files that can't be opened, Explorer's ⋯, the folder menu, and
   Undo on Close all files.
8. Pane stack: drop zones (edges, merge, between-row gaps, Alt to copy), the "+" between rows, chip × and drag
   and reorder, chip overflow, the view button's icon, the Add pane ▾ types, Duplicate beside, and the empty-pane
   and no-panes states.
9. Measure: the Cursors tiles and table, a Stats panel with a channel picker, RMS and Missing, and stats over the
   visible range when no cursors are placed. Values tiles and tables in the channel's decimals.
10. Overview: the channel picker, the min/max band drawing, the time range, a switch for Show on top, and
    gestures that follow the mouse mode.
11. Settings: the remaining Phase 3 rows (Clicking a row, Quiet interface, Show tips, When a file opens with
    "Files opened for the first time" and Forget, Scroll wheel zooms, Draw every sample, New pane from a dragged
    channel, the start page and recording-folder rows), as switches.
12. Start page and the rail logo, and then Live's remaining Phase 3 pieces: the session name, Pause, Stop and
    Remove per source, + Add source, Trigger, the live dots on the tab and rail, and the Connect dialog's Name,
    recording folder and saved connections.

**Finish the partly ported Phase 4 features:**

1. Tables: sort and filter in Rust with the filter row and strip, Columns, Autofit, the header ▾ menu, column
   selection, pinned Sample and Time, the cursor line, row right-click, row hover crosshair, the rows band on the
   charts, and copy as CSV.
2. The all-panes view switch in the top bar.
3. Workspaces: the tab menu's header, status and Save layout (always listed), the Layouts sidebar, the `.tlmbench`
   entry in Explorer, built-in starters, Undo on closing tabs, and per-folder sessions with Reset session.
4. Export data… and Save the view as an image… once the owner accepts them; the descriptor form and the CSV import
   step once the owner accepts the form.

Phase 5 items (events, rules and checks, widgets and gauges, derived channels, slots and several files per tab,
HTML export, the channel editor) stay where the roadmap puts them.
