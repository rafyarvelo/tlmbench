# Release test plan: Windows

One pass per OS, on the release candidate's installers. Each item is a short thing to do and what should happen. After
`Result:` write **P** (pass) or **F** (fail); add a few words only for an F.

This is the Windows copy of `test-plan.md` (written by `tools/testplan.py`, which keeps the results): fill it
in and hand it back.
The agent fixes every F, ranked, and the next release candidate follows.

**OS:** ______ **Version:** ______ **Date:** ______ **Screen:** (HiDPI / normal / both)

## 0. Before you start

1. Download from the release candidate's page (Releases, marked Pre-release): the app for this OS and `SHA256SUMS`.
   `docs/install.md` names the file for each OS.
2. Get the test data script, `make.py`, and run it. It needs Python 3.9 or newer (install it from python.org; `py
   --version` checks it). In PowerShell:
   ```
   cd ~; curl.exe -fsSLO https://raw.githubusercontent.com/rafyarvelo/tlmbench/main/tools/testdata/make.py
   py make.py big            # (everything, with the 400 MB file for section 9; without "big", the rest only)
   ```
   If it says `pyarrow isn't installed`, item 2.6 has no Parquet file: run `py -m pip install pyarrow` and
   `py make.py big` again.
   It writes `tlmbench-testdata` in your home folder: `flights/` (four flight CSVs and a `.tlmbench/` with a check set, a layout,
   widgets and events), `other/` (`bench.bin` with its descriptor, `modes.csv`), `browser/` (`bench.tlm`, Parquet files)
   and `big/`. Running it again writes the same files.

## 1. Install and first open

**1.1** Check the download: put `SHA256SUMS` in the same folder as the installer, then follow `docs/install.md`,
"Checking a download" (its PowerShell lines). It prints `True`. Result:

**1.3** Run the `-setup.exe`. SmartScreen may warn: More info › Run anyway. It installs and starts from the Start
menu. Result:

**1.4** Unpack the `-portable.zip` and run `TelemetryBench.exe`: it starts. Result:

**1.7** The start page shows; Help › User manual opens the PDF manual; Help › Online manual opens a browser page.
Result:

**1.8** About TelemetryBench (end of Help) shows the version, MIT OR Apache-2.0, and
mentions THIRD_PARTY_LICENSES.md. Result:

