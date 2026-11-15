# Browser-only mode: TelemetryBench in a browser tab

The same interface as the app, with the core compiled to WebAssembly, running in the page. Nothing is installed, nothing is
uploaded (the page has no server behind it), and nothing is written beside your data. Choose a CSV or drop one on the page.

What it reads today: CSV files (the decoder's own CSV reader, so units in the header, the time column and time that goes
backwards read as in the app) and tlmbench reference files (`.tlm`: the decoder's own header, descriptor and time code). The parse runs in a Worker, so a big file doesn't freeze the tab. What was read is kept in the
browser's private storage (OPFS); the same file opens from it next time, and **a reload brings back the files that were
open**. Settings › Clear cache empties that storage. What a tab can't do is left out, not shown as a button that does nothing (no Live in the rail; Settings has no browser access, plugins, live recording or shared configuration). The app reads more (packets, Parquet, descriptors from files,
plugins, folders, live sources, derived channels, checks); see `docs/follow-ups.md`.

## Hosting it

The page is static: a folder of files, relative paths, served from any folder of any static host.

```sh
just browser-site            # builds app/browser/dist (the page, its script, the WASM core)
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
