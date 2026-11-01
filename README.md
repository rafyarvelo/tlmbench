# TelemetryBench (tlmbench)

A desktop workbench for plotting, inspecting and checking telemetry, live or recorded. It opens multi-GB
recordings and live feeds, pans and zooms over billions of samples, and runs the same rules, checks, derived
channels and layouts headlessly from a CLI and Python.

Status: **design and prototype.** The v1 code (Rust core, Tauri app, CLI, Python bindings) starts with Phase 1
of the roadmap in `docs/design.md`.

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

Planned for v1, added as each roadmap phase starts:

```
crates/core           storage, pyramid (M4), time maps, query engine
crates/decode         format descriptors (TOML), WASM and native decoder plugins
crates/expr           expression language: derived channels, rules, checks, triggers
crates/check          rule-and-check runner, roll-up, reports
crates/export         CSV, Parquet, Arrow, MAT/HDF5, static HTML export
cli/                  tlmbench: convert, check, export html, info, validate, derive, config export, watch
python/               pyo3 bindings (wheels on PyPI)
app/                  Tauri shell and the web UI (one renderer for time, live, XY and overview panes)
viewer/               the reduced frontend bundle used by HTML exports and the browser-only mode
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