**1.9** Help › Install command-line tool…, then in a new terminal `tlmbench --version` prints the version. (If it says
the folder isn't on PATH, use Copy the line, add it as it says, open a new terminal.) Result:

## 2. Opening files

**2.1** Open folder… › `tlmbench-testdata/flights`. The Explorer lists the files; the folder's widgets, layout and check
set are available. Result:

**2.2** Open `flight-1.csv`. The import dialog shows the columns with time detected; open it. Charts appear, and
nothing new is written in `flights/` (look in the folder). Result:

**2.3** Open `flight-2.csv`: the dialog doesn't ask again (same header). Result:

**2.4** Open `other/bench.bin` with Open recording…: it opens through its folder's descriptor (altitude, battery, armed).
Result:

**2.5** Open `other/modes.csv`: `mode` shows its words (IDLE, TAXI, CLIMB…) on the chart axis and in values. Result:

**2.6** Open `browser/bench.tlm` and a Parquet file from `browser/`. Both open. Result:

**2.7** With the app closed, double-click `bench.tlm` in the file manager: the app opens it, in front. Again while it
runs: a tab of its own. Result:

**2.8** Right-click `flight-1.csv` › Open With: TelemetryBench is offered (and isn't the default). Result:

**2.9** Drag `flight-gap.csv`, then the `flights` folder, from the file manager onto the window: the file opens; the
folder opens as Open folder… does. Result:

**2.10** Open the `flights` folder in Explorer. Click the folder's ⋯ › **Refresh**, and straight away double-click
`flight-2.csv`. Do it three times, quickly. Each double-click opens the file (or shows its tab): none is lost. (A UI
test lost one on CI, Nov 25; this checks whether a person can.) Result:

## 3. Looking at data

**3.1** On `flight-1.csv`: wheel-zoom, drag to pan, Box zoom, Back, Fit. Each works and the time range updates.
Result:

**3.2** Point at a chart: values at the pointer. Place cursors A and B, open Measure: Δt, Δy, min, max, mean.
Result:

**3.3** `flight-gap.csv`: the 30 s hole is a gap in the line, not a straight line across. Result:

**3.4** Play: the playhead moves; change the speed; set an A–B loop. Result:

**3.5** Add a Table pane: sort by a column (its ▾ menu); type `> 15` in the filter box under battery_v's header; step
through the matches (▲ ▼ in the row that appears under the table's bar); tick **Apply to charts** at that row's right. The
charts leave out the other rows (hatched along the bottom, a "Filtered · N rows hidden" chip).
Result:

**3.6** Add an XY pane: altitude against speed. Result:

**3.7** Light and dark (Settings): everything readable in both; text crisp. Result:

## 4. Panes and layout

**4.1** Drag a channel from the Explorer onto a pane, onto a pane's left and right edges, and between panes. An outline
shows where it lands, and it lands there. Result:

**4.2** In a pane of two channels, drag one chip to the pane's own left edge, then (another time) its right edge: the
channel moves to a new pane on that side. Hold Ctrl while dropping: it's copied instead.
Result:

**4.3** Make one pane on top, two side by side, one at the bottom. Close the left one of the pair: the right one widens
in its row. Ctrl+Z: the pair is back side by side. Same with the right one. Result:

**4.4** Drag three channels onto categories in the Explorer one after another, close a pane in between; Ctrl+Z four times
steps back through each, Ctrl+Shift+Z redoes. Result:

**4.5** Save the layout (Save as…), rename it, Ctrl+Z (the name comes back), delete it, Ctrl+Z (it's back). Result:

**4.6** Layouts: right-click a layout › Pin to the top bar; its button opens it; right-click the button › Unpin.
Result:

**4.7** Settings › Quiet interface on: pane headers show only the view menu and ⋯ until you point at them. Should it be
on by default? Write yes or no in the note. Result: Note (yes/no):

**4.8** Quit and reopen: the tabs, the layout and the settings are as you left them. Result:

## 5. Channels, events, checks

**5.1** Click a chip › Display: change colour, line width, dashed, steps. The chart follows. Result:

**5.2** Limits: give `battery_v` a warn at 13 and an alarm at 12: dashed lines, yellow and red shading. Result:

**5.3** Derive a channel: `speed * 3.6`, unit km/h. It appears and plots. Result:

**5.4** Events: add an event by hand; the flight-1 events from the folder (Takeoff, Hard turn) show with a tag saying
they're from the folder, and say why they can't be edited. Result:

**5.5** Open `flight-bad.csv`. Rules & checks › the folder's "Flight limits": the battery check fails (✗), the others
pass. Result:

**5.6** Rules & checks › Run on folder… › Run: the Report tab opens with each file's result; Pass rates and trends,
sort by pass rate, click a point: that file opens. Quit and reopen: the Report tab is back. Result:

**5.7** Edit a check set: + New category… asks for a name in the app's dialog and picks it; Delete set asks first and
No keeps it. Result:

## 6. Two files in one tab

**6.1** Right-click `flight-2.csv` › Add to this tab (with flight-1 open): channels read `name·2`; Alignment offers
Align starts, Same clock, Offset, Cross-correlation… Pick Align starts. Result:

**6.2** A Table with a channel from each file: sort and filter by the second file's column. Result:

**6.3** An XY of file 1's channel against file 2's: a line on the shared timeline. Result:

## 7. Widgets and gauges

**7.1** A Gauges pane: add widgets; ⋯ › Range… asks for "min, max" in the app's dialog and the widget follows.
Result:

**7.2** The folder's code widget asks before it runs (trust); after Run widgets from this folder it draws. Result:

**7.3** Your widgets ▸ › New or edit widgets…: add an SVG and a code widget, rename one, add an input, Duplicate,
Delete, Edit as JSON and back, Save; the preview follows. Result:

**7.4** Settings › Icons: paste an SVG; a preview shows. Result:

## 8. Live

**8.1** In a terminal: `python3 make.py send`. In the app: Connect › TCP, `127.0.0.1:5555`, descriptor
`other/.tlmbench/decoders/bench.toml`, Reconnect on, Connect. The strip says LIVE, charts follow. Result:

**8.2** Scroll back, then press End: it follows live again. Result:

**8.3** Stop the sender (Ctrl+C) and start it again: RECONNECTING, then LIVE; the chart goes on from the new start.
Result:

**8.4** Live tail: `python3 make.py grow live.csv` in a terminal, open `live.csv` as a growing file: it follows. Result:

**8.5** Add a live check (battery above 12 V): it shows its state as data arrives. Result:

## 9. Big file

**9.1** Open `big/big-100M.tlm`: hatching while it builds, then the full chart; zoom in to single samples smoothly.
Result:

**9.2** The spikes (every 0.25 s) stay visible at every zoom. Result:

## 10. Sharing and exports

**10.1** Share… on `flight-1.csv`'s tab: it writes only what changed into `flights/.tlmbench/`. Result:

**10.2** Export data: CSV and Parquet of two channels over a span. The files open in another tool. Result:

**10.3** Export to HTML (Light, one file): open the `.html` in a browser by double-click: the viewer opens Locked,
Unlock lets you rearrange. Result:

**10.4** Save image (PNG): the image matches the pane. Result:

**10.6** Read-only: make a copy of `flights` read-only (Properties › Read-only on the folder and its
files), Open folder… on it and open a file. The tab menu says the folder is read-only and
offers Export file… instead of Share. Result:

## 11. Windows and the browser

**11.1** Drag a tab out of the window: a new window; drag it back onto the first window's tab strip. Result:

**11.2** Two windows, quit, reopen: both come back where they were. Result:

**11.3** Browser-only: unpack `tlmbench-…-browser-site.zip`, serve it (`python3 -m http.server` in that folder), open
`http://localhost:8000`, open `browser/bench.tlm` and a CSV. They open in the tab. Result:

**11.4** In that browser page add `?renderer=2d` to the address and reload: the same charts, drawn without WebGL.
Result:

## 12. The command line

**12.1** `tlmbench info flights/flight-1.csv` lists the channels. Result:

**12.2** `tlmbench check flights/ --set "Flight limits"`: exit code 1 (flight-bad fails); the report names it.
Result:

**12.3** `tlmbench convert flights/flight-1.csv --to parquet --out out/`: writes the file. Result:

## 13. Look and feel, last

**13.1** Walk the app once more in light and dark: start page, Explorer, a few panes, a dialog, Settings, the Report
tab. Anything clipped, blurry, misaligned or off: list it in the note. Result: Note:

**13.2** Turn off animations (Settings › Accessibility › Visual effects › Animation effects off): zooms and menus don't animate. Result:

---

**Signed off:** ______ (all items P or –, or the F items accepted for after v1)
