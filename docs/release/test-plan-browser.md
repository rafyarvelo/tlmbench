# Release test plan: the browser

The browser builds of TelemetryBench, tested by hand: browser-only mode (the app in a tab, the core in WebAssembly), the
HTML export viewer, and the copy on GitHub Pages. The main plan (`test-plan.md`) is for the installed app; its 10.3,
11.3 and 11.4 are the only browser items there, and this plan replaces them in depth.

After `Result:` write **P** (pass) or **F** (fail); add a few words only for an F. Skip an item that doesn't apply by
writing **–**. Items marked *(known on rc.2)* failed there and are fixed for the next candidate: on rc.2 an F is expected.

**Browsers:** (which, and their versions)
**Release:** (e.g. 1.0.0-rc.2)

## 0. Before you start

- The test data: `python3 tools/testdata/make.py` (from the release's source, or the repo) writes `tlmbench-testdata/`.
  Part A uses `flights/`, `other/` and `browser/` from it. A CSV of daily values (dates in the time column, a year or
  more) is useful for A2.6; any one will do, e.g. a price history.
- Browsers: one Chromium browser (Chrome or Edge: the only ones with the folder picker), Safari, Firefox, and a phone
  (Safari on iOS or Chrome on Android).
- Part A serves the release's `tlmbench-<version>-browser-site.zip` from this machine:
  unzip it into a folder, then `python3 -m http.server 8000` in that folder and open `http://localhost:8000`.

## A. Local: the browser-only site served from this machine

### A1. Opening files

**A1.1** The start page shows **Open recording…**; in Chromium also **Open folder…**; in Safari and Firefox it says once
that this browser can't open folders. No Live, Layouts or Rules & checks in the rail. Result:

**A1.2** Open recording… › `flights/flight-1.csv`: the import dialog shows the columns with time detected; open it.
Charts appear, the tab is named after the file. Result:

**A1.3** Drop `browser/bench.tlm` on the page: it opens in a tab of its own. Result:

**A1.4** Open a Parquet file from `browser/`: it opens (the Parquet reader loads the first time). Result:

**A1.5** Reload the page: the files that were open come back, without the import dialog. Result:

**A1.6** Settings › Clear cache, reload: the files have to be chosen again. Result:

### A2. Looking at data

**A2.1** Chart: pan, zoom (wheel, trackpad, + −), Fit; cursors A and B with their readouts. Result:

**A2.2** Table (a pane's view menu › Table): rows page smoothly; click a column to sort, again to reverse; a filter
(`> 13` on `battery_v`) leaves only those rows, and ▲ ▼ step through the matches. *(known on rc.2: sort and filter do
nothing)* Result:

**A2.3** Gauges: each channel's widget shows its value at the end of the view, and follows when the view moves.
*(known on rc.2: stays at "reading…")* Result:

**A2.4** XY: a pane with two channels (e.g. altitude and speed) as XY draws the points; box zoom, double-click to fit.
*(known on rc.2: empty axes)* Result:

**A2.5** Spectrum: names each channel's peak. *(known on rc.2: says to zoom out)* Result:

**A2.6** The time range at the top on the daily CSV: dates and times (`2023-01-03 00:00:00 → …`), not
`00:00:00 → 00:00:00`. *(known on rc.2)* Result:

**A2.7** Light and dark (Settings): readable in both; the charts, gauges and XY follow at once. Result:

**A2.8** Add `?renderer=2d` to the address and reload: the same charts, drawn without WebGL. Result:

### A3. Folders (Chromium only)

**A3.1** Open folder… › `tlmbench-testdata/flights`: Explorer lists the files read-only; the folder's widgets and event
categories are used. Result:

**A3.2** Reload: the folder is remembered, and the browser asks for permission again. Result:

**A3.3** Open folder… › `other/`, open `bench.bin`: it opens through the folder's descriptor (shown as shared in the
descriptor form). Result:

**A3.4** Nothing was written in the folder (look in it, and in its `.tlmbench/`). Result:

### A4. Plugins

**A4.1** Settings › Plugins › Add plugin…, choose a `.wasm` decoder plugin: it's listed, and a file it names opens through
it. Remove takes it away. Result:

**A4.2** A folder that shares plugins: they wait for your OK; once trusted, a file they name opens. A native plugin is
listed as needing the app. Result:

### A5. Other browsers

**A5.1** Safari: A1.2–A1.5 and A2.1–A2.5. Result:

**A5.2** Firefox: the same. Result:

## B. HTML exports (the viewer)

Made in the installed app: Export to HTML of a file with a chart, a table, gauges and an XY pane (Light, one file; then
Full, a folder).

**B1** Open the one-file `.html` by double-click (from disk): it opens Locked, the panes as they were exported. Result:

**B2** Pan and zoom after Unlock; cursors work while Locked. Result:

**B3** Table: sort and filter work, Locked or not. *(known on rc.2: they do nothing)* Result:

**B4** Gauges and XY show their values and points. Result:

**B5** The Full export's folder, served (`python3 -m http.server` in it) and opened in Safari and Firefox too. Result:

## C. GitHub Pages (the published copy)

The Pages copy is updated by a release (`v1.0.0`), not by a candidate: until then it shows the last one published (the
page doesn't show its version; the repository's Actions › the last Pages deployment says which commit).

**C1** The README's links open: User Manual, Try it in your browser, A sample export. Result:

**C2** Try it in your browser: the start page loads over HTTPS, with no errors in the browser's console. Result:

**C3** Open a CSV from this machine and from the phone's files: it opens and draws. Result:

**C4** On the phone, upright and sideways: the bars fit, menus open, panes scroll, a chart pans and pinch-zooms.
Result:

**C5** A2.2–A2.6 on the phone. Result:

**C6** The sample export: opens Locked, its charts, table and gauges draw. Result:

**C7** The manual: chapters and pictures load; its links to the app and the sample work. Result:
