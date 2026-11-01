# TelemetryBench tasks. The same recipes run on the Mac, in CI and in the CI image (ci/Dockerfile).
# Pinned tool versions: ci/versions.env. Recipes that need code not written yet say so and skip.

set dotenv-path := "ci/versions.env"
set dotenv-load
set shell := ["bash", "-euo", "pipefail", "-c"]

python := env("TLMBENCH_VENV", ".venv") / "bin/python"
# The data folder `ro-test` proves untouched, and whether it must be read-only (CI mounts it read-only)
ro_dir := env("TLMBENCH_RO_DIR", "prototype/tests/fold")
ro_required := env("TLMBENCH_RO_REQUIRED", "")
# Commands run against that folder (its cache goes to a temp folder); each roadmap phase adds what it brings
ro_commands := "--cmd 'c=$(mktemp -d); find \"" + ro_dir + "\" -name \"*.csv\" -exec target/debug/tlmbench info --cache \"$c\" {} +; r=$?; rm -rf \"$c\"; exit $r'"
# Generated recordings (never in the repo; see docs/follow-ups.md for the large sets)
data := env("TLMBENCH_DATA", home_directory() / "tlmbench-data")

default:
    @just --list --unsorted

# Install the pinned toolchain, Playwright's Chromium and the Python venv
setup *steps:
    ci/setup.sh {{steps}}

# rustfmt and clippy (warnings are errors)
lint:
    if [ -f Cargo.toml ]; then cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings; \
    else echo "lint: no Cargo workspace yet, skipping"; fi

# Rust unit, property and doc tests, then the Python binding tests
test:
    if [ -f Cargo.toml ]; then cargo nextest run --workspace --no-tests=pass && cargo test --workspace --doc; \
    else echo "test: no Cargo workspace yet, skipping Rust tests"; fi
    if [ -d python ]; then VIRTUAL_ENV="$(dirname "$(dirname "{{python}}")")" "{{python}}" -m maturin develop -m python/Cargo.toml \
      && "{{python}}" -m pytest python; else echo "test: no python/ yet, skipping binding tests"; fi

# Criterion benchmarks (run on the Mac: CI runners are too noisy for timing); extra args go to cargo bench
bench *args:
    if [ -f Cargo.toml ]; then cargo bench --workspace {{args}}; else echo "bench: no Cargo workspace yet, skipping"; fi

# Run every benchmark once, untimed, so CI knows they still build and run
bench-smoke:
    if [ -f Cargo.toml ]; then cargo test --workspace --benches; else echo "bench-smoke: no Cargo workspace yet, skipping"; fi

# Check the prototype's script parses
proto-check:
    node ci/proto_check.mjs

# The prototype's Playwright suite (`just proto-test t_checks t_rules` runs some)
proto-test *tests: proto-check
    cd prototype/tests && ./run.sh {{tests}}

# Fuzz each target of every crates/*/fuzz for a few seconds (nightly)
fuzz-smoke secs="30":
    #!/usr/bin/env bash
    set -euo pipefail
    shopt -s nullglob
    dirs=(crates/*/fuzz)
    [ ${#dirs[@]} -gt 0 ] || { echo "fuzz-smoke: no fuzz targets yet, skipping"; exit 0; }
    for d in "${dirs[@]}"; do
      for t in $(cd "$d/.." && cargo +"$RUST_NIGHTLY" fuzz list); do
        (cd "$d/.." && cargo +"$RUST_NIGHTLY" fuzz run "$t" -- -max_total_time={{secs}})
      done
    done

# Fuzz one target until stopped: just fuzz decode fixed_width
fuzz crate target:
    cd crates/{{crate}} && cargo +"$RUST_NIGHTLY" fuzz run {{target}}

# Prove nothing is written next to the data (see ci/ro_check.py)
ro-test:
    cargo build -q -p tlmbench
    python3 ci/ro_check.py "{{ro_dir}}" {{ if ro_required != "" { "--require-ro" } else { "" } }} {{ro_commands}}

# ro-test against a read-only disk image of the folder (macOS)
ro-test-mac:
    #!/usr/bin/env bash
    set -euo pipefail
    tmp="$(mktemp -d)"; img="$tmp/tlmbench-ro.dmg"; mnt="$tmp/mnt"; mkdir "$mnt"
    hdiutil create -quiet -srcfolder "{{ro_dir}}" -format UDRO "$img"   # UDRO: a read-only image
    hdiutil attach -quiet -readonly -nobrowse -mountpoint "$mnt" "$img"
    trap 'hdiutil detach -quiet "$mnt"; rm -rf "$tmp"' EXIT
    TLMBENCH_RO_DIR="$mnt" TLMBENCH_RO_REQUIRED=1 just ro-test

# Run the desktop app with hot reload (Mac: WKWebView, as shipped)
app-dev:
    if [ -d app ]; then cd app && cargo tauri dev; else echo "app-dev: no app/ yet (Phase 1 adds it)"; fi

# Build the desktop app without bundling (CI builds it against WebKitGTK, WKWebView and WebView2)
app-build:
    if [ -d app ]; then cd app && cargo tauri build --debug --no-bundle; else echo "app-build: no app/ yet, skipping"; fi

# Everything the Linux CI job runs (inside the CI image)
ci-linux: lint test bench-smoke proto-test ro-test fuzz-smoke app-build

# Everything the macOS and Windows CI jobs run
ci-native: lint test app-build

# Write synthetic recordings: just gen --samples 10M --channels 4 (see tools/gen)
gen *args:
    cargo run -q --release -p tlmbench-gen -- {{args}}

# The Phase 1 gate recording: 100M samples of one channel, as .tlm (~400 MB) and CSV (~2 GB)
gen-gate:
    cargo run -q --release -p tlmbench-gen -- --samples 100M --channels 1 --out "{{data}}/gate" --name gate-100M

# Remove Rust build output (it grows to 10+ GB over time)
clean:
    cargo clean
