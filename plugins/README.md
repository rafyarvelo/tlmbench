# Plugins

Templates for TelemetryBench plugins. Each is a standalone crate (not part of the workspace) that builds a WASM
component for `wasm32-wasip2`; the interface is `crates/plugin/wit/tlmbench.wit`.

- `template-decoder/`: a decoder (a file format). Copy the folder, rename the crate, replace the format code.
- `template-estimator/`: an alignment method (finds how a file's clock maps onto the primary's). Same way.
- `template-payload/`: a payload decoder: a descriptor names it (`plugin = "name"`) for a packet type, whose packets'
  payloads it turns into rows, or for one field (`size` bytes a record), whose bytes it turns into one value per record.
  The app frames the packets, checks their checksums and decompresses them first. Check it with
  `tlmbench plugins --check my_payload.wasm FILE --descriptor DESCRIPTOR.toml`. Browser-only mode doesn't run payload
  plugins yet: such a file opens in the app.
- `test-hostile/` and `test-native/`: used by the tests (a decoder that misbehaves, to check the sandbox; a native decoder
  written in Rust against `crates/plugin/include/tlmbench_plugin.h`). Not templates.

Start a plugin with [cargo-generate](https://github.com/cargo-generate/cargo-generate) (it asks which template and the
name of your crate), or copy a folder:

```sh
cargo generate --path /path/to/tlmbench/plugins --name my-format     # pick template-decoder, -estimator or -payload
cd my-format
cargo test                                         # the harness: your format code on this machine, against sample bytes
cargo build --release --target wasm32-wasip2
tlmbench plugins --check target/wasm32-wasip2/release/my_format.wasm some-file.bin   # as the app runs it
# the .wasm goes in the plugins folder (Settings › Plugins › Open folder); the app reloads it when it changes
```

Each template's `cargo test` is its harness: the code that turns bytes into values runs natively (the host's imports are
stubs off WASM, so test the functions that take bytes, as the templates do). `tlmbench plugins --check` then runs the
built component in the app's sandbox, with its limits, and says what it decoded.

`just plugin-template` builds the templates and checks that each copy of the WIT matches the app's.

What a plugin can do and see is in `crates/plugin/wit/tlmbench.wit` (WASM) and `crates/plugin/include/tlmbench_plugin.h`
(native). WASM plugins get no files, network or clock, a CPU budget per call (30 s for a decoder's chunk, 2 s for an
estimator) and a memory cap (1 GB, 256 MB). Native plugins have none of that, so the app loads them only from your own
plugins folder and only after you turn each one on.
