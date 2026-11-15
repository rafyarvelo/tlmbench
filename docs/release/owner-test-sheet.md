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
App: release build from commit (agent fills in): f9adbf3 (Nov 15, 2026), `target/release/tlmbench-app` (CLI: `target/release/tlmbench`). What is new since the Nov 14 build is section S below. Items you confirmed good are removed.
Mac / macOS version:
Display (scale, one or two monitors):
Opened as: (the built app / an installed copy)
Started with my usual settings, or a fresh profile?:
```

*For the agent reading this:* every `FAIL` and every non-empty `Notes:` is an item for the triage table. The IDs (`A1`,
`B3`…) are stable; cite them in commit messages and in the ledger row you add or update. `L` = ledger row it exercises.

---

## R. Confirm the Nov 14 changes against your comments

Build `f9adbf3` (`target/release/tlmbench-app`). What you confirmed good (R1, R2, R4 to R11, and A1, A2, A4 to A6, B1 to B3, B5, C1, E1 to E3) is removed; what is left is open. Each item quotes what you wrote, says what changed and how to check it.
Type `PASS` or `FAIL` after `Result:` as above; under `Notes:` say what still isn't right, in your words. Your original
answers in sections A to H stay as they were.

### R3. Reset to defaults and clear everything (A3)
You wrote: "There should be a clear way for the user to 'Reset to default settings' and clear all of the cache. A warning
dialog is appropriate."
Changed: Settings › Reset app… lists what goes (settings, import profiles, recipes, rules and checks, layouts, your events,
widgets, alignments, the cache), all ticked; untick what to keep. Settings › Import profiles lists each profile with a
button to forget one. Clear cache's tooltip says what it keeps.
Check: open a CSV you opened before; in Settings forget its profile (or Reset app… with Import profiles ticked); open it
again: the import dialog comes back.
Result: This works better now, but it's not clear on the difference between "Reset app..." and "Reset to defaults"
Notes: Is "Reset to defaults" needed anymore or is it redundant?
Changed since (Nov 15, after your answer): Reset to defaults is gone. Reset app… has one more choice, "Things the app
remembers" (recent files, each file's panes and channel display, the sidebars, export choices, saved connections),
unticked to start; with only Settings ticked it does what Reset to defaults did. Needs a rebuild of the app to see.
Result:
Notes:

### R12. Export to HTML: findable, feedback, read-only (F1)
You wrote: "the export icon should be way easier to find … The visual feedback was also weak … the dialog didn't close
immediately after … it was not obvious that the export is read only."
Changed (decision 6): an Export button in the top bar (HTML, data, image). Export closes the dialog at once and shows a
progress bar under the top bar, then "Exported to …" with Show in Finder. An export's viewer says "Read-only view" with the
file, the date and the app version.
Check: export flight-1 to HTML, then open it through a web server.
Result: The share icon is good, and the visual feedback is a little better, but the exported view isn't really "read-only" since some of the panes "duplicate" actions work, but the dragging options don't work so it's awkward.
Notes: Since the prototype shows dragging works fine in html, is it possible to just allow exported views to have the user be able to move and drag channels like that app. Having the exported view start "locked" and having the user click unlock is probably a good idea. There doesn't have to be 100% parity in the browser version, but clean it up so it's consistent at least; it doesn't make sense to make the view read-only when it clearly at least works allowing the user to rearrange panes. Also the Events tab shows the rules buttons but they don't work and there's no visual indication to the user as to why, either hide them or disable and put some UI indication as to why.

### R13. The web server note (F2)
You wrote: "it didn't provide the exact python command needed."
Changed: opened from a file, a folder export shows the exact command for its folder with a Copy button, the address to
open, and that One file opens anywhere.
Check: double-click the exported folder's `index.html`.
Result: This works fine now, the instructions provide the correct command
Notes:
The export tiers (decision 7) are built: see S1 and S2. Opening folders and CSVs in a browser like the prototype is R2.2,
partly built: see S4.

### R14. Dragging table columns ("Other" #1)
You wrote: "Dragging the columns onto another pane would work same as dropping channels on other panes."
Changed: a column header drags like a chip: onto another pane to move it (⌘ or Option copies), onto another header to
reorder.
Check: switch a pane to Table and drag a column header onto another pane.
Result:
Notes:

### R15. Your v1 ideas
You wrote: (1) the menu bar should have File › Open and every action; (2) a file system watcher or a refresh in Explorer;
(3) all the icons that fit should show on a pane when hovering.
Changed: (2) Explorer follows its folder (new and removed files appear by themselves) and the folder menu has Refresh, for
network shares. (3) On hover a pane shows, in the prototype's order: view, axis settings, Events, Duplicate, Split, ⋯ and
Close; the ones that don't fit stay in ⋯. (1) The menu bar is built now: see S3.
Check: add a CSV to the open folder from Finder; hover a pane, then make panes narrow (four in a row).
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
(the layout's charts then keep only summaries), serve it as F1, and compare the viewer with the app over the same file at the
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
back; the page no longer offers folders or live sources, which a tab can't do. Not built yet: Parquet, `.tlm`, packets,
descriptors, folders, derived channels, events, checks, several files in a tab. They come next.
Do: Serve the page (see above). Choose or drop `flights/flight-1.csv`. Then drop a bigger CSV (`/Users/rafy/workspace/test`
has some; or make a 500,000-row one). Reload the page. Try dropping a `.tlm` or a Parquet file. Look at it in light and dark.
Expect: the CSV opens and draws like the app's; while a big one loads, the page still responds; after a reload the files
are back with no picker; the start page offers only Open recording…; a binary file says browser-only mode reads CSV.
Result:
Notes:

---

## A. Open things and the Explorer

### A3. Open a CSV by double-click, and with ⌘O
Do: Double-click AAPL.csv in Explorer. Then ⌘O and pick GOOG.csv.
Expect: the import dialog appears the first time for a new header, a chart appears with channels listed, the second file opens
in its own tab (or this tab, as the dialog says). Clear cache keeps import profiles: to see the dialog again for a file you
opened before, delete its profile, `~/Library/Caches/tlmbench/profiles/<key>.json` (Reset app… in Settings will do this once built).
Result: The file opened right away because I had opened it previously. Clearing the cache didn't change the profile.
Notes: There should be a clear way for the user to "Reset to default settings" and clear all of the cache. A warning dialog is appropriate as needed.

## B. Reading a chart

### B4. A file that is still building
Do: Settings › Cache › Clear cache first (a file opened before has its pyramid in the cache and opens at once). Then open the 100M-sample file (`just testdata big`, then `~/tlmbench-testdata/big/big-100M.tlm`).
Expect: hatching while the pyramid builds, then the chart fills in; the min/max band shows on a spiky channel. (L: X-win-06)
Result: The file opened pretty much instantly, so I couldn't really tell....
Notes: Not verified, need a better test case

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