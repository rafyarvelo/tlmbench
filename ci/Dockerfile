# CI image: the Linux side of CI, and a Linux shell for debugging a CI failure (Codespaces via .devcontainer/, or
# `docker build -f ci/Dockerfile .`). Built for linux/amd64 and linux/arm64. Versions come from rust-toolchain.toml,
# ci/versions.env, requirements-dev.txt and prototype/tests/package.json, installed by ci/setup.sh as on the Mac.
FROM mcr.microsoft.com/devcontainers/base:ubuntu-24.04@sha256:d94c97dd9cacf183d0a6fd12a8e87b526e9e928307674ae9c94139139c0c6eae

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PLAYWRIGHT_BROWSERS_PATH=/ms-playwright \
    TLMBENCH_VENV=/opt/venv \
    PATH=/usr/local/cargo/bin:/opt/venv/bin:$PATH

# Build tools, Python 3.12 (Ubuntu 24.04's own) and the Tauri v2 Linux dependencies (WebKitGTK 4.1)
RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential cmake pkg-config curl wget file xz-utils unzip ca-certificates git libssl-dev \
      python3.12 python3.12-venv python3.12-dev \
      libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev librsvg2-dev \
      libayatana-appindicator3-dev libxdo-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /tmp/setup
COPY ci/versions.env ci/

# Node LTS from nodejs.org, checksum verified
RUN . ci/versions.env && arch=$(dpkg --print-architecture | sed 's/amd64/x64/') \
    && f=node-v$NODE_VERSION-linux-$arch.tar.xz \
    && curl -fsSLO https://nodejs.org/dist/v$NODE_VERSION/$f \
    && curl -fsSL https://nodejs.org/dist/v$NODE_VERSION/SHASUMS256.txt | grep " $f\$" | sha256sum -c - \
    && tar -xJf $f -C /usr/local --strip-components=1 --exclude=CHANGELOG.md --exclude=README.md --exclude=LICENSE \
    && rm $f

COPY rust-toolchain.toml requirements-dev.txt ./
COPY ci/setup.sh ci/
COPY prototype/tests/package.json prototype/tests/package-lock.json prototype/tests/
RUN ci/setup.sh rust cargo-tools wasm playwright python \
    && rm -rf prototype/tests/node_modules $CARGO_HOME/registry /root/.npm /root/.cache \
    && chown -R vscode:vscode $RUSTUP_HOME $CARGO_HOME $TLMBENCH_VENV

WORKDIR /work
