# Release app test sheet (owner pass)

**How to fill this in (about 40 minutes).** Open the release app (`target/release/tlmbench-app`, or the installed one) and go
down the list. For each test type `PASS`, `FAIL` or `SKIP` after `Result:` (nothing else on that line), and when it isn't a
clean pass write what you saw after `Notes:` in plain words. Optional: add `Severity: blocker | annoying | cosmetic`.
Then use "Other things I noticed" at the end for anything the list didn't ask about. When done, hand this file to the next
agent (`docs/release/next-agent-prompt.md` says what it does with it). You don't need to explain a fix, only what you saw.

Test data: your own folder `/Users/rafy/workspace/test` (AAPL.csv, GOOG.csv, FB.csv, IBM.csv, example.csv, run.parquet) is
enough for most of it. Two checks want a file with a time gap or a live source; they say so.

```
Date:
App: release build from commit (agent fills in):
Mac / macOS version:
Display (scale, one or two monitors):
Opened as: (the built app / an installed copy)
Started with my usual settings, or a fresh profile?:
```

*For the agent reading this:* every `FAIL` and every non-empty `Notes:` is an item for the triage table. The IDs (`A1`,
`B3`…) are stable; cite them in commit messages and in the ledger row you add or update. `L` = ledger row it exercises.

---

## A. Open things and the Explorer

### A1. Open a folder
Do: Explorer › Open folder, choose `/Users/rafy/workspace/test`.
Expect: the folder name shows and its recordings (CSV, Parquet) are listed, other files hidden or greyed, with no blank list.
Result:
Notes:

### A2. "Show only these"
Do: Click the "Show only these" pill (Explorer header) with no file open, then open a file, then switch it off.
Expect: with nothing open the list is not just silently empty (known issue: it is empty today); with a file open only that
file shows; off shows everything again.
Result:
Notes:

### A3. Open a CSV by double-click, and with ⌘O
Do: Double-click AAPL.csv in Explorer. Then ⌘O and pick GOOG.csv.
Expect: the import dialog appears the first time for a new header, a chart appears with channels listed, the second file opens
in its own tab (or this tab, as the dialog says).
Result:
Notes:

### A4. Drop a file from Finder
Do: Drag a CSV from Finder onto the window. Then a folder. (L: X-win-01)
Expect: the file opens; the folder is listed in Explorer.
Result:
Notes:

### A5. Open a Parquet file
Do: Open `run.parquet`.
Expect: opens with channels and a chart, no import errors.
Result:
Notes:

### A6. Read-only folder
Do: Copy a CSV to a disk image or a read-only folder (Get Info › read only), open it. Try Share. (L: X-win-09)
Expect: everything works; nothing is written beside the file; Share offers Export instead of writing.
Result:
Notes:

## B. Reading a chart

### B1. Zoom, pan, fit
Do: Scroll to zoom, drag to pan, double-click to fit, ⌘Z to go back a view.
Expect: smooth, no flicker, the time axis labels stay readable.
Result:
Notes:

### B2. Cursors and Measure
Do: Click to place cursor A, then B. Open Measure (rail).
Expect: values at the cursors and stats between them; they match the numbers in the table pane.
Result:
Notes:

