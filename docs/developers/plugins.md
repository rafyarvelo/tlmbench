# Plugins

Custom code runs as a plugin: a **WASM component** (sandboxed, the default choice) or a **native library** (trusted, for
speed). The app never runs Python or other programs. Three kinds of plugin use the WASM interface:

- a **decoder** turns a file format into channels (`world decoder`),
- a **payload decoder** turns the payload bytes of a packet type, or the bytes of one field, into values; a descriptor
  names it with `plugin = "name"`, and the app frames the packets, checks their checksums and decompresses them first
  (`world payload`),
- an **alignment estimator** finds how one file's clock maps onto the primary's (`world estimator`).

`crates/plugin` (`tlmbench-plugin`) hosts them. The interface is the WIT package
[`crates/plugin/wit/tlmbench.wit`](../../crates/plugin/wit/tlmbench.wit) (`tlmbench:plugin`, three worlds) and, for
native decoders, [`crates/plugin/include/tlmbench_plugin.h`](../../crates/plugin/include/tlmbench_plugin.h) (C, ABI 1).
The WIT file is the reference: read it for the exact records and what each function must do.

## Start a plugin

`plugins/` holds standalone crates, each with its own `target/`, outside the Cargo workspace:

| Folder | What |
| --- | --- |
| `template-decoder/` | A decoder. Copy the folder, rename the crate, replace the format code. |
| `template-estimator/` | An alignment method. Same way. |
| `template-payload/` | A payload decoder. Check it with `tlmbench plugins --check my_payload.wasm FILE --descriptor DESCRIPTOR.toml`. Browser-only mode doesn't run payload plugins: such a file opens in the app. |
| `test-hostile/`, `test-native/` | Used by the tests, not templates: a decoder that misbehaves (loops, over-allocates, reads files and the clock, returns a bad chunk) to check the sandbox, and a native decoder written in Rust against the C header. |

Start with [cargo-generate](https://github.com/cargo-generate/cargo-generate) (it asks which template and the name of your
crate, `cargo binstall cargo-generate@0.26.0`), or copy a folder:

```sh
cargo generate --path /path/to/tlmbench/plugins --name my-format     # pick template-decoder, -estimator or -payload
cd my-format
cargo test                                         # the harness: your format code on this machine, against sample bytes
cargo build --release --target wasm32-wasip2
tlmbench plugins --check target/wasm32-wasip2/release/my_format.wasm some-file.bin   # as the app runs it
# the .wasm goes in the plugins folder (Settings › Plugins › Open folder); the app reloads it when it changes
```

Each template's `cargo test` is its harness: the code that turns bytes into values runs natively (the host's imports are
stubs off WASM, so test the functions that take bytes, as the templates do). `tlmbench plugins --check` then runs the built
component in the app's sandbox, with its limits, and says what it decoded. A plugin can live in its own repository: it
needs only the WIT package.

A plugin exports `start`, not `open`: a guest's export named `open` collides with the C library's symbol.

## What a plugin can do

| | WASM component | Native library |
| --- | --- | --- |
| Files, network, clock | none: a decoder reads the file only through `source` (byte ranges), an estimator gets only the series it asked for | everything the user can |
| CPU | a budget per call: 30 s for a decoder's chunk, 2 s for an estimator | none |
| Memory | a cap: 1 GB for a decoder, 256 MB for an estimator | none |
| Where it is read from | your plugins folder, the open folder's `.tlmbench/plugins/`, the CLI's `--plugins DIR` | your plugins folder only, and only after you turn each one on in Settings |
| In a browser tab | decoders and estimators run (a worker) | never |

Where the plugins folder is: `plugins/` in the app's data folder; `tlmbench plugins` and Settings › Plugins show the path.

## The C interface

A native decoder is a shared library (`.so`, `.dylib`, `.dll`) exporting `tlm_abi`, `tlm_describe`, `tlm_sniff`,
`tlm_open`, `tlm_next` and `tlm_close` (see the header's comments for the rules: UTF-8 strings that are not
NUL-terminated, pointers valid until the next call on the session, never let a panic or exception cross). It runs in the
app with the user's own rights. Use WASM unless you need native speed.

## Keeping the copies of the interface in step

Each WASM template keeps its own copy of the WIT file. `just plugin-template` builds the templates and fails if a copy
differs from the app's. The plugin tests (`cargo test -p tlmbench-plugin`, `-p tlmbench --test plugins` — these two build
other feature sets than the workspace run does, so on a small disk prefer `cargo nextest run --workspace`) build the crates
themselves with a nested `cargo build --release` for `wasm32-wasip2`, so the first run adds about a minute.
