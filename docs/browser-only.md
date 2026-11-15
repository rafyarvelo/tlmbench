# Browser-only mode: TelemetryBench in a browser tab

The same interface as the app, with the core compiled to WebAssembly, running in the page. Nothing is installed, nothing is
uploaded (the page has no server behind it), and nothing is written beside your data. Choose a CSV or drop one on the page.

**What it reads today:** CSV (the decoder's own reader, so units in the header, the time column and time that goes
backwards read as in the app), tlmbench reference files (`.tlm`: the decoder's own header, descriptor and time code) and
Parquet (snappy, LZ4, gzip, brotli and Zstandard, by the app's rules for the time column, units and nulls). The Parquet
reader is a module of its own (about 1 MB gzipped) that the page fetches only when a Parquet file opens. The parse runs in
a Worker, so a big file doesn't freeze the tab. What was read is kept in the browser's private storage (OPFS); the same
file opens from it next time, and **a reload brings back the files that were open**. Settings › Clear cache empties it.

**Folders.** Where the browser has `showDirectoryPicker` (Chromium browsers today; not Firefox or Safari), **Open folder…**
lists the folder in Explorer, read-only (the picker is asked for read access only and nothing is written to the folder):
sub-folders, the files the app can read, dot folders left out. A file in it opens from its handle. The folder is remembered
in the browser, and after a reload the browser asks for permission again. Where there is no picker, the start page says so
and offers Open recording…. The folder's `.tlmbench/` (shared configuration) is not read yet.

**Still to come for v1** (release plan R2.2): fixed-width and packet files through descriptors (kept in the browser,
found by file name), WASM decoder plugins in the browser's own engine, and reading the folder's `.tlmbench/`.

**What a tab leaves out:** what needs the app (live sources, native plugins, browser access, recording, writing shared
configuration), and what comes after v1 (several files in one tab with alignment, derived channels, events, rules and
checks, layouts: `docs/post-v1.md`). A tab doesn't show a button that can't act.
In the page that means: no Live, Layouts or Rules & checks in the rail; no Derive, Compare, Open together, Add to tab,
Open with layout, Mark when or Add check; no Save, Share or Export file in the tab menu (a tab keeps no layouts); no
Checks view for a pane; the Events panel lists but has no Marker, Range or Rules; Settings leaves out what it can't do.

## Hosting it

The page is static: a folder of files, relative paths, served from any folder of any static host.

```sh
just browser-site            # builds app/browser/dist (the page, its script, the WASM core and the Parquet module)
python3 -m http.server --directory app/browser/dist     # try it: http://localhost:8000
```

A browser only runs WebAssembly and Workers from a web address, not from `file://`: put the folder on a host or serve it as
above. For GitHub Pages, publish `app/browser/dist` with the standard Pages action:

```yaml
- run: just browser-site
- uses: actions/upload-pages-artifact@v3
  with: { path: app/browser/dist }
- uses: actions/deploy-pages@v4
```

(The release workflow, R4, runs this on a tag. The hosting choice is the owner's: see the release plan, decision 7.)
