# UI audits

One dated section per audit (`docs/release/release-plan.md`, "UI audits with the apple-design skill"). Findings are
ranked; each names the screenshot it comes from. The prototype and CLAUDE.md's UI conventions win over the skill: where
they conflict, or a finding would change the design, it goes to the owner with a recommendation. Kept until R8.

## Nov 14, 2026: end of R2.1a (the owner's first test-sheet list)

Scope: the screens that list touched: dialogs (Export to HTML, Rules and checks, Reset app), the export's progress and
end note, the top bar's Export menu, pane actions and grips, XY's header, Chart and values, the Explorer header, the
viewer's header. Screenshots: headless Chromium at 1440 × 900 against the built UI and the fake core, dark and light
(`dark-*.png`, `light-*.png` from a scratch script; not kept in the repo). The `anthropic-skills:apple-design` skill was
loaded and its headings used: response and feedback, spatial consistency, restraint and simplicity, typography, materials
and depth, motion and reduced motion.

### Fixed in this pass

1. **Feedback kinds: a finished export showed the warning dot** (`*-html-done.png`). Every top-bar note used the
   orange warning dot, so "Exported to … · Show in Finder" read like a problem. A note that reports success now uses
   the accent dot (`ui.noteOk`); failures keep the warning dot. `t_exporthtml`.
2. **Restraint and legibility: the Export menu's hints ran inline and were cut off** (`dark-export-menu.png`:
   "Export to HTML…A page anyone can open, with no…"). The three items are plain names; the hints are tooltips.
   `t_export`.
3. **Grouping: the progress bar sat at the far right, away from its label** (`dark-html-progress.png`). The bar and
   percentage now follow the label ("Exporting flight to HTML… ▬▬▬ 25%"). Screenshot.

### Checked, no change

- **Response.** The export dialog closes as soon as the folder is chosen, and the progress line appears in the same
  frame; pane actions, Duplicate, Split and Close act in one click.
- **Spatial consistency.** Every dialog opens centred near the top over a scrim and rises 8 px as it fades in (the
  prototype's), and closes the same way; the axis popover opens at its button. The Export menu opens from its button.
- **Calm UI.** Pane actions, XY's controls and the Events toggle show on hover or selection only (`*-main-hover.png`,
  `light-xy-cv.png`); the rarer ones give way when they don't fit. The viewer's "Read-only view" pill is the only new
  always-visible element and carries the export's provenance in muted text.
- **Typography and tokens.** Sizes in the new UI are 11.5, 12, 13 and 15 px; radii 4, 8 and 12; monospace only for the
  percentage, the command line and numbers.
- **Reduced motion.** The progress dot's pulse and the bar's width transition stop under `prefers-reduced-motion`.
- **Materials.** The app uses opaque surfaces with a scrim for modal dialogs, as the prototype does; translucent chrome
  is not part of its design, so the skill's material advice doesn't apply.

### For the owner (not changed: they would change the design)

- **The XY header's "Fit" when disabled** is a faint box (`light-xy-cv.png`), as the prototype's `.xyb:disabled`. The
  skill would hide a control that can't act. Recommendation: keep it (the prototype's, and it shows where Fit is).
- **Chart and values' grip at the pane's bottom** follows the owner (sheet B5), not the prototype (whose grip is under
  the plot). Recommendation: keep the owner's; note it for the design doc.

## Nov 15, 2026: R2.2, browser-only mode and the viewer (light and dark)

Scope: browser-only mode (the start page, the Explorer empty state, a CSV open with three panes) and the viewer over an
HTML export of the same file. Screenshots: headless Chromium at 1440 × 900 against `app/browser/dist` and a folder export
made by the release CLI, dark and light (`{dark,light}-browser-empty.png`, `-browser-open.png`, `-viewer.png` from a scratch
script; not kept in the repo). This audit covers what exists: R2.2's binary formats, derived channels, events, checks and
several files in a tab are not built yet (ledger S5.10-04), so their screens are not audited. Headings from the
`anthropic-skills:apple-design` skill: wayfinding, familiarity, simplicity, feedback, typography, materials, motion.

