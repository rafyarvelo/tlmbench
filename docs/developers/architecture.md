# Architecture

One Rust core answers every question about a recording. Four front ends use it: the desktop app, the command line, the
Python package and, compiled to WebAssembly, the viewer in an HTML export and browser-only mode. A web UI is the only
interface; it runs inside the desktop app's window, in a browser tab against the app's daemon, in an export's viewer, or
alone in a browser tab.

```
 files on disk (read-only)            .tlmbench/ (Share only)
        │                                    │
        ▼                                    ▼
 ┌──────────────┐   ┌──────────────┐   ┌───────────────┐
 │ tlmbench-    │──▶│ tlmbench-    │◀──│ tlmbench-expr │  expressions: derived channels,
 │ decode       │   │ store        │   └───────────────┘  rules, checks
 │ CSV, Parquet,│   │ cache, pyramid,   ▲
 │ records,     │   │ checks, events,   │            ┌───────────────┐
 │ packets,     │   │ alignment,        │            │ tlmbench-live │ sockets, tailed files,
 │ plugins      │   │ shared config     │            │               │ ring buffer, recorder
 └──────┬───────┘   └──────┬───────┘    │            └───────┬───────┘
        └──────┬───────────┘            │                    │
               ▼                        │                    │
        ┌──────────────┐  ┌─────────────┴──┐  ┌──────────────┴─┐
        │ tlmbench-core│  │ tlmbench-export│  │ tlmbench-plugin│  WASM (wasmtime) and native
        │ time, columns│  │ CSV, Parquet,  │  │ decoders,      │  decoders, alignment estimators
        │ M4, pyramid, │  │ Arrow, HDF5,   │  └────────────────┘
        │ wire format  │  │ MAT, HTML      │
        └──────┬───────┘  └────────────────┘
               │
   ┌───────────┼───────────────────┬────────────────────┐
   ▼           ▼                   ▼                    ▼
 app/src-tauri   cli (tlmbench)   python (tlmbench)   crates/web → WASM → viewer, browser-only
 Tauri shell +   one binary,      pyo3 package,       (core + expr, no store, no threads)
 daemon          no window        same core
   │
   ▼
 app/ui: Svelte + TypeScript, one codebase for the window, a browser tab (daemon), the viewer and browser-only mode
```

## The core (`crates/core`, `tlmbench-core`)

