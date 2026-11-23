# The repository

```
Cargo.toml, rust-toolchain.toml   one workspace, one pinned Rust
crates/
  core/         time base, columns, M4, pyramid, wire format          (tlmbench-core)
  decode/       CSV, Parquet, records, packets, plugin hook           (tlmbench-decode)
  store/        cache, pyramid builds, checks, events, alignment      (tlmbench-store)
  expr/         expression parser and evaluator                       (tlmbench-expr)
  live/         sockets, tailed files, ring buffer, recorder          (tlmbench-live)
  export/       CSV, Parquet, Arrow, HDF5, MAT, HTML export           (tlmbench-export)
  plugin/       WASM and native plugin host, the WIT package, the C header (tlmbench-plugin)
  web/          the core as WASM for the viewer and browser-only mode (own target, not in the workspace)
  web-parquet/  the Parquet reader for browser-only mode              (own target, not in the workspace)
cli/            the `tlmbench` command line
python/         the `tlmbench` Python package (pyo3, maturin; own target)
app/
  src-tauri/    the desktop app: Tauri shell, menu, daemon, installers' config
  ui/           the web UI (Svelte, TypeScript, Vite) and its tests
  viewer/       the packed viewer an export ships; browser/ the browser-only site (both built by `just viewer`)
plugins/        plugin templates and the plugins the tests use (standalone crates)
tools/          gen (synthetic recordings), testdata (the release test data), manual (PDF/HTML build), icons, scripts
ci/             the CI image and setup, gates, the read-only check, the ledger check, versions
docs/           manual/, developers/ (this guide), reference/ (generated), design, install, release/, images/
examples/       an example `.tlmbench/` folder
prototype/      the single-file UI prototype and its tests (the design reference)
justfile        the same recipes on the Mac and in CI
.github/        workflows: ci, python, release (and the web copy's)
```

## Documents

- [`docs/design.md`](../design.md): the design, exported from the living document in Claude Docs (links in
  [`docs/README.md`](../README.md)). Each decision is Proposed, Accepted, Modified or Rejected; treat Accepted and Modified
  as requirements. The living doc is edited first, then exported.
- [`docs/follow-ups.md`](../follow-ups.md): notes owed to the design doc, from behaviour that changed after it was written.
- [`docs/post-v1.md`](../post-v1.md): what comes after the release. New ideas go here, not into v1.
- [`docs/release/`](../release/): the plan and its status table, the ledger, the owner's test plan, the security review.
- [`CLAUDE.md`](../../CLAUDE.md): notes for the AI agents that work on the repository, which hold the same conventions as
  this guide (and the owner's preferences).

## Conventions

- Rust edition and lints are the workspace's (`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all`).
- Run cargo workspace-wide (`--workspace`), not `-p`: `-p` builds another feature set and fills the disk.
- Never commit recordings, caches or test output (see `.gitignore`).
- Small commits, one topic each, with a clear message; don't rewrite pushed history.
