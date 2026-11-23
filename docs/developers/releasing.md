# Releasing

`.github/workflows/release.yml` runs on a tag `v<version>` (a final release) or `v<version>-rc.N` (a dry run of it, a
pre-release), and by hand (Actions › Release › Run workflow), which builds and tests everything and publishes nothing.

- **version**: `tools/check_versions.py` (also `just versions`): the workspace version in `Cargo.toml` is the one
  version; `tauri.conf.json`, `python/pyproject.toml`, `python/Cargo.toml` and the WASM crates must say the same, and
  the tag must name it.
- **app** (macOS universal, Windows, Linux): `just viewer`, the CLI built for the target and put in
  `app/src-tauri/binaries/tlmbench-<target>` (the bundle ships it beside the app), then `cargo tauri build` with
  `tauri.release.conf.json` merged over `tauri.conf.json`: bundles on, the CLI, the plugin templates and the example
  `.tlmbench/` as resources, `.tlm` registered (CSV, TSV and Parquet offered in Open With), the hardened runtime with
  `entitlements.plist` (the WASM runtime's JIT, native plugins). The base config keeps bundling off, so a development
  build and CI need none of this. `just bundle-mac` builds the same `.app` and `.dmg` on a Mac.
- **smoke**: each installer installed on a clean runner of its OS, then `tools/smoke.mjs` (also `just smoke`) from the
  installed files: `tlmbench --version`, a recording opened in a read-only folder through the app's daemon and read
  back, nothing written next to it. Then it is uninstalled.
- **cli** (macOS arm64 and x86_64, Windows, Linux x86_64 and arm64): an archive each; **docker**: the Linux binaries as
  `ghcr.io/<owner>/tlmbench` (`cli/Dockerfile`); **site**: browser-only mode as a zip; **wheels**: `python.yml`,
  published to PyPI for a final tag.
- **release** (tags only): every file, `SHA256SUMS`, a build-provenance attestation for each, and a draft GitHub
  Release with the CHANGELOG's entry. The owner reads it and publishes it.

Signing (nothing is paid for in v1): macOS builds are ad-hoc signed (`signingIdentity: "-"` in
`tauri.release.conf.json`) and not notarised, so a user opens the app once through System Settings › Privacy & Security
› Open Anyway; Windows is signed by SignPath Foundation once it accepts the project (unsigned until then). The secrets
below stay supported for a paid identity later: `APPLE_CERTIFICATE` (a base64 Developer ID Application `.p12`),
`APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` (an app-specific password) and
`APPLE_TEAM_ID` (tauri signs and notarises), and `WINDOWS_CERTIFICATE` (a base64 `.pfx`) with
`WINDOWS_CERTIFICATE_PASSWORD`. Without them the build is unsigned and the smoke tests still run (the macOS one reports
Gatekeeper's verdict). PyPI's trusted publisher must name `release.yml` (the workflow a tag starts) as well as
`python.yml`, with the `pypi` environment.

## The runbook

1. `main` is green on all three CI jobs and the version in `Cargo.toml` is the one you mean to release
   (`just versions` checks that every manifest agrees).
2. `CHANGELOG.md` has the entry for the version, by user-facing area.
3. Run the release candidate's gates ([Gates](gates.md#the-release-candidates-pass)) on a build of this commit.
4. Tag `v<version>-rc.N` and push it: the workflow builds, smoke-tests and drafts a pre-release. Download every
   installer on each OS and follow the owner's test plan (`docs/release/test-plan.md`).
5. Fix what fails, tag the next `rc`. When a candidate passes, tag `v<version>`: the workflow publishes the wheels to PyPI
   (the trusted publisher must name `release.yml` and `python.yml`, with the `pypi` environment) and drafts the release.
6. Read the draft: every installer, the CLI archives, `SHA256SUMS`, attestations, `LICENSE` and
   `THIRD_PARTY_LICENSES.md`, the manual inside the app. Publish it.

A tag is outward-facing and can't be taken back: the owner pushes it.

## The web copy

`just pages OUT` builds the web copy into `OUT`: a landing page (`docs/web/index.html`), the manual as HTML at `manual/`
(`tools/manual/build.mjs --web`), browser-only mode at `app/` and a sample export of the example flight at `sample/`
(`tools/pages.py`). Every address is relative, so it works under any path on a static host. The app's Help › Online
manual opens `https://rafyarvelo.github.io/tlmbench/manual/`.

`.github/workflows/pages.yml` builds it and publishes it to GitHub Pages. It runs by hand (Actions › Pages › Run workflow)
and when a release is published, never on a push. Turning Pages on is a repository setting, and it is the owner's:
Settings › Pages › Source: GitHub Actions. Until then the workflow's deploy step fails and nothing else is affected.