### B3. Line styles (L: X-win-05)
Do: On one channel set line width 3, dashed, then step.
Expect: each is drawn as set (headless tests can't see this).
Result:
Notes:

### B4. A file that is still building
Do: Open the biggest file you have (or `just gen --samples 100M --channels 1` and open it).
Expect: hatching while the pyramid builds, then the chart fills in; the min/max band shows on a spiky channel. (L: X-win-06)
Result:
Notes:

### B5. Table, values, XY, spectrum, gauges
Do: Switch a pane to each mode from its mode menu.
Expect: each shows data and responds to the cursor and the time view.
Result:
Notes:

### B6. Text and fonts
Do: Compare a screen of the app with the prototype (`prototype/plotter.html`, or the published copy) side by side.
Expect: same fonts, sizes, spacing; nothing clipped. (L: X-win-06)
Result:
Notes:

## C. Dragging and windows

### C1. Drag a channel to a pane, between panes, to a pane edge (L: X-win-03)
Result:
Notes:

### C2. Drag sidebar panels, tabs, and pane numbers to reorder (L: X-win-03)
Result:
Notes:

### C3. Drag a tab out to a new window, then onto another window's tab strip (L: X-win-04)
Expect: a window opens with that tab; dropping on the other strip joins it; quit and reopen puts windows back.
Result:
Notes:

## D. Several files in one tab

### D1. Add a second file to the tab
Do: With AAPL open, add GOOG to the tab (import dialog "This tab", or the tab menu).
Expect: channels from both, the second named `…·2` or by file name; the time axis spans both; alignment shown.
Result:
Notes:

### D2. Align
Do: Change the alignment (start, clock, nudge). 
Expect: the second file moves; nothing is rebuilt or slow; undo works.
Result:
Notes:

## E. Events, checks, derived channels

### E1. Add an event and see it on the chart and in the table
Result:
Notes:

### E2. Run a check set and click a failing cell
Do: Rules & checks › add a simple check (`Close > 0` should pass; `Close < 1` should fail), run it.
Expect: pass and fail show with counts; clicking a failure frames the first violation.
Result:
Notes:

### E3. Derived channel
Do: Add a derived channel (`Close * 2`) from the + in Explorer.
Expect: it appears as a channel, charts, and survives closing and reopening the file.
Result:
Notes:

## F. Export (what R2.1 added)  — the most important section

### F1. Export to HTML… (Full), opened from a web server
Do: ⋯ menu › Export to HTML…, choose a folder. In a terminal run `python3 -m http.server` inside the exported folder, open the
address in your browser.
Expect: the viewer opens with the same panes; the chart, table and cursors work; no editing buttons.
Result:
Notes:

### F2. Open that exported folder by double-clicking index.html
Expect: a short note that a folder export needs a web server and that `--single-file` opens from anywhere (not a blank page).
Result:
Notes:

### F3. One HTML file
Do: In a terminal: `target/release/tlmbench export html /Users/rafy/workspace/test/AAPL.csv --single-file --out ~/Desktop/aapl.html`
then double-click the file.
Expect: it opens straight from the Desktop with the chart, under 25 MB, nothing fetched from the network.
Result:
Notes:

### F4. Light mode and the raw-window fields
Do: Export to HTML… with Mode: Light.
Expect: three fields appear ("Raw around events", "At least (s)", "At most (s)"); changing one changes the command line shown
at the bottom of the dialog and is remembered next time.
Result:
Notes:

### F5. A Light export, in the viewer
Do: Export a big file as Light (or force a small budget with the CLI `--budget 2MB --mode light`), open it in the viewer.
Expect: charts draw and are labelled coarse where raw samples aren't in the export; zooming in to an event window is exact;
a table or a value at the cursor outside the windows is blank, never a made-up number.
Result:
Notes:

### F6. Two files in one export
Do: With two files in a tab (D1), Export to HTML…, choose "Every channel of the file", open the result in the viewer.
Expect: one tab with both files' channels, the second on the same timeline as in the app.
Result:
Notes:

### F7. A check report that links to the failure
Do: In a terminal, with a folder of CSVs and a failing set: `tlmbench check <folder> --set "<name>" --report html --views --out ~/Desktop/report`
then open `report.html` through a web server (as F1) and click a red cell.
Expect: the cell opens an export of that file, already zoomed on the first violation.
Result:
Notes:

### F8. Widgets of your own in an export
Do: Make an SVG widget (Settings › widgets) and a code widget, put each in a Gauges pane, export, open the viewer.
Expect: the SVG widget draws; the code widget shows as a plain value card; nothing runs.
Result:
Notes:

### F9. Redaction
Do: `tlmbench export html <file> --all-channels --redact Close --checks "<set>" --out ~/Desktop/r`
Expect: no `Close` data in the folder; a check that read it is gone from the report; open it and confirm.
Result:
Notes:

### F10. Other exports
Do: Export data… (CSV, Parquet) and Save the view as an image… (PNG, SVG).
Expect: each opens a native save dialog starting in a sensible folder; the files open correctly elsewhere. (L: X-win-02)
Result:
Notes:

## G. Live and plugins (skip what you don't use)

### G1. A growing file
Do: Live › a growing `.tlm` or CSV; append rows from a terminal.
Expect: the chart follows; Stop ends the recording; the file opens afterwards. (L: X-win-08)
Result:
Notes:

### G2. TCP or UDP source with a trigger
Result:
Notes:

### G3. A code widget in the app
Expect: asks once per folder before running shared code; then runs. (L: X-win-07)
Result:
Notes:

## H. Settings and look

### H1. Settings that stick
Do: Change theme, a chart option, quit, reopen.
Expect: everything configurable is remembered.
Result:
Notes:

### H2. Dark and light
Expect: both readable, no clipped or invisible controls.
Result:
Notes:

### H3. Tooltips
Expect: short, helpful, they stop appearing after three uses.
Result:
Notes:

### H4. Calm UI
Expect: controls appear on selection or hover, not all at once; frequent actions are one click.
Result:
Notes:

### H5. Memory and speed
Do: Open the 100M-sample file (B4). Watch the status bar memory number and the feel of zoom.
Expect: memory stays reasonable, zoom stays smooth.
Result:
Notes:

---

## Other things I noticed

Copy this block once per thing. Don't worry about being tidy.

```
What I did:
What I expected:
What happened instead:
Where (screen, tab, setting):
How often (always / sometimes / once):
Severity (blocker / annoying / cosmetic):
```

## Anything you want built or changed (new ideas)

New ideas go on a list for after the release unless you say they matter for v1 (`docs/post-v1.md`).

```
Idea:
Why it matters:
Needed for v1? (yes / no):
```
