# Mac tools: `brew bundle`, then `ci/setup.sh`. Exact versions are pinned in CI (ci/versions.env); Homebrew keeps
# these current. Rust, just and the cargo tools come from ci/setup.sh at their pinned versions, not Homebrew.
brew "coreutils"            # GNU timeout, used by prototype/tests/run.sh
brew "python@3.12"
# Node LTS: the official installer from nodejs.org (version in ci/versions.env), not Homebrew, which builds it from
# source on macOS versions it no longer ships prebuilt packages for
