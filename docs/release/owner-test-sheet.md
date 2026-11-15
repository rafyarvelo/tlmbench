# Release app test sheet (owner pass)

**How to fill this in (about 40 minutes).** Open the release app (`target/release/tlmbench-app`, or the installed one) and go
down the list. For each test type `PASS`, `FAIL` or `SKIP` after `Result:` (nothing else on that line), and when it isn't a
clean pass write what you saw after `Notes:` in plain words. Optional: add `Severity: blocker | annoying | cosmetic`.
Then use "Other things I noticed" at the end for anything the list didn't ask about. When done, hand this file to the next
agent (`docs/release/next-agent-prompt.md` says what it does with it). You don't need to explain a fix, only what you saw.

**Test data:** run `just testdata` (or `python3 tools/testdata/make.py`). It writes `~/tlmbench-testdata/` and never touches your
own folders:

| File | Use it for |
| --- | --- |
| `flights/flight-1.csv` | a good run, 60 s at 100 Hz (altitude, speed, battery_v, temp); every check passes |
| `flights/flight-2.csv` | the same channels an hour later: the second file in a tab (D1, D2, F6) |
| `flights/flight-bad.csv` | battery under 12 V for 10 s and a temperature spike: the checks fail here (F7, F9) |
| `flights/flight-gap.csv` | a 30 s hole in the times: gap breaks and "connect gaps" (F5) |
| `flights/.tlmbench/` | shared config: the check set "Flight limits", the layout "Altitude and battery", two widgets (an SVG dial and a code widget), two events on flight-1 (F8) |
| `grow.py` | `python3 ~/tlmbench-testdata/grow.py ~/tlmbench-testdata/live.csv` adds a row a second, for Live (G1) |
| `just testdata big` | also writes a 100M-sample `.tlm` (about 400 MB) under `big/`, for B4 and H5 (needs `cargo build --release -p tlmbench-gen` first) |

Open the folder `~/tlmbench-testdata/flights` in Explorer. Your own folder `/Users/rafy/workspace/test` (AAPL.csv, GOOG.csv, …,
run.parquet) is good for Parquet and for a messier look.

In the terminal commands below, replace `<file>` with `~/tlmbench-testdata/flights/flight-1.csv` and `<set>` with `Flight limits`.

```
Date:
App: release build from commit (agent fills in): the R3.1/R3.2 build of Nov 15, 2026 (`git log -1` when built), `target/release/tlmbench-app` (CLI: `target/release/tlmbench`). Everything you confirmed fixed or good is removed: what is here is still open.
Mac / macOS version:
Display (scale, one or two monitors):
Opened as: (the built app / an installed copy)
Started with my usual settings, or a fresh profile?:
```

*For the agent reading this:* every `FAIL` and every non-empty `Notes:` is an item for the triage table. The IDs (`A1`,
`B3`…) are stable; cite them in commit messages and in the ledger row you add or update. `L` = ledger row it exercises.

---

## R. Still open from the Nov 14 changes

R12 and B4 as changed on the Nov 15 build from the commit below: please re-test. 

### R12. An export's viewer: locked, with Unlock (F1)
You wrote: the exported view "isn't really 'read-only' since some of the panes 'duplicate' actions work, but the dragging
options don't work … is it possible to just allow exported views to have the user be able to move and drag channels like
the app. Having the exported view start 'locked' and having the user click unlock … Also the Events tab shows the rules
buttons but they don't work and there's no visual indication … either hide them or disable and put some UI indication."
Changed: the viewer opens **Locked** (the pill in the top bar; its tooltip keeps the file, date and app version). Click it to
**Unlock**: drag channels between panes (Option or ⌘ copies), from the Explorer, reorder panes by their number, split,
duplicate, close, resize, change a pane's view. Click again to lock. Nothing is written into the export; a reload is the
exported view again. Locked, those controls are greyed with "Unlock the view to…". Everything else either works or is left
out: the Events panel's Marker, Range and Rules are greyed and say an export is static; Settings shows only display
settings; the tab menu has no Save or Share.
Do: export flight-1 to HTML (One file), open it. Try dragging a channel while Locked, then Unlock and drag it, with Option
held, split, duplicate, close, resize. Open the Events panel and Settings. Open it in Safari too if you can.
Expect: locked nothing moves and the greyed controls say why; unlocked it behaves like the app; no button that does nothing.
You decided (Nov 15): an unlocked arrangement is not kept when you reload; a reload is the exported view.
Result:
Notes:

---

## S. New in the Nov 15 build (your decisions 7 and 8, and browser-only mode)

For the Nov 15 build only. `Result:` and `Notes:` as above. Browser-only mode needs the page served: run
`python3 -m http.server --directory app/browser/dist` from the repository and open http://localhost:8000 (S4).

