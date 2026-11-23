# TelemetryBench developers guide

For people who build, change or extend TelemetryBench: the code, the tests, the file formats, plugins and the release.
Using the app is in the [User Manual](../manual/README.md). What the product promises, and why, is in the
[design doc](../design.md); what is true today, requirement by requirement, is the [ledger](../release/ledger.md).

1. [Architecture](architecture.md): the core, the store and pyramid, the app, the viewer and browser-only mode, the daemon.
2. [The repository](repository.md): where everything is.
3. [Building and testing](building-and-testing.md): setup, the `just` recipes, pinned versions, CI, the read-only rule.
4. [The test suites](tests.md): Rust, Python, the UI tests and the fake core, the prototype's, fuzzing, screenshots.
5. [Gates and measurements](gates.md): the performance and correctness gates and how each is judged.
6. [File formats](formats.md): `.tlm`, the cache, descriptors, `.tlmbench/`, exports, the wire protocol.
7. [Plugins](plugins.md): decoders, payload decoders and alignment estimators (WASM), native decoders (C).
8. [Changing the UI](ui-conventions.md): the conventions a new control follows.
9. [Releasing](releasing.md): the release workflow as a runbook.
10. [Reference pages](../reference/): the CLI's help, the expression functions, the settings keys and the menu shortcuts,
    generated from the source (`just reference`).

## Hard requirements

Every change keeps these. They are why the design looks the way it does.

- **Full capability from the app cache alone.** Everything works on a read-only folder.
- **Nothing is written next to the data** except by a user action: Share (to the folder's `.tlmbench/`) and exports.
- **Exports are static files.** An HTML export opens from any static host or file share, with no install.
- **The app never runs Python or other external programs.** Custom code is a WASM plugin (sandboxed) or a native
  plugin; Python users work through the bindings, outside the app.
- **Everything in the design is v1.** Ideas that come later go to [`docs/post-v1.md`](../post-v1.md).

## Contribution rules

[CONTRIBUTING.md](../../CONTRIBUTING.md) is the short version: report bugs with what you did, the version and your OS;
every behaviour change comes with a test; `just lint test` and `just app-test` pass before a pull request; small commits,
one topic each; contributions are Apache-2.0. The design is the source of truth: a decision marked Accepted or Modified
is a requirement, and a change to behaviour gets a note in [`docs/follow-ups.md`](../follow-ups.md) so the design doc can be
brought up to date.