Pure algorithms, no files: the time base (nanoseconds, with rate, field, counter and index sources), columns of
values, M4 downsampling, the min/max **pyramid** that answers a view of any span from a few thousand summaries,
gap detection, masks, time maps (alignment) and the **wire format** of a view reply
([formats](formats.md#the-view-reply)). It has no dependencies on the rest, so it compiles to WASM unchanged.

## Decoders (`crates/decode`)

Every format opens as a `Recording` that the CLI and the app query the same way. CSV and Parquet are decoded once,
in parallel, into a columnar cache and then read through memory maps. Fixed-width records are read in place from the
mapped file, laid out by a **descriptor** (a TOML file); the `.tlm` reference format carries its descriptor in its
header. A descriptor with `[packets]` frames records in packets (sync word, length, checksum, compression), with a
packet index built once in the cache. A format nothing claims can be decoded by a plugin. Nothing here writes next to
the data: the only writes are cache files under the cache root.

## The store (`crates/store`)

Recordings opened with their cache entries, which live only under the cache root. The CLI and the app share one root
(`--cache`, else `$TLMBENCH_CACHE`, else the OS cache folder). Each recording gets a pyramid and a gap index, **built
once in the background** and resumable, so a view of a huge file comes from summaries while the build runs and the UI
hatches what isn't summarised yet. The store also holds derived channels, checks and rules, events, alignment of several
files, the chunk cache for interleaved channels, and the shared configuration reader and writer.

## Expressions (`crates/expr`)

The parser, type checks and chunked evaluator behind derived channels, rules and checks. One language everywhere: the
Derive dialog, a check's ƒ mode, a table filter, `tlmbench derive`. The function table is
[generated into the reference](../reference/expressions.md).

## Live (`crates/live`)

A source (serial, UDP, TCP, a tailed file, file playback) runs two threads: one reads frames into a **ring buffer**, one
records them to a session folder as they arrive. The UI follows the ring; the recording is an ordinary `.tlm` folder
afterwards. A source counts lost frames from a sequence field the descriptor names.

## Plugins (`crates/plugin`)

Custom decoders, payload decoders and alignment estimators run as WASM components on wasmtime with no files, network or
clock and a CPU and memory budget; native decoders are trusted code the user turns on one by one. See
[Plugins](plugins.md).

## Export (`crates/export`)

A recording's samples to CSV, Parquet, Arrow IPC, HDF5 or MAT v7.3, streamed chunk by chunk so memory stays flat, and the
**HTML export**: a folder (or one file) with the viewer, the core as WASM, a manifest and the data in checksummed chunks.
[`docs/exports.md`](../exports.md) says what a user sees; [`docs/viewer-store.md`](../viewer-store.md) says what an
export holds and the integrity rule: a viewer only ever shows a raw sample or an exact aggregate.

## The app (`app/src-tauri`, `app/ui`)

A Tauri shell around the core. The UI calls **commands** (`launch`, `open`, `view`, `values_at`, `stats`, `rows`,
`derive_apply`, …) over Tauri's IPC; a view reply is the binary wire format, which the UI reads in place as typed arrays.
The UI is Svelte and TypeScript, built by Vite; charts draw on canvas, and the table uses TanStack's table core. The
shell also builds the native menu bar from one table (`app/ui/src/commands.ts`), embeds the viewer and the CLI, and hosts
sandboxed code widgets in a frame served from its own scheme.

**The daemon.** `tlmbench-app --daemon` (or Settings › Browser access) serves the same commands over a loopback
WebSocket to browser tabs, each with a hidden window of its own, so a tab behaves as a window. A token made for the
session guards it. The UI runs unchanged in a tab; Explorer, tabs and everything else work as in the window.

## The viewer and browser-only mode (`crates/web`, `crates/web-parquet`)

`tlmbench-web` is the core and the expression engine compiled to WASM, with a small in-memory column store and one `call`
entry point (no wasm-bindgen). The same UI is built twice more with Vite:

- **The viewer** ships inside every HTML export (`app/viewer`). It reads the export's manifest and chunks with `fetch`,
  opens **Locked**, and leaves out whatever needs the app.
- **Browser-only mode** (`app/browser/dist`) is a static site: open files in a tab, with the browser's folder picker where
  it has one; Parquet's reader is a second WASM module fetched only when needed. Several recordings in one tab, live
  sources, sharing and native plugins need the app.

`just viewer` builds both; the CLI and the app embed the viewer pack (`app/viewer/viewer.pack`), so build it first.

## The CLI and Python (`cli`, `python`)

`tlmbench` is one binary over the same crates: `info`, `inspect`, `derive`, `cache`, `query`, `convert`, `export`,
`check`, `validate`, `config`, `record`, `plugins`. Exit codes of `check` are fixed (0, 1, 2, 3) so CI can rely on them.
The Python package (`python/`, pyo3 with the stable ABI, built by maturin) wraps the same core: what Python reads is what
the app reads. [`docs/python-api.md`](../python-api.md) is generated from its docstrings.

## Where state lives

| State | Where | Written by |
| --- | --- | --- |
| The pyramid and gap index | the cache root, `pyr/v2/<key>/` | the store, in the background |
| Decoded CSV columns, derived channels | the cache root, `csv/<key>/` and `derived/<key>/` | the decoder and the store |
| Events you add to a recording | the cache root, `events/<fingerprint>.json` | the core |
| Settings, sessions, recent files, your layouts | `settings.json` in the app's config folder | the app |
| Shared configuration | the folder's `.tlmbench/` | **Share** only |
| Recorded live sessions | the Recordings folder (Settings) | the recorder |
| Exports | where you point them | Export |
