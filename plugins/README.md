# Plugins

Templates for TelemetryBench plugins. Each is a standalone crate (not part of the workspace) that builds a WASM
component for `wasm32-wasip2`; the interface is `crates/plugin/wit/tlmbench.wit`.

- `template-decoder/`: a decoder (a file format). Copy the folder, rename the crate, replace the format code.
- `template-estimator/`: an alignment method. Same way.
- `template-payload/`: a payload decoder for a packet type or one field, named by a descriptor (`plugin = "name"`).
- `test-hostile/` and `test-native/`: used by the tests. Not templates.

```sh
cargo generate --path /path/to/tlmbench/plugins --name my-format     # pick a template
cd my-format && cargo test && cargo build --release --target wasm32-wasip2
tlmbench plugins --check target/wasm32-wasip2/release/my_format.wasm some-file.bin
```

What a plugin can do, its limits, the native (C) interface and how the templates are kept in step are in the
developers guide: `docs/developers/plugins.md` in the source repository.
