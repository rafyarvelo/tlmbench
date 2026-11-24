# Changelog

All notable changes, by release. Versions follow [Semantic Versioning](https://semver.org/).

## 1.0.0-rc.1

The first release candidate of 1.0.0, for testing: everything listed under 1.0.0 below. It is a pre-release; the
Python package is not published to PyPI.

- Installers are not signed by a paid certificate: on macOS, the first open needs System Settings › Privacy & Security
  › Open Anyway (macOS 15 and later) or right-click › Open (macOS 14 and earlier); on Windows, SmartScreen asks once.
  [`docs/install.md`](docs/install.md) has the steps.
- Tested on macOS (Apple silicon). On Windows and Linux the automated tests and the install checks pass; checks in
  a real window are still to do.

## 1.0.0 (unreleased)

The first release: the whole design in [`docs/design.md`](docs/design.md). What follows is what 1.0.0 can do, by area;
[`docs/release/ledger.md`](docs/release/ledger.md) lists every requirement with the test that shows it.

### Opening data
- CSV and TSV (delimiter, units and time column detected; an import dialog remembered for files with the same header;
  text columns of up to 256 values as labelled channels), Parquet, the self-describing `.tlm` format, fixed-width binary
  records and packet files (sync word, length, checksum, zstd, LZ4 or gzip payloads) described by a TOML descriptor or
  made with its form, and decoder plugins.
- Each file is read once, at disk speed, into a cache outside the data folder: a read-only folder or network share
  works as well as a local disk, and nothing is written beside the data.
- Files open from the system: `.tlm` by default, CSV, TSV and Parquet offered under Open With; a drop on the window.

### Reading data
- Pan and zoom over billions of samples, every pixel column drawn from its exact minimum and maximum (no spike hidden
  at any zoom); a coarse first frame that the exact one replaces; gaps shown as gaps.
- Panes in rows and columns (split, duplicate, drag, close with Undo); an overview strip; Box zoom, Pan, Cursors, Back,
  Fit and End; values at the pointer; cursors A and B with Measure; Play with speed and an A–B loop.
- Tables with sort, filter and Apply to charts; XY panes; Gauges with built-in widgets and your own (SVG or code).
- ⌘Z and ⇧⌘Z (Ctrl+Z, Ctrl+Shift+Z) for every layout change; a native menu bar; every setting kept.

### Analysis
- Derived channels from expressions and recipes; channel display, limits with shading, missing values, value labels.
- Events: added by hand, read from the file, or raised by rules and checks; categories and filters.
- Rules and checks with reports, on one file, every file of a tab, a folder (Run on folder…) or live data; the Report
  tab with pass rates and trends; `tlmbench check` in CI with exit codes.

### Several files
- Recordings on one timeline in one tab, each in a numbered slot, lined up by Align starts, Same clock, an offset, sync
  points, event matching, cross-correlation, a time channel or an alignment plugin. Tables and XY across files.

### Live
- Serial, TCP, UDP, a growing CSV or `.tlm`, or a recording replayed: triggers, live checks, several sources in one
  tab, Reconnect, recording to a folder you choose.

### Sharing and exporting
- A folder's `.tlmbench/` holds layouts, check sets, recipes, events, widgets, icons and decoders as small files ready
  for git; Share… writes only what changed; a read-only folder offers Export instead.
- Data export to CSV, Parquet, Arrow, HDF5 and MAT; images; HTML exports (a folder or one file, Full or Light) that open
  from any static host or file share with the app's own viewer.

### Windows and the browser
- Several windows on one core; drag a tab out; link the time view across windows; browser access to the running app.
- Browser-only mode: the same UI with the core in WebAssembly, opening CSV, Parquet, `.tlm`, descriptor and packet
  files and WASM decoder plugins in a tab, with nothing leaving the machine.

### Plugins
- WASM decoders, payload decoders and alignment estimators in a sandbox (no files, network, environment or clock; a
  time and memory limit per call), from `cargo generate` templates; native decoders that stay off until turned on.

### Command line and Python
- `tlmbench`: info, query (view, stats), record, check, convert, export html, and more, over the same core and cache.
- `pip install tlmbench`: open, read into NumPy or Arrow, view, stats, check and export from Python.

### Install
- macOS (universal `.dmg`), Windows (installer, `.msi`, portable zip), Linux (`.deb`, AppImage), the CLI on its own for
  five platforms and as a container image, Python wheels, the browser-only site; `SHA256SUMS` and build-provenance
  attestations for every file. See [`docs/install.md`](docs/install.md).
