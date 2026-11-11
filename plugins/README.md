# Plugins

Templates for TelemetryBench plugins. Each is a standalone crate (not part of the workspace) that builds a WASM
component for `wasm32-wasip2`; the interface is `crates/plugin/wit/tlmbench.wit`.

- `template-decoder/`: a decoder (a file format). Copy the folder, rename the crate, replace the format code.

```sh
cd template-decoder
cargo build --release --target wasm32-wasip2
# target/wasm32-wasip2/release/my_decoder.wasm goes in the plugins folder (Settings › Plugins › Open folder)
```

`just plugin-template` builds the template and checks that its copy of the WIT matches the app's.
