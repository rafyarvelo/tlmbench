# Building and testing

Development is native on the Mac; Linux runs in CI, in an image built from `ci/Dockerfile`. The same `just` recipes run in both places, and every tool version is pinned in one place (below). Windows is built and tested in CI too.

## Setup (Mac)

```
brew bundle          # coreutils (GNU timeout), Python 3.12
                     # Node LTS: the macOS installer from nodejs.org, version NODE_VERSION in ci/versions.env
ci/setup.sh          # rustup + Rust from rust-toolchain.toml, the fuzzing nightly, just and the cargo tools,
                     # wasmtime and wasm-tools, Playwright's headless Chromium, and the Python venv in .venv
```

The first run installs rustup from sh.rustup.rs, which adds `~/.cargo/bin` to your shell profile; open a new shell
afterwards. From then on `just setup` reruns it, and `just setup <step>` runs one step: `rust`, `cargo-tools`,
`wasm`, `playwright` or `python`. `just` itself is pinned (`ci/versions.env`), installed by the `cargo-tools` step.

About 9–10 GB at first: toolchains ~3–4 GB, `target/` ~5 GB (it grows; `just clean` resets it), cargo registry ~1 GB.

## Recipes

The test recipes are described in [The test suites](tests.md), the gates in [Gates and measurements](gates.md).

| Recipe | What it does |
| --- | --- |
| `just lint` | rustfmt check and clippy, warnings are errors |
| `just test` | Rust unit, property and doc tests (nextest), then the Python binding tests |
| `just bench` | criterion benchmarks; run them on the Mac, since CI runners are too noisy for timing |
| `just bench-record`, `just bench-compare` | after `just bench`: save the medians with the commit (`bench/history.json`); compare with the last saved run, failing on anything over 10% slower |
| `just proto-test [t_name …]` | syntax-check the prototype, then its Playwright suite |
| `just app-test [t_name …]` | build the app's UI, then its tests: headless Chromium against a fake core (`app/ui/tests/README.md`) |
| `just fuzz-smoke [secs]`, `just fuzz <crate> <target>` | fuzz every target briefly, or one until stopped (nightly) |
| `just ro-test`, `just ro-test-mac` | prove nothing is written next to the data (below) |
| `just app-dev [file]`, `just app-build` | run the desktop app with hot reload (WKWebView), or build it without bundling |
| `just app-shot [file] [out]` | screenshot the release app's window to `target/app-shot.png` (Mac; the editor needs Screen Recording in System Settings › Privacy & Security) |
| `just app-icons` | regenerate `icon.png`, `icon.icns` and `icon.ico` from `app/src-tauri/icons/icon.svg` (Mac) |
| `just gen …`, `just gen-gate` | write synthetic recordings (sine + noise + spikes + gaps) to `$TLMBENCH_DATA` (default `~/tlmbench-data`) |
| `just gate [file]` | the Phase 1 gate in the release app (below) |
| `just gen-gate2`, `just gate2 [file] [keep]` | write the 10 GB Gate 2 recording; run the Phase 2 gate over it (below) |
| `just gate3 [core\|app] [seconds]` | the Phase 3 gate: 4M samples/s streamed to the recorder, or to a live tab in the app, and checked (below) |
| `just viewer` | the viewer an HTML export ships and browser-only mode: builds the core and the Parquet reader for WASM (`crates/web`, `crates/web-parquet`, their own target folders) and the UI for each; packs the viewer into `app/viewer/viewer.pack`, which the CLI and the app embed (run it before building them); browser-only mode is the static site in `app/browser/dist` |
| `just gate5` | the v1 gate: a folder check, and an HTML export of a 2 h, 20-channel flight within 250 MB that opens in under 2 s from a static host (below) |
| `just ci-linux`, `just ci-native` | everything a CI job runs |

| `just unused` | `cargo machete` and `knip` (in `app/ui`): unused dependencies and files, before a release |
| `just pages OUT` | the web copy in `OUT`: a landing page, the manual as HTML, browser-only mode, a sample export |
| `just manual [--html]` | the User Manual as one PDF (and the HTML page) from `docs/manual`; fails on a broken link or a missing image |
| `just reference [--check]` | the reference pages in `docs/reference` from the source; `just lint` runs the check |
| `just python-docs [--check]` | `docs/python-api.md` from the module's docstrings |

## The daemon and browser access

`target/debug/tlmbench-app --daemon [--port N] [--ui app/ui/dist] [--token T] [--once]` runs the core with no window and
prints the page's address (`http://127.0.0.1:PORT/?token=…`); open it in a browser on the same computer. It uses the
app's settings and cache. The desktop app serves the same with Settings › Browser access, or `--browser`. `t_browser`
(`app/ui`, `node tests/t_browser.js`) runs the UI in Chromium against the real daemon and needs the app built first
(`cargo build -p tlmbench-app`, and `npm run build` in `app/ui`); with `TLMBENCH_GUI=1` it starts the desktop app
instead (it opens its window). It keeps its settings and cache under a temporary `HOME`.

## Pinned versions

