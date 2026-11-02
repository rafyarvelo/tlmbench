# TelemetryBench (tlmbench)

A desktop workbench for plotting, inspecting and checking telemetry, live or recorded. It opens multi-GB
recordings and live feeds, pans and zooms over billions of samples, and runs the same rules, checks, derived
channels and layouts headlessly from a CLI and Python.

Status: **Phase 1 (core loop) of the roadmap in `docs/design.md`.** CSV and fixed-width ingest, M4 straight from raw
samples, one WebGL2 pane over binary IPC, and `tlmbench info`; the Phase 1 gate (100M points at 60 fps) passes on a
MacBook Air M1.

## Hard requirements

- Full capability from the app cache alone: the app works completely on a read-only folder.
- Writing next to the data only by user action: Share (to `.tlmbench/`) and exports.
- Exports are static files: an HTML export opens from any static host or file share with no install.
- Everything in the design is v1; there is no rush to release.

## Repository layout

```
docs/                 design doc (exported from the living doc) and links to the decision log and audit
ci/                   CI image (Dockerfile), pinned versions, setup and read-only check scripts
prototype/            single-file HTML prototype of the UI
  tests/              Playwright browser tests for the prototype (./run.sh)
examples/recordings/.tlmbench/
                      example shared configuration: check sets, categories, recipes, views, a code widget
```

Code, growing phase by phase (✓ = started):

```
crates/core      ✓    time base, M4 from raw samples, wire format; later the pyramid, time maps, query engine
crates/decode    ✓    CSV columnar cache, format descriptors (TOML), reference .tlm format; later WASM and native plugins
crates/expr           expression language: derived channels, rules, checks, triggers
crates/check          rule-and-check runner, roll-up, reports
crates/export         CSV, Parquet, Arrow, MAT/HDF5, static HTML export
cli/             ✓    tlmbench: info now; later convert, check, export html, info, validate, derive, config export, watch
python/               pyo3 bindings (wheels on PyPI)
app/             ✓    Tauri shell and the web UI (one renderer for time, live, XY and overview panes)
viewer/               the reduced frontend bundle used by HTML exports and the browser-only mode
tools/gen        ✓    deterministic synthetic recordings (just gen, just gen-gate)
```

## Development

Native on the Mac, Linux in CI. See `docs/development.md`.

```
brew bundle && ci/setup.sh   # then open a new shell
just proto-test           # the prototype's Playwright suite (just proto-test t_checks runs one)
just lint test bench      # once the Cargo workspace exists
```

## Shared configuration

A folder's `.tlmbench/` holds configuration only, ready for git: one small file per object, TOML for files
people edit, JSON for layouts. See `examples/README.md`.
