# Follow-ups: design-doc notes owed

Everything this file held up to Nov 24 was folded into the design doc's "Final sync (Nov 24, 2026)" section (in Claude Docs,
exported to `docs/design.md`; the older text is in git history: `git show 86ae974:docs/follow-ups.md`). The same sync moved four
decisions from Proposed to Accepted (the descriptor form, `files` patterns, `[packets]` without a length, the app's Export; the
owner's Nov 7 calls, which had only been made in the repository's copy) and added the `just gate4` result.

New notes go here, with the date, when behaviour differs from the design; they are folded at the next export. This file is
deleted at the release tag (R8).

- **Nov 25, new panes (owner, rc.1 test plan 3.5).** Add pane (under the stack, and its ▾ kinds), ⋯ › New pane and ⌘⇧N
  add an empty pane below the selected pane's row (at the bottom when none is selected), and select it; before, Add
  pane and ⋯ New pane always added at the bottom. ⋯ › All panes has New pane in the desktop app again (the prototype's
  group); the design's Panes bar line ("Pane actions (new, clear channels, remove all) live in the Explorer's Panes
  bar") and the ⋯ menu line ("New pane, Remove every channel or pane and Export are the menu bar's and the top bar's")
  need updating to say where these are now.

- **Nov 25, Explorer row actions (owner, rc.1 test plan 3.5: match the prototype).** The selected channel row (while
  pointed at), the open file's row and, on hover, a category row show the prototype's icons over the row's end: add to
  the selected pane, a new chart, a new table, and for a plotted channel remove from every pane. Every "open in a new
  chart / table" (the selection bar, Explorer ⋯, a table's selected columns, a check's row) now puts the new pane below
  the selected one, not at the end. The prototype's "Clicking a row" setting is not in the app (Select first is how the
  app always works). The design's Explorer text could say so.
- **Nov 26, Install command-line tool (owner, rc.1 test plan 1.9).** When the tool's folder isn't on PATH, the line to
  add is shown in a small dialog that stays until Done, read-only and selected, with Copy (it was a note that went in
  12 s). The design's R4 text could say so.
- **Nov 26, XY wheel zoom (owner, rc.1 test plan 3.6).** The XY pane's wheel zooms as much as the wheel moved, as the
  time charts do (and follows Settings › Zoom speed); it was 25% for every event, far too fast on a trackpad.
- **Nov 26, theme on drawn panes (owner, rc.1 test plan 3.7).** The XY and Spectrum panes repaint when the theme
  changes (it showed only when they were next drawn for another reason). A bug fix; nothing for the design.
- **Nov 26, Loop A–B (owner, rc.1 test plan 3.4).** While looping, a playhead outside A–B goes to A at once (turned on
  while playing past B, it looped only after a pause and play). A bug fix; nothing for the design.
- **Nov 26, readouts over days (owner, on a daily file).** In Automatic, a readout of an absolute time over a view of
  more than a day says the date first (`2026-01-01 00:00:00 → 2027-12-31 00:00:00`): the time of day alone read
  `00:00:00 → 00:00:00` on daily data. The design's time format text could say so.
- **Nov 26, the version (owner).** Settings ends with `TelemetryBench <version>` (the workspace's version, built in),
  in the app, browser-only mode and the viewer's builds: the copy on GitHub Pages didn't say which build it was. The
  design's Settings text could list it.
- **Nov 27, recipes reach open tabs (owner, rc.3 test plan 5.3; for rc.4).** A recipe kept for files with this import
  profile, or for any file with its channels, is to be computed for every open tab it applies to when it's saved (today
  only the tab in front, the others on reopening). Its channel joins their channel lists; their panes stay. The design's
  Derived channels text could say so once it's built.
- **Nov 26, table sort and filter in the browser builds (D-ui-62).** In browser-only mode and in an HTML export's
  viewer, a table's sort, filter, ▲ ▼ and Apply to charts did nothing: the page's core (`crates/web`) had none of the
  table query commands. It now answers them as the app does, and Apply to charts leaves the rows out of the page's
  charts, statistics, values and XY. A bug fix; nothing for the design.
