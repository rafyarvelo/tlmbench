# Contributing

Thanks for helping. Bug reports, plugins and fixes are all welcome.

## Reporting a bug

Open an issue with what you did, what you expected and what happened, the version (the About window, or
`tlmbench --version`) and your OS. A small file that shows it, or the steps to make one with `tlmbench-gen`, makes it
quick to fix. Security problems go privately: see [SECURITY.md](SECURITY.md).

## Ideas

New features are welcome as issues. TelemetryBench follows a design ([`docs/design.md`](docs/design.md)); a proposal
that fits it, or says where it would change it, is easiest to take on.

## Changing the code

- Set up as [the developers guide](docs/developers/building-and-testing.md) says (`brew bundle`, `ci/setup.sh`; the same recipes run in
  CI through the `justfile`).
- Keep the hard requirements: the app works from its cache alone on a read-only folder; nothing is written next to the
  data except by Share and exports; exports are static files; the app never runs Python or other programs (custom code
  is a WASM or native plugin).
- Every behaviour change comes with a test: Rust tests beside the code, the UI's Playwright tests in `app/ui/tests`
  (their README lists what each covers).
- Before a pull request: `just lint test` and `just app-test` pass, and `cargo fmt --all` leaves nothing to change.
- Small commits with clear messages, one topic each.

## Plugins

Decoders, payload decoders and alignment estimators are WASM components; start from the templates in
[`plugins/`](docs/developers/plugins.md) (`cargo generate`). A plugin can live in its own repository: it needs only the WIT
package in `crates/plugin/wit`.

## Licence

By contributing you agree that your contribution is licensed under the MIT License or the Apache License 2.0, at
the choice of whoever uses it, as the project is ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)).
