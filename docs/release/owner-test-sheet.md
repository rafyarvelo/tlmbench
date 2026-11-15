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
| `flights/flight-bad.csv` | battery under 12 V for 10 s and a temperature spike: the checks fail here (E2, F7, F9) |
| `flights/flight-gap.csv` | a 30 s hole in the times: gap breaks and "connect gaps" (B1, F5) |
| `flights/.tlmbench/` | shared config: the check set "Flight limits", the layout "Altitude and battery", two widgets (an SVG dial and a code widget), two events on flight-1 (E1, F8) |
| `grow.py` | `python3 ~/tlmbench-testdata/grow.py ~/tlmbench-testdata/live.csv` adds a row a second, for Live (G1) |
| `just testdata big` | also writes a 100M-sample `.tlm` (about 400 MB) under `big/`, for B4 and H5 (needs `cargo build --release -p tlmbench-gen` first) |

Open the folder `~/tlmbench-testdata/flights` in Explorer. Your own folder `/Users/rafy/workspace/test` (AAPL.csv, GOOG.csv, …,
run.parquet) is good for A5 (Parquet) and for a messier look.

In the terminal commands below, replace `<file>` with `~/tlmbench-testdata/flights/flight-1.csv` and `<set>` with `Flight limits`.

```
Date:
App: release build from commit (agent fills in): 2778484 (Nov 14, 2026), `target/release/tlmbench-app`. Re-test list: docs/release/next-agent-prompt.md, "The owner's re-test list"
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
Result: As expected.
Notes: Works fine.

### A2. "Show only these"
Do: Click the "Show only these" pill (Explorer header) with no file open, then open a file, then switch it off.
Expect: with nothing open the list is not just silently empty (known issue: it is empty today); with a file open only that
file shows; off shows everything again.
Result: "Show only these" only shows up after opening a file, it's not part of the explorer header that is always visible like the expand all and collapse all buttons
Notes: I recommend this option be always visible as an icon as needed to minimize confusion.

### A3. Open a CSV by double-click, and with ⌘O
Do: Double-click AAPL.csv in Explorer. Then ⌘O and pick GOOG.csv.
Expect: the import dialog appears the first time for a new header, a chart appears with channels listed, the second file opens
in its own tab (or this tab, as the dialog says). Clear cache keeps import profiles: to see the dialog again for a file you
opened before, delete its profile, `~/Library/Caches/tlmbench/profiles/<key>.json` (Reset app… in Settings will do this once built).
Result: The file opened right away because I had opened it previously. Clearing the cache didn't change the profile.
Notes: There should be a clear way for the user to "Reset to default settings" and clear all of the cache. A warning dialog is appropriate as needed.

### A4. Drop a file from Finder
Do: Drag a CSV from Finder onto the window. Then a folder. (L: X-win-01)
Expect: the file opens; the folder is listed in Explorer.
Result: Dropping a file works fine, but dropping a folder (~/tlmbench-testdata) gave the error: "Couldn't open tlmbench-testdata: /Users/rafy/tlmbench-testdata: Is a directory (os error 21)
Notes: When dropping a folder, it should be equivalent through opening the folder through the "Open folder..." action

### A5. Open a Parquet file
Do: Open `run.parquet`.
Expect: opens with channels and a chart, no import errors.
Result: File parsed and opened panes as expected.
Notes: Nominal behavior.

### A6. Read-only folder
Do: Copy a CSV to a read-only folder and open that folder in Explorer, then the CSV. Try the tab menu's Share and Explorer's folder menu › Share…. (L: X-win-09)
To make the folder read-only: Finder › Get Info › Sharing & Permissions › your name: Read only, or `chmod -R a-w <folder>`
(undo with `chmod -R u+w <folder>`). `just ro-test-mac` also makes a read-only disk image.
Expect: everything works; nothing is written beside the file; Share offers Export instead of writing.
Result: Got a "Couldn't share" error, no option to export was offered
Notes: This feature appears to be broken.

## B. Reading a chart

### B1. Zoom, pan, fit
Do: Scroll to zoom, drag to pan, double-click to fit, Backspace (or Previous view) to go back a view. (⌘Z is undo, not "back a view": decision of Nov 14.)
Expect: smooth, no flicker, the time axis labels stay readable.
Result: Zoom, pan, and fit work as expected.
Notes:  ⌘Z did not work, but the Backspace key and "Previous view" button work to go back to a view

### B2. Cursors and Measure
Do: Click to place cursor A, then B. Open Measure (rail).
Expect: values at the cursors and stats between them; they match the numbers in the table pane.
Result: The cursors and measure pane worked as expected.
Notes: Nominal behavior.

### B3. Line styles (L: X-win-05)
Do: On one channel set line width 3, dashed, then step.
Expect: each is drawn as set (headless tests can't see this).
Result: This appears to work fine.
Notes: Is there a global setting for the default? If not, there should be one in case a user want to make all channels "thin" or "heavy"

### B4. A file that is still building
Do: Settings › Cache › Clear cache first (a file opened before has its pyramid in the cache and opens at once). Then open the 100M-sample file (`just testdata big`, then `~/tlmbench-testdata/big/big-100M.tlm`).
Expect: hatching while the pyramid builds, then the chart fills in; the min/max band shows on a spiky channel. (L: X-win-06)
Result: The file opened pretty much instantly, so I couldn't really tell....
Notes: Not verified, need a better test case

### B5. Table, values, XY, spectrum, gauges
Do: Switch a pane to each mode from its mode menu.
Expect: each shows data and responds to the cursor and the time view.
Result: The widget types all seem to work, but the "..." menu from the gauges doesn't match the UI of the prototype
Notes: Resizing a "Chart and values" pane or "Chart and table" pane is awkward because the handle is in the middle of the pane on the bottom of the chart instead of the bottom of the pane below the widget / table. The "XY Plot" kind of works but the header is messed up and it's pretty confusing honestly, maybe needs a tooltip or something, resizing for the "XY Plot" doesn't seem to work at all.

### B6. Text and fonts
Do: Compare a screen of the app with the prototype (`prototype/plotter.html`, or the published copy) side by side.
Expect: same fonts, sizes, spacing; nothing clipped. (L: X-win-06)
Result:
Notes:

## C. Dragging and windows

### C1. Drag a channel to a pane, between panes, to a pane edge (L: X-win-03)
Result: Works but doesn't properly support the Ctrl or Cmd keys
Notes: Dragging a channel while holding Ctrl or Cmd is supposed to "Copy" the channel to another pane instead of moving it.

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

## E. Events, checks, derived channels

### E1. Add an event and see it on the chart and in the table
Result: Events and checks work as expected.
Notes: The UI is not that intuitive for Rules and checks, need a better explanation of what each one does. Also might help to allow users to easily enable events for a single pane, right now it seems like the only way to do that would be to manually enable the global events switch and toggle off every pane you don't want (not ideal).

### E2. Run a check set and click a failing cell
Do: With `flights` open as the folder, Rules & checks › the "Flight limits" set (it is shared in the folder) › run it on flight-bad.csv.
Expect: pass and fail show with counts; clicking a failure frames the first violation.
Result: This works fine, but the rules and checks dialog has the same issue as the other dialogs, it's clamped to the top left corner.
Notes: Is it possible to edit the Colors for Check categories? it should be.

### E3. Derived channel
Do: Add a derived channel (`altitude * 2`) from the + in Explorer.
Expect: it appears as a channel, charts, and survives closing and reopening the file.
Result: Works as expected.
Notes: Nominal behavior.

## F. Export (what R2.1 added)  — the most important section

### F1. Export to HTML… (Full), opened from a web server
Do: ⋯ menu › Export to HTML…, choose a folder. In a terminal run `python3 -m http.server` inside the exported folder, open the
address in your browser.
Expect: the viewer opens with the same panes; the chart, table and cursors work; no editing buttons.
Result: It worked as advertised, but the UI was disappointing and it was not obvious that the export is read only.
Notes: Export to HTML dialog is clamped to the top-left corner, it should be in the middle of the screen. Also the export icon should be way easier to find... it's buried behind a bunch of menus. The visual feedback was also weak... it should have better contrast and be more professional than plaintext on the dialog. Also the dialog didn't close immediately after

### F2. Open that exported folder by double-clicking index.html
Expect: a short note that a folder export needs a web server and that `--single-file` opens from anywhere (not a blank page).
Result: The note appeared as expected, but it didn't provide the exact python command needed so I had to read the usage and pass the --directory flag
Notes: Is the web server required? The User should be able to choose between a full export with a server and a standalone HTML / static folder when exporting. Consider perhaps different tiers. The "Full" version should be larger to allow for more fidelity as needed. I expected "Browser support" to be almost analogous to what the prototype does now how it allows you to open folders and CSV files. If the prototype can open folders and give pretty much all the features in a browser, the production app should be able to as well.

### F3. One HTML file
Do: In a terminal: `target/release/tlmbench export html ~/tlmbench-testdata/flights/flight-1.csv --single-file --out ~/Desktop/flight-1.html`
then double-click the file.
Expect: it opens straight from the Desktop with the chart, under 25 MB, nothing fetched from the network.
Result: It works, but the "coarse" plots are way too coarse... they should look more like the prototype right?
Notes: The HTML support in the current app is disappointing at best.

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

Copy this block once per thing. Don't worry about being tidy.

```
What I did: Opened a table
What I expected: Dragging the columns onto another pane would work same as dropping channels on other panes (same as prototype)
What happened instead: Table columns are not draggable
Where (screen, tab, setting): main screen
How often (always / sometimes / once): always
Severity (blocker / annoying / cosmetic): blocker
```

## Anything you want built or changed (new ideas)

New ideas go on a list for after the release unless you say they matter for v1 (`docs/post-v1.md`).

```
Idea: The Mac toolbar menu should include "File > Open", and all of the actions that the app uses. Right now, File only has a "Close Window" option
Why it matters: Expected from a professional app
Needed for v1? (yes / no): yes
```

```
Idea: There should be a file system watcher and / or a button to refress the file explorer
Why it matters: There's no indication to the user if the folder is up to date in the view
Needed for v1? (yes / no): yes
```

```
Idea: I like that the switcher only shows up when hovering over a pane. I think all the icons that can fit should show up on the pane when hovering over it so it's quicker for the user.
Why it matters: Clicking on "..." takes too much time to do actions on Panes
Needed for v1? (yes / no): yes
```