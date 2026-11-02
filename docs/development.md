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
| `just app-dev [file]`, `just app-build` | run the desktop app with hot reload (WKWebView), or build it without bundling |
| `just app-shot [file] [out]` | screenshot the release app's window to `target/app-shot.png` (Mac; the editor needs Screen Recording in System Settings › Privacy & Security) |
| `just app-icons` | regenerate `icon.png`, `icon.icns` and `icon.ico` from `app/src-tauri/icons/icon.svg` (Mac) |
| `just gen …`, `just gen-gate` | write synthetic recordings (sine + noise + spikes + gaps) to `$TLMBENCH_DATA` (default `~/tlmbench-data`) |
| `just gate [file]` | the Phase 1 gate in the release app (below) |
| `just gen-gate2`, `just gate2 [file] [keep]` | write the 10 GB Gate 2 recording; run the Phase 2 gate over it (below) |
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

`ro_commands` runs `ci/ro_cli.sh`: every CLI command that reads recordings (`info`, `cache build`, `query view`,
`values`, `stats`, `cache list`, `clear`) over each CSV in the folder. Each roadmap phase adds its commands there.

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

## The Phase 1 gate

`just gen-gate` writes the gate recording (100M samples, one channel, as `.tlm` ~400 MB and CSV ~2.4 GB) to
`$TLMBENCH_DATA/gate`. `just gate` builds the release app and runs a scripted 30 s sweep over it (zoom from the whole
file to 1 s and back, pans), then prints a JSON report and exits 0 on pass. Keep the window visible: a hidden window
or sleeping display pauses animation frames, and the run is reported as invalid.

Pass: at most 1% of frames miss a vsync and no interval exceeds 3 refresh periods; the core reduces views of up to
1M samples in p95 < 5 ms (timed in Rust; the webview's clocks round to 1 ms). Whole-file views are reported only.

Last run (MacBook Air M1, 8 GB, 59 Hz, 2264 px): pass. 1799 frames, 3 missed; reduce p95 1.4 ms for views of up to
1M samples, whole-file p50 21 ms (657 ms on a cold first read); round trip p50 4 ms.

## The Phase 2 gate

`just gen-gate2` writes the Gate 2 recording (a 10 GB `.tlm`, 20 channels × 125M f32 samples) to
`$TLMBENCH_DATA/gate2` in about 30 s. It needs about 12 GB free: the file and its 1.1 GB pyramid. `just gate2` builds the
pyramid with the CLI into a fresh cache, drops the file's and the cache's pages from memory (`ci/evict.py`, no root),
and times a reopen in the release app; then it clears the cache and times a first-ever open. It prints two JSON reports,
exits 0 when the reopen passes, and deletes the 10 GB file afterwards (`just gate2 <file> 1` keeps it). Keep the window
visible.

Pass: the reopen's first paint of the whole view (all 20 channels, coarse counts) within 1 s of asking to open.
Reported: when the exact view replaced it, the 30 s sweep of Phase 1 over all 20 channels, and for the first-ever open,
when axes and progress showed and how long the build took.

Last run (MacBook Air M1, 8 GB, 1736 px): pass. First paint 142 ms (coarse), exact 1.9 s; sweep 12 of 1,778 frames
missed (0.7%), longest 245 ms, 20-trace replies p50 1.6 ms in the core. First-ever open: axes and progress in 84 ms,
build 20 s. That run's disk was nearly full, which slowed writes to 74 MB/s (the disk reads the file cold at
1.06 GB/s, 9.5 s); the build is to be measured again with space free.

