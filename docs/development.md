# Development

Development is native on the Mac; Linux runs in CI, in an image built from `ci/Dockerfile`. The same `just`
recipes run in both places, and every tool version is pinned in one place (below).

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

| Recipe | What it does |
| --- | --- |
| `just lint` | rustfmt check and clippy, warnings are errors |
| `just test` | Rust unit, property and doc tests (nextest), then the Python binding tests |
| `just bench` | criterion benchmarks; run them on the Mac, since CI runners are too noisy for timing |
| `just proto-test [t_name …]` | syntax-check the prototype, then its Playwright suite |
| `just fuzz-smoke [secs]`, `just fuzz <crate> <target>` | fuzz every target briefly, or one until stopped (nightly) |
| `just ro-test`, `just ro-test-mac` | prove nothing is written next to the data (below) |
| `just app-dev`, `just app-build` | run the desktop app with hot reload (WKWebView), or build it without bundling |
| `just ci-linux`, `just ci-native` | everything a CI job runs |

Recipes for code that doesn't exist yet (no Cargo workspace, no `app/`, no `python/`) say so and skip.

## Pinned versions

| What | Where |
| --- | --- |
| Rust stable, components, wasm targets | `rust-toolchain.toml` |
| Node, nightly, just, cargo-binstall, nextest, cargo-fuzz, tauri-cli, wasmtime, wasm-tools | `ci/versions.env` |
| Playwright | `prototype/tests/package.json` (+ lock file) |
| maturin, pytest, numpy, pyarrow | `requirements-dev.txt` |
| CI base image | digest in `ci/Dockerfile` |

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

Each roadmap phase adds its CLI and core calls to `ro_commands`.

## CI (GitHub Actions)

`.github/workflows/ci.yml` has three jobs:

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
