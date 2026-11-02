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

# The app's web UI, built once (the app crate embeds it, so clippy and tests need it)
[private]
ui:
    [ -d app/ui/dist ] || (cd app/ui && npm run build)

# rustfmt, clippy (warnings are errors) and the UI's type check
lint: ui
    if [ -f Cargo.toml ]; then cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings; \
    else echo "lint: no Cargo workspace yet, skipping"; fi
    cd app/ui && npm run -s typecheck

# Rust unit, property and doc tests, then the Python binding tests
test: ui
    if [ -f Cargo.toml ]; then cargo nextest run --workspace --no-tests=pass && cargo test --workspace --doc; \
    else echo "test: no Cargo workspace yet, skipping Rust tests"; fi
    if [ -d python ]; then VIRTUAL_ENV="$(dirname "$(dirname "{{python}}")")" "{{python}}" -m maturin develop -m python/Cargo.toml \
      && "{{python}}" -m pytest python; else echo "test: no python/ yet, skipping binding tests"; fi

# Criterion benchmarks (run on the Mac: CI runners are too noisy for timing); extra args go to cargo bench
bench *args:
    if [ -f Cargo.toml ]; then cargo bench --workspace {{args}}; else echo "bench: no Cargo workspace yet, skipping"; fi

# Run every benchmark once, untimed, so CI knows they still build and run
bench-smoke: ui
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
        mkdir -p "$d/corpus/$t" "$d/seeds/$t"   # seeds/<target>: committed starting inputs
        (cd "$d/.." && cargo +"$RUST_NIGHTLY" fuzz run "$t" "fuzz/corpus/$t" "fuzz/seeds/$t" -- -max_total_time={{secs}})
      done
    done

# Fuzz one target until stopped: just fuzz decode reference
fuzz crate target:
    mkdir -p crates/{{crate}}/fuzz/corpus/{{target}} crates/{{crate}}/fuzz/seeds/{{target}}
    cd crates/{{crate}} && cargo +"$RUST_NIGHTLY" fuzz run {{target}} fuzz/corpus/{{target}} fuzz/seeds/{{target}}

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

# Run the desktop app with hot reload (Mac: WKWebView, as shipped); extra args go to the app (a file to open)
app-dev *args:
    cd app/src-tauri && cargo tauri dev -- -- {{args}}

# Build the desktop app without bundling (CI builds it against WebKitGTK, WKWebView and WebView2)
app-build:
    cd app/src-tauri && cargo tauri build --debug --no-bundle

# Regenerate the app icons from icons/icon.svg (Mac: sips and iconutil): icon.png 1024 px, icon.ico, icon.icns
app-icons:
    #!/usr/bin/env bash
    set -euo pipefail
    d="$PWD/app/src-tauri/icons"; tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
    (cd prototype/tests && node ../../tools/icons/render.mjs "$d/icon.svg" "$d/icon.png" 1024)
    set="$tmp/icon.iconset"; mkdir "$set"
    for n in 16 32 128 256 512; do
      sips -z $n $n "$d/icon.png" --out "$set/icon_${n}x${n}.png" > /dev/null
      sips -z $((n * 2)) $((n * 2)) "$d/icon.png" --out "$set/icon_${n}x${n}@2x.png" > /dev/null
    done
    iconutil -c icns "$set" -o "$d/icon.icns"
    for n in 24 48 64; do sips -z $n $n "$d/icon.png" --out "$tmp/$n.png" > /dev/null; done
    python3 tools/icons/ico.py "$d/icon.ico" "$set/icon_16x16.png" "$tmp/24.png" "$set/icon_32x32.png" \
      "$tmp/48.png" "$tmp/64.png" "$set/icon_256x256.png"

# Screenshot the release app's window (Mac; needs Screen Recording for the editor): opens a file, waits, captures
app-shot file=(data / "gate/gate-100M.tlm") out="target/app-shot.png" wait="3":
    #!/usr/bin/env bash
    set -euo pipefail
    cd app/src-tauri && cargo tauri build --no-bundle && cd ../..
    mkdir -p target && swiftc -O tools/winid/winid.swift -o target/winid
    target/release/tlmbench-app "{{file}}" > /dev/null 2>&1 & pid=$!
    trap 'kill $pid 2> /dev/null || true' EXIT
    id=""; for _ in $(seq 40); do id=$(target/winid tlmbench-app | head -1 | cut -f1); [ -n "$id" ] && break; sleep 0.25; done
    [ -n "$id" ] || { echo "app window not found" >&2; exit 1; }
    sleep {{wait}}
    screencapture -x -o -l "$id" "{{out}}"
    echo "{{out}}"

# The Phase 1 gate: a 30 s scripted sweep in the release app over the gate recording (`just gen-gate` first)
gate file=(data / "gate/gate-100M.tlm"):
    cd app/src-tauri && cargo tauri build --no-bundle
    # caffeinate keeps the display awake: a sleeping display pauses animation frames and invalidates the run
    TLMBENCH_GATE="{{file}}" {{ if os() == "macos" { "caffeinate -d" } else { "" } }} target/release/tlmbench-app

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
