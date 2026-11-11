# Plugins

Templates for TelemetryBench plugins. Each is a standalone crate (not part of the workspace) that builds a WASM
component for `wasm32-wasip2`; the interface is `crates/plugin/wit/tlmbench.wit`.

- `template-decoder/`: a decoder (a file format). Copy the folder, rename the crate, replace the format code.
- `template-estimator/`: an alignment method (finds how a file's clock maps onto the primary's). Same way.
- `test-hostile/` and `test-native/`: used by the tests (a decoder that misbehaves, to check the sandbox; a native decoder
  written in Rust against `crates/plugin/include/tlmbench_plugin.h`). Not templates.

```sh
cd template-decoder
cargo build --release --target wasm32-wasip2
# target/wasm32-wasip2/release/my_decoder.wasm goes in the plugins folder (Settings › Plugins › Open folder)
```

`just plugin-template` builds the templates and checks that each copy of the WIT matches the app's.

What a plugin can do and see is in `crates/plugin/wit/tlmbench.wit` (WASM) and `crates/plugin/include/tlmbench_plugin.h`
(native). WASM plugins get no files, network or clock, a CPU budget per call (30 s for a decoder's chunk, 2 s for an
estimator) and a memory cap (1 GB, 256 MB). Native plugins have none of that, so the app loads them only from your own
plugins folder and only after you turn each one on.