### Fixed in this pass

1. **Wayfinding: the start page and Explorer offered what a browser tab can't do** (`dark-browser-empty.png`). "Open folder…"
   was the primary button, "Connect live source…" sat beside it, the Explorer said "Open a folder" and "drop a CSV or .tlm
   file", and Connections listed "None saved". None of these can work with no app behind the page: a press did nothing. In
   browser-only mode the start page now offers Open recording… as the primary button, no folder or live buttons, no
   Connections section; the Explorer says "Choose a CSV file, or drop one on the page" and its header says "This browser"
   (a label, as the viewer's says "Export"). `t_browseronly`.

### Checked, no change

- **Responsiveness while a file loads.** The parse runs in a Worker; a 600,000-row CSV opens while the page keeps drawing
  frames (`t_browseronly`). The open shows the usual progress note.
- **The viewer is the app, with less** (`dark-viewer.png`, `light-viewer.png`): the same layout, the same tokens, no editing
  controls, a "Read-only view" pill with the export's file, date and version. Light and dark match the app's themes; plots
  read well in both (the series colours are the app's).
- **Typography and tokens.** Nothing new: sizes 11 to 15 px, radii 4, 8 and 12, monospace only for numbers and units.
- **Materials and motion.** Opaque surfaces and a scrim, as the prototype; no new motion. Reduced motion is unchanged.

### For the owner (not changed)

- **The viewer's provenance text is cut with an ellipsis at 1440 px** ("… TelemetryBench 0…."). The full text is in the pill's
  tooltip. Recommendation: keep; at narrower widths it must give way, and the version is the part that matters least.
- ~~Browser-only mode keeps the rail's Live, Rules and checks, and Layouts icons.~~ Closed Nov 15 by the owner's rule (sheet
  R12: every control acts, or is left out, or is disabled with the reason): Live is gone from the rail in a tab, and
  Settings leaves out browser access, plugins, live recording and shared configuration. Layouts and Rules & checks stay
  until R2.2 builds checks and layouts, then are checked again (ledger D-brow-05).

## Nov 15, 2026: the viewer, Locked and Unlocked (R12), light and dark

Scope: the viewer over a flight-1 export (a chart pane with two channels, a chart, a table), Locked and Unlocked, light and
dark, 1440 × 900 (`{light,dark}-{locked,unlocked}.png` from a scratch script; not kept in the repo). Headings from the
`anthropic-skills:apple-design` skill.

### Fixed in this pass

1. **Feedback: a control that looked live but only said "locked" afterwards.** The first build let every pane control
   through to a note. Locked now greys them with "Unlock the view to…" (chips and rows stop being draggable, the pane
   actions, the add-pane row, the height grips, the ⋯ menu), so the state is visible before a press, not after.
2. **Wayfinding: two padlocks.** The pill first carried a padlock beside the toolbar's "Lock the time view" padlock. The
   pill is text only ("Locked", "Unlocked"); the provenance (file, date, version) stays in its
   tooltip and in the muted text beside it.
3. **Simplicity: settings that could not apply.** Settings in a viewer showed rules for plugins, live recording, browser
   access and the like. They are left out (not greyed): a viewer shows only how it draws.

### Checked, no change

- **Familiarity.** Unlocked, the viewer is the app: the same drag, the same copy key, the same pane actions on hover.
- **Typography and tokens.** The pill is 12 px, radius 12; nothing new. **Materials, motion.** Opaque surfaces; no new motion.
- **Light and dark.** Both read well; the greyed controls keep their contrast above the disabled floor.

### For the owner

- **Whether an unlocked arrangement is kept across reloads** (sheet R12). Recommendation: no. A reload is the exported
  view, which is what a reader expects of a shared link, and nothing is stored. If you want it kept, it would go in that
  browser's storage only, with a "Back to the exported view" action.

## Nov 15, 2026: R3.0, the whole app against the prototype (dark, 1440 × 900)

Scope: every scene of `just app-parity` (the app on the fake core against `prototype/plotter.html`), read side by side for
the main view, start page, Connect, Import, the tab and ⋯ menus and the table; the README screenshots (`node
app/ui/tests/shots.js`, including the viewer and browser-only mode). The `anthropic-skills:apple-design` skill was loaded;
headings from its principles. Nothing changed in this pass: each finding is a Port row in `parity-classes.md`, where it
is ranked with the parity differences.

1. **Simplicity and hierarchy: the start page is a small panel over the workspace** (`start_app.png`). The tab and the
   Explorer behind it compete with it, and the three ways in are equal-weight buttons without saying what each opens. The
   prototype's full page with cards and two columns answers "where can I go" at a glance. P1.
2. **Restraint: the ⋯ menu is 19 items and scrolls at 900 px** (`more-menu_app_crop.png`), and half of it is now in the top
   bar's Export menu or the menu bar. P3.
3. **Restraint: every Explorer row shows its unit and pane** (`main_app.png`). Useful on hover, noise as a column of
   "pane 3 rpm" on every row. P2.
4. **Feedback: a mouse-opened menu shows the keyboard focus ring on its first item** (`tab-menu_app_crop.png`). It reads as
   a selection the user didn't make. P9.
5. **Craft, spatial consistency: the time range changes width as you zoom** (its decimals follow the zoom), so the text and
   its neighbours shift. P10.
6. **Familiarity: rates read as "1000.000 Hz" and "0.04167 /hour"** (table, import). People say "1 kHz" and "daily". P5, P6.
7. **Legibility: the viewer's tab cuts a long layout name at the left** ("ltitude and …", `app-viewer.png`). P11.

Checked, no change: dialogs share one frame, open centred over a scrim and close the way they came (B1); pane actions and
XY's controls appear on hover only (B5); sizes and radii stay on the tokens; reduced motion is honoured (earlier audits).

The owner approved every "App is better" row and decided P3 (trim the ⋯ menu) and P10 (fixed-width time range) as recommended, Nov 15.

## Nov 15, 2026: end of R3.1 and R3.2 (dark, 1440 × 900; run by Sonnet, the Opus pass is still owed)

Scope: the tab menu, the ⋯ menu, menu focus, the time range, the viewer's tab, the Layouts panel's rename, the Explorer's
file rows and channel rows, Open with layout ▸, the recipe dialogs, the start page. A screenshot of the start page was
read (`tests/out/start.png`); the rest from the code and the tests. Principles from `anthropic-skills:apple-design`.

1. **Restraint, done:** the ⋯ menu is View, All panes and Window (no Export, no bulk removals on the desktop app); a
   channel row is a name and a tick until the pointer is on it. Both fade with `prefers-reduced-motion` honoured.
2. **Feedback, done:** a menu opened with the mouse has no ring; an arrow key or Tab brings it back. Rename says why a name
   is refused in the field, not in a note.
3. **Spatial consistency, done:** the time range holds a 26 ch minimum with tabular numbers, so its neighbours stay put.
4. **Finding, left:** on the start page the Layouts rows are greyed with no file open, because a layout opens on the file
   in front. The tooltip says so, but a person with only a folder open may wonder. Recommendation: keep it; opening a
   file's right-click ▸ Open with layout is the other way in. Needs the owner's eye in the real window.
5. **Finding, left:** Open with layout ▸ swaps the file menu's list in place (a Back item first) instead of a flyout. It
   keeps arrow-key and focus behaviour simple; a real flyout would need Menu to nest. Owner pass: does it read as a submenu?
6. **Owner pass (real window):** the ⋯ height at 900 px, the focus ring after a click in WebKit (Tauri's engine decides
   `:focus-visible` differently from Chromium; the menu now sets it itself), the viewer tab's name, the start page at a
   narrow width.

## Nov 15, 2026: R2.2's folders and hidden controls, and R3.3's UI rows (run by Sonnet; the Opus pass is owed)

Scope: the browser page's start page, Explorer and menus; pane names; Open together; the pane and chip menus for several files;
the event tooltip; Stop at. From the code, the tests and the existing screenshots; no new screenshots of the browser page.

1. **Restraint, done:** a browser tab hides what comes after v1 (rail, Explorer, channel, file and tab menus, Events editing, ⋯, Reset
   app); nothing is greyed. The Events panel stays but is empty there: **question for the owner** (U2) whether it goes too.
2. **Honesty, done:** where the browser has no folder picker the start page and the Explorer say so, once, and keep Open recording….
3. **Feedback, done:** a pane's name is typed in the header (Enter keeps, Escape leaves, blur keeps); a pane's name and the event tip
   use the existing tokens (12 px, the tip's own style). Stop at keeps its menu open while categories are ticked.
4. **Finding, left:** Duplicate for file N, From file N and Add from every file are menu items (one per file), not the submenu the
   owner's wording has (Menu doesn't nest). Fine up to a few files; with many files the menu gets long.
5. **Owner pass (real window):** U1 to U5 on the sheet.

## Nov 16, 2026: end of R2.2, browser-only mode (Opus; light and dark, 1280 × 800, Chromium)

Scope: every screen browser-only mode has, after descriptors and packets (D-brow-08), WASM plugins (D-brow-09) and the
folder's `.tlmbench/` (D-brow-10): the start page, a folder in Explorer, the import dialog, a CSV open, the descriptor form on a
binary file, Settings › Plugins (the folder's plugin waiting, then trusted), and a file opened through a plugin. Screenshots
taken with a scratch Playwright script over `app/browser/dist`; the apple-design skill's headings (feedback, restraint,
honesty of copy, spatial consistency, materials) and CLAUDE.md's tokens.

### Fixed in this pass

1. **Wayfinding (ranked first):** a file the folder's plugin names showed "no descriptor" until the folder was trusted, which
   sends the person to the wrong fix. It now lists as openable, and opening it says the folder's plugin waits for their OK in
   Settings › Plugins (`t_browserfolder`).
2. **Feedback kinds:** a native plugin in the folder read in error red, with "runs with your rights", which isn't true in a tab.
   It is a status, not an error: muted, "Native", "Needs the app: a browser tab runs WASM plugins only" (`t_browserfolder`).
3. **Restraint:** the start page's right column (Live sources, Layouts) was half there in a tab: Layouts showed with nothing a
   tab can do with it. The column is left out in a tab; the subtitle no longer offers a live source and says the file stays in
   this browser.
4. **Copy:** Explorer's empty text said "Choose a CSV file" though the page now reads Parquet, `.tlm`, described binaries and
   plugin formats: "Choose a file". The descriptor form promised "the logger's live stream" in a tab, which has no live
   sources: left out there. Settings' Layout section said "app cache": "this browser".

### Checked, no change

- The descriptor form, the folder listing and the plugin rows use the app's own components, tokens and spacing in both themes;
  nothing clipped at 1280 × 800.
- The trust block in Settings › Plugins wraps to many lines in the sidebar's width (dark-6). It is the app's own wording and
  layout; the same in the app's sidebar. Left as it is.
- Opening a large plugin file shows the existing progress under the top bar; a plugin that runs too long ends with a note that
  says so (`t_browserplugin`).

### For the owner

- Firefox and Safari: plugins need Workers with `FileReaderSync` and module Workers (both there today); folders need
  `showDirectoryPicker` (Chromium only, as before). Real browsers: R5.
