#!/usr/bin/env bash
# Installs the pinned toolchain. Used by `just setup` on the Mac and by ci/Dockerfile, so both get the same versions.
# Usage: ci/setup.sh [rust] [cargo-tools] [wasm] [playwright] [python]   (no arguments: all of them)
set -euo pipefail
cd "$(dirname "$0")/.."
set -a; . ci/versions.env; set +a

CARGO_HOME=${CARGO_HOME:-$HOME/.cargo}
export PATH="$CARGO_HOME/bin:$PATH"
VENV=${TLMBENCH_VENV:-.venv}

rust() {
  command -v rustup >/dev/null ||
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none --profile minimal
  rustup toolchain install                     # the toolchain, components and targets in rust-toolchain.toml
  rustup toolchain install "$RUST_NIGHTLY" --profile minimal   # cargo-fuzz only
  # the pinned stable is the default outside the repo too (otherwise the first toolchain installed, the nightly, wins)
  rustup default "$(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml)"
}

cargo_tools() {
  command -v cargo-binstall >/dev/null && [[ "$(cargo-binstall -V)" == "$CARGO_BINSTALL_VERSION" ]] ||
    curl -L --proto '=https' --tlsv1.2 -sSf \
      "https://raw.githubusercontent.com/cargo-bins/cargo-binstall/v$CARGO_BINSTALL_VERSION/install-from-binstall-release.sh" |
      BINSTALL_VERSION=$CARGO_BINSTALL_VERSION bash
  # native binaries only (an x86_64 cargo-fuzz under Rosetta builds fuzz targets for x86_64); builds from source if none
  cargo binstall --no-confirm --locked --targets "$(rustc -vV | sed -n 's/^host: //p')" \
    "just@$JUST_VERSION" "cargo-nextest@$NEXTEST_VERSION" "cargo-fuzz@$CARGO_FUZZ_VERSION" "tauri-cli@$TAURI_CLI_VERSION"
}

# wasmtime and wasm-tools from their GitHub releases (no Rust target triples in the asset names, so not via binstall)
wasm() {
  local arch os
  arch=$(uname -m); [[ $arch == arm64 ]] && arch=aarch64
  case "$(uname -s)" in Darwin) os=macos ;; Linux) os=linux ;; *) echo "wasm tools: unsupported OS" >&2; return 1 ;; esac
  local tmp; tmp=$(mktemp -d)
  curl -fsSL "https://github.com/bytecodealliance/wasmtime/releases/download/v$WASMTIME_VERSION/wasmtime-v$WASMTIME_VERSION-$arch-$os.tar.xz" |
    tar -xJ -C "$tmp" --strip-components=1
  curl -fsSL "https://github.com/bytecodealliance/wasm-tools/releases/download/v$WASM_TOOLS_VERSION/wasm-tools-$WASM_TOOLS_VERSION-$arch-$os.tar.gz" |
    tar -xz -C "$tmp" --strip-components=1
  install -m 755 "$tmp/wasmtime" "$tmp/wasm-tools" "$CARGO_HOME/bin/"
  rm -rf "$tmp"
}

# Chromium for the prototype tests, matching the Playwright version in package.json. Headless shell only: the tests
# never open a window (`npx playwright install chromium` adds the full browser for headed debugging).
playwright() {
  local deps=()
  [[ "$(uname -s)" == Linux && $(id -u) == 0 ]] && deps=(--with-deps)
  (cd prototype/tests && npm ci && npx playwright install ${deps[@]+"${deps[@]}"} --only-shell chromium)
}

python() {
  local py; py=$(command -v python3.12) || { echo "python3.12 not found (brew bundle installs it)" >&2; return 1; }
  [[ -x "$VENV/bin/python" ]] || "$py" -m venv "$VENV"
  "$VENV/bin/pip" install -q --upgrade pip
  "$VENV/bin/pip" install -q -r requirements-dev.txt
}

steps=("$@"); [[ ${#steps[@]} -gt 0 ]] || steps=(rust cargo-tools wasm playwright python)
for s in "${steps[@]}"; do
  echo "== $s"
  case $s in
    rust) rust ;; cargo-tools) cargo_tools ;; wasm) wasm ;; playwright) playwright ;; python) python ;;
    *) echo "unknown step: $s" >&2; exit 2 ;;
  esac
done