### S1. Export to HTML: Folder or One file (decision 7, F2)
You said: "The user should be able to choose between a full export with a server and a standalone HTML / static folder."
Changed: Export to HTML… has "Save as": Folder (any size; needs a web server or a host) or One file (opens from anywhere),
each with its size. One file has a Size limit field (25 MB to start) and the command line shown at the bottom follows.
Do: Export to HTML… on flight-1: look at the Save as list and the summary line; choose One file, Export…, save it on the
Desktop, double-click it. Then choose One file with the limit at 1 MB and look at what the dialog says. Then a Folder.
Expect: One file asks for a file name (not a folder), writes one `.html` that opens from the Desktop with the chart and
nothing fetched; with every raw sample in it when it fits (the end note says "Full"); at 1 MB it says Light and why, and
that a folder has no limit. The Folder choice works as before. Your choice and the limit are there next time.
Result:
Notes:

### S2. The viewer's coarse plots (F3)
You said: "the 'coarse' plots are way too coarse… they should look more like the prototype."
Changed: a small export now carries every raw sample (S1), so it is not coarse at all. When it is coarse (Light, a trimmed
export), the viewer draws the same min/max band under the line that the app does.
Do: (a) flight-1 as One file: zoom in and out in the viewer. (b) Make it coarse:
`target/release/tlmbench export html <file> --layout "Altitude and battery" --mode light --detail 0 --out ~/Desktop/coarse`
(the layout's charts then keep only summaries), serve it (`python3 -m http.server --directory ~/Desktop/coarse`), and compare the viewer with the app over the same file at the
same zoom.
Expect: (a) smooth, exact lines, no "coarse" label. (b) a "coarse" label, and the band and line look like the app's.
Result:
Notes:

### S3. The menu bar (decision 8, R15 idea 1)
Changed: File, Edit, View, Pane, Window and Help, all from one list that also drives the shortcuts. Shortcuts with ⌘ are
bound in the menu; single keys (E, L, T, F, Esc, Backspace) are shown in the item's name, like "Fit to the data (F)", and
still work from the keyboard only. Cut, Copy, Paste and Select All are the system's.
Do: Open each menu. Try: Open recording… (⌘O), Open folder… (⇧⌘O), Save layout (⌘S), Undo and Redo (⌘Z, ⇧⌘Z), Duplicate
pane (⌘D), New pane (⇧⌘N), Settings… (⌘,), Focus mode (⌘.), and a click on Event markers and Lock the time view (the check
mark should follow). With nothing open, look at what is greyed out. Click in a text field (the Explorer's Filter) and use
⌘Z, ⌘C, ⌘V there. Open a Table pane, select cells, press ⌘C and paste into another app. Click in the Explorer and press ⌘A.
Expect: each does what its name says, once (not twice); items that can't act are greyed; text fields keep their own undo,
copy and paste; the Table's ⌘C and the Explorer's ⌘A still work. With two windows open, a menu click acts on the window in
front. Is anything missing that you expected in a menu?
Result:
Notes:

### S4. Browser-only mode (R2.2, partly built)
You said: "I expected Browser support to be almost analogous to what the prototype does: open folders and CSV files."
Changed so far: a CSV is read in a background thread (a big file doesn't freeze the tab); a reload brings the open files
back; the page no longer offers folders or live sources, which a tab can't do. **New (Nov 15, later build): a `.tlm` and a
Parquet file open** (Parquet: snappy, LZ4, gzip, brotli and Zstandard; its reader is a separate download of about 1 MB that
the page fetches only when you open a Parquet file). Not built yet: packets, descriptors, WASM plugins and folders
(next). Several files, derived channels, events and checks in a tab come after v1 (your scope, Nov 15).
Do: Serve the page (see above). Choose or drop `flights/flight-1.csv`. Then `browser/bench.tlm` (200,000 samples; `just
testdata` writes it, and `browser/codecs-snappy.parquet` and `codecs-zstd.parquet` if pyarrow is installed). Open a
Parquet file and look at the Network tab of the browser's developer tools: `tlmbench_parquet.wasm` loads only then. Drop a
bigger CSV (`/Users/rafy/workspace/test` has some; or make a 500,000-row one). Reload the page. Drop a file that isn't
one of these. Look at it in light and dark.
Expect: each opens and draws like the app's (the `.tlm` has altitude in m and battery in V, the time from its own field;
the Parquet file has speed in m/s and Temp in °C, its text column left out); while a big CSV loads, the page still
responds; after a reload the files are back with no picker; the start page offers only Open recording…; a file it can't
read says browser-only mode reads CSV, Parquet and .tlm files.
Result:
Notes:

---

## T. R3.1 and R3.2 (Nov 15 build): what you haven't checked yet

You confirmed (removed): the ⋯ menu, the time range, Explorer's quiet rows, Open with layout ▸, the start page, rates and
filters. Left:

### T1. Slot numbers after closing a tab (fixed Nov 15, rebuilt)
You wrote: the numbers stay in Explorer after closing a tab made with Open together or Compare.
Changed: closing the last tab (the window empties) left the slot list behind; it is cleared now.
Do: Open together on flight-1 and flight-2, close the tab; also Compare, then close; also with another tab open.
Expect: no slot numbers or "primary" on any file row once its tab is gone.
Result:
Notes:

### T2. The tab menu: Rename… and Save layout / Revert
Do: ⌄ on a file tab: Save layout and Revert are listed, greyed. Save as… "A", then Rename… to "B"; also double-click the
name in the Layouts panel. Try a name already in use.
Expect: the tab and the list follow the new name; a used name is refused in the field; Undo in the note puts it back.
Result:
Notes:

### T3. Duplicate recipe… and Save as new
Do: make a derived channel (±2σ or a delta); right-click it ▸ Duplicate recipe…; then Edit recipe…, change the
expression, Save as new.
Expect: the first opens the Derive dialog filled in and named "… copy"; the second adds a recipe and leaves the first as it was.
Result:
Notes:

### T4. The viewer's tab name
Do: export flight-bad to HTML as One file with the layout "Altitude and battery"; open it.
Expect: the tab name reads from its start, cut with an ellipsis at the end.
Result:
Notes:

### T5. The start page, narrow
Do: make the window as narrow as it goes, with a file open (the logo opens it) and with none.
Expect: the cards and columns stack, nothing clipped; Layouts rows are greyed with no file open and say why on hover.
Result:
Notes:

---

## B. Reading a chart

### B4. A file that is still building
Why it showed nothing: on this Mac the 100M-sample file builds its pyramid in about a second, so the hatching was gone before
you could look. The app now has a slow-motion build for looking at it: `TLMBENCH_SLOW_BUILD=<ms>` waits that long after each
block of about a million samples (100 ms makes the 100M file take about ten seconds).
Do: quit the app. In a terminal: `target/release/tlmbench cache clear` (a file opened before has its pyramid in the cache and
opens at once), then
`TLMBENCH_SLOW_BUILD=100 target/release/tlmbench-app ~/tlmbench-testdata/big/big-100M.tlm`
(`just testdata big` first if the file isn't there).
Expect (seen in the release window on Nov 15: hatching right of the built part, "Reading 27%" in the middle of it, the overview hatched too): for about ten seconds the chart is hatched where it isn't built yet and fills in left to right; the min/max band shows
on a spiky channel; "coarse" shows after 0.3 s; afterwards zoom is smooth. Without the variable it still opens in about a second. (L: X-win-06, X-win-11)
Result:
Notes:

### B6. Text and fonts
Do: Compare a screen of the app with the prototype (`prototype/plotter.html`, or the published copy) side by side.
Expect: same fonts, sizes, spacing; nothing clipped. (L: X-win-06)
Result:
Notes:

## C. Dragging and windows

### C2. Drag sidebar panels, tabs, and pane numbers to reorder (L: X-win-03)
Result:
Notes:

### C3. Drag a tab out to a new window, then onto another window's tab strip (L: X-win-04)
Expect: a window opens with that tab; dropping on the other strip joins it; quit and reopen puts windows back.
Result:
Notes:

## D. Several files in one tab

### D1. Add a second file to the tab
Do: With flight-1 open, add flight-2 to the tab (import dialog "This tab", or the tab menu).
Expect: channels from both, the second named `…·2` or by file name; the time axis spans both; alignment shown.
Result:
Notes:

### D2. Align
Do: Change the alignment (start, clock, nudge). 
Expect: the second file moves; nothing is rebuilt or slow; undo works.
Result:
Notes:

## F. Export (what R2.1 added): the most important section

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
Do: In a terminal, with a folder of CSVs and a failing set: `target/release/tlmbench check ~/tlmbench-testdata/flights --set "Flight limits" --report html --views --out ~/Desktop/report`
then open `report.html` through a web server (`python3 -m http.server --directory ~/Desktop/report`) and click a red cell.
Expect: the cell opens an export of that file, already zoomed on the first violation.
Result:
Notes:

### F8. Widgets of your own in an export
Do: Make an SVG widget (Settings › widgets) and a code widget, put each in a Gauges pane, export, open the viewer.
Expect: the SVG widget draws; the code widget shows as a plain value card; nothing runs.
Result:
Notes:

### F9. Redaction
Do: `target/release/tlmbench export html <file> --all-channels --redact battery_v --checks "Flight limits" --out ~/Desktop/r`
Expect: no `battery_v` data in the folder; the check "Battery above 12 V" is gone from the summary; open it through a web server and confirm.
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

```
What I did: Open together on two files, and Compare (Nov 15, release build from 01c8bb4)
What I expected: pick channels from both files in the Explorer
What happened instead: the Explorer only lets me select channels for the first file
Where (screen, tab, setting): Explorer, a tab with two files
How often (always / sometimes / once):
Severity (blocker / annoying / cosmetic): high priority (owner)
```
(Agent: you confirmed it works; the confusion is that only file 1 is marked. Ledger D-ui-45 (mark every file, slot and
alignment) and D-ui-46 (Open together draws every file's starters).)

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