| What | Where |
| --- | --- |
| Rust stable, components, wasm targets | `rust-toolchain.toml` |
| Node, nightly, just, cargo-binstall, nextest, cargo-fuzz, tauri-cli, wasmtime, wasm-tools | `ci/versions.env` |
| cargo-generate (plugin templates only, not in CI: `cargo binstall cargo-generate@0.26.0`) | 0.26.0, here |
| Playwright | `prototype/tests/package.json` (+ lock file); the app's UI tests use the same install |
| Svelte, its Vite plugin, svelte-check, TypeScript, Vite, the Tauri JS API, TanStack Table (table-core) | `app/ui/package.json` (+ lock file) |
| maturin, pytest, numpy, pyarrow | `requirements-dev.txt` |
| CI base image | digest in `ci/Dockerfile` |

The UI stays on TypeScript 6 until svelte-check supports 7 (it needs TypeScript's JavaScript API, which 7 doesn't have).

`ci/setup.sh` installs from these on the Mac and in the CI image. On the Mac, Python and coreutils
follow Homebrew and Node is whatever nodejs.org installer you ran; CI uses the exact versions.

## The read-only rule

Nothing may be written next to the data except by Share and exports. `ci/ro_check.py` records every path under a
folder (type, size, mtime, SHA-256), runs the commands in `ro_commands` (justfile), and fails if anything changed or
a command failed. With `--require-ro` it also fails unless the folder is really read-only, so a write attempt errors
instead of quietly succeeding.

- `just ro-test`: against `prototype/tests/fold` (or `TLMBENCH_RO_DIR`).
- `just ro-test-mac`: copies that folder into a read-only disk image (`hdiutil`, UDRO), mounts it read-only and runs
  the check there. Use the same mount for manual runs of the desktop app.
- CI mounts the folder read-only (`-v …:/data/ro:ro`) and passes `--require-ro`.

`ro_commands` runs `ci/ro_cli.sh`: every CLI command that reads recordings (`info`, `cache build`, `query view`,
`values`, `stats`, `cache list`, `clear`) over each CSV in the folder. Each roadmap phase adds its commands there.

## CI (GitHub Actions)

`.github/workflows/ci.yml` has three jobs (releases: [Releasing](releasing.md)):

- **CI image**: builds `ci/Dockerfile` for amd64 and arm64 and pushes it to GHCR, tagged by a hash of its inputs, so
  it rebuilds only when a pinned version or the Dockerfile changes.
- **linux**: `just ci-linux` inside that image: lint, tests, benchmarks run once, the prototype suite, `ro-test`
  on a read-only mount, fuzz smoke, and the app built against WebKitGTK.
- **native** (macOS, Windows): `just ci-native`: lint, tests, and the app built against WKWebView and WebView2.

When a Linux job fails and you need a shell in the same environment, open the repo in GitHub Codespaces: it builds
`.devcontainer/` from the same Dockerfile, with the fixture folder mounted read-only at `/data/ro`. Or build it locally
(`docker build -f ci/Dockerfile -t tlmbench-ci .`, about 6 GB) and remove it afterwards.

## Editor

Cursor and VS Code read `.vscode/extensions.json` (rust-analyzer, Even Better TOML, Playwright, Python, basedpyright,
Ruff, CodeLLDB, EditorConfig, Tauri) and `.vscode/settings.json` (clippy on save, the `.venv` interpreter, `target/`
excluded from watching). basedpyright stands in for Pylance, which is VS Code only.

## The viewer, browser-only mode and Python

`crates/web` (`tlmbench-web`) is the core compiled to WASM: `tlmbench-core` plus a small store of columns in memory, one `call`
entry (no wasm-bindgen), and the decoder's CSV reader, descriptor and `.tlm` header code included as source (`#[path]` modules). Browser-only mode's Parquet reader is a second crate, `crates/web-parquet` (same layout, its own target): it includes the decoder's `parquet_core.rs` (the rules the app's reader uses too), reads with pure-Rust codecs (a shim crate stands in for `zstd`, which binds to C, and decodes with `ruzstd`), and the page fetches its WASM only when a Parquet file is opened. `just viewer` builds both; `just test` runs both crates' tests natively. It is outside the workspace (its own
`Cargo.lock` and `target/`; it needs the `wasm32-unknown-unknown` target that `rust-toolchain.toml` lists). `just viewer` builds it,
then the UI twice with Vite (`app/ui/vite.viewer.config.ts` and `vite.browser.config.ts`: relative paths, so a folder of either
opens from any place on a static host). `t_viewer` and `t_browseronly` (`app/ui`) test them from a plain file server and skip when
the build isn't there.

`python/` is the `tlmbench` package (pyo3, abi3, built by maturin; its own workspace and target folder too). `just test` runs
`maturin develop -m python/Cargo.toml` into the venv and then `pytest python`; if `VIRTUAL_ENV` and `CONDA_PREFIX` are both set,
unset one (maturin refuses). The package is the same core as the CLI: `python/tests` compares its HTML export with the CLI's
checksums.

Gate 5 (`just gate5`, `ci/gate5.py`): writes a 2 h flight (20 channels at 100 Hz) and two short ones, runs a check set over the
folder, exports the flight with every channel and its checks, requires it to fit 250 MB in Full mode, serves the folder with a plain
static file server and opens it in headless Chromium (`ci/gate5_open.mjs`), requiring the first chart view in under 2 s and no page
errors. CI doesn't run it (it needs the viewer built and Chromium); run it on the Mac.

