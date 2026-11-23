# Security review (R6)

R6's security review of what a stranger can reach (`release-plan.md`, R6). One dated section per pass, kept until R8.
Each finding is fixed (with its test) or justified here in writing.

## Nov 22, 2026: dependencies, and the SVG sanitisers (first pass)

### Dependency audit

`cargo audit` (advisory database of Nov 22) on the four lock files, and `npm audit` on `app/ui` and `prototype/tests`.

**Fixed** by patch releases (no API change):

| Crate | Was | Now | Advisories |
| --- | --- | --- | --- |
| wasmtime (the plugin sandbox) and its family | 50.0.1 | 50.0.3 | RUSTSEC-2026-0566 to -0573 (a guest could abort the host or grow its memory without bound), RUSTSEC-2026-0587 (wasmtime-wasi) |
| libz-sys | 1.1.31 | 1.1.33 | RUSTSEC-2026-0544 (heap buffer overflow) |
| url | 2.5.8 | 2.5.9 | RUSTSEC-2026-0592 (stack overflow on nested `blob:` URLs) |
| ruzstd (the browser's Zstandard, `crates/web-parquet/zstd-shim`) | 0.8.3 | 0.9.1 | RUSTSEC-2026-0344 (decompression bomb) |

**npm:** no advisories in either package.

**Justified** (warnings with no fixed release; none is a vulnerability):

| Crate | Advisory | Why it stays |
| --- | --- | --- |
| chrono 0.4.45 | RUSTSEC-2026-0543, unmaintained | Formats dates; no known flaw. Replace when a maintained fork is the ecosystem's choice (post-v1). |
| glib 0.18.5 | RUSTSEC-2024-0429, unsound `VariantStrIter` | Linux only, through Tauri's GTK 3 stack (gtk, atk, muda). The app never iterates a GLib variant of its own; it goes when Tauri moves to a newer GTK binding. |
| proc-macro-error 1.0.4 | RUSTSEC-2024-0370, unmaintained | A build-time macro helper (no code of its own at run time). |
| zbus_names 4.3.5, zvariant 5.15.1, zvariant_derive 5.15.1 | RUSTSEC-2026-0370 to -0372, unmaintained | Linux only, through Tauri's D-Bus use (zbus); no known flaw. |

`cargo deny` is installed but has no `deny.toml` yet; licences are checked with R7's LICENSE (decision 1).

### User SVG (shared widgets and icons)

Widgets and icons come from other people's folders (`.tlmbench/widgets/`, `.tlmbench/icons/`) and reach exported HTML,
which opens from any host with no content policy, so the sanitisers are the only guard there (the app's own CSP blocks
outside loads and inline scripts as a second line).

**Found:** both sanitisers (the page's `sanitizeSvg` in `app/ui/src/widgets.ts`, and the export's `sanitize_svg` in
`crates/export/src/widgets.rs`) checked attribute values with a plain substring test. They missed `javascript:` with a
tab or newline inside (`java&#9;script:`; URL parsing drops them), animations that set a link or a handler afterwards
(`<set attributeName="href" to="…">`), `feImage`, and CSS that reaches outside without "http" in it (`url(//host/x)`, a
style sheet's url or `@import`: a beacon that tells a third party the file was opened), and data URLs.

**Fixed:** both read values as a browser does (character references decoded, spaces and control characters dropped),
drop animations of a link, a handler or a style, and treat any `url()` to something but a fragment, `data:` and `@import`
as reaching outside. Tests: `app/ui/tests/t_svgsafe.js` (13 hostile SVGs, one ordinary widget kept whole) and
`crates/export` `what_hides_from_a_plain_substring_check_goes_too`.

### Text from files and from people in HTML

Svelte escapes text by default, so the review read every `{@html}` and every `innerHTML` the page builds itself. Every
`{@html}` is a fixed icon or goes through `sanitizeSvg`; the string-built panels (overview picker, category and gap
dialogs, the event tooltip, Measure) escape names, units and event text with `esc`. The table is TanStack's core rendered
by Svelte.

**Found:** Measure's cursor table and stats put a channel's formatted value in raw, and a labelled channel's value is
text from the file (a CSV text column, D-ingest-12): a label like `<img src=x onerror=…>` became markup. The app's CSP
blocks the inline handler, but browser-only mode and exported HTML have no policy.

**Fixed:** Measure escapes formatted values; a custom widget's `{{value}}` is escaped before its SVG is sanitised.
Test: `app/ui/tests/t_hostilefile.js` (it fails without the fix).

### The app's policy, code widgets and the daemon

- **The app's CSP** (`tauri.conf.json`): `default-src 'self'`, inline styles only, images from itself and `data:`, IPC
  only, frames only from the widget scheme. No inline script and no outside loads: a second line behind the sanitisers.
- **Code widgets** (`tlmwidget://`, `widget-host.html`): `default-src 'none'` with inline script and `eval` only, framed
  with `sandbox="allow-scripts"` (no same-origin, no top navigation, no network, no images). The pane acts on a message
  only when it comes from its own frame, and only on "ready". Shared code widgets wait for the person's trust.
- **The daemon** (`daemon.rs`): loopback only; the socket needs the session token and a local or absent Origin, so a web
  page can't drive it. **Found and fixed:** the token ignored a failure of the OS's random source (it would have been 32
  zeros): the daemon now refuses to start without one. With `--ui DIR`, the page's files were guarded against `..` only:
  on Windows `a\..\..\x` or `C:/…` would have reached `dir.join` and read outside the folder (loopback only, and a
  web page can't read the answer, but closed): `\` and `:` are refused too. Test: `daemon`
  `the_page_is_served_from_its_folder_only`. The token is compared plainly, not in constant time: over loopback the
  timing is noise, and it is 128 random bits.

## Nov 23, 2026: hostile plugins on wasmtime 50.0.3 (second pass)

What a WASM plugin can do when its author means harm: the app's sandbox (`crates/plugin/src/wasm.rs`, wasmtime 50.0.3
with the patched advisories above) and browser-only mode's (`browser-plugin-worker.ts`, jco and the preview2 shim). A
plugin loads from the person's own plugins folder, or from a folder's shared `.tlmbench/plugins/` once they trust that
folder (until then it is listed and not even compiled). Native plugins are trusted code by design: they load only from
the person's own folder, and only once they turn each one on by name.

### Held already (and tested before this pass)

A loop ends at the call's CPU budget (an epoch deadline); memory past the cap fails inside the plugin; it can't read
the machine's files or clock; a chunk that doesn't match its schema is refused, naming the plugin (`crates/plugin`
tests `decoder`, `t_browserplugin`). What the host lends is one read-only file through `source`; WASI has no preopens,
no environment or arguments, standing clocks, and stdout and stderr thrown away. Every chunk, payload batch and field
the plugin returns is checked against its schema (column count, bytes per column, times per row) before a byte reaches
the cache, in the app (`decode/src/plugin.rs`, `decode/src/payload.rs`, `live/src/packets.rs`) and the browser.
Names and messages from a plugin reach the page as text (Svelte escapes them; the string-built panels use `esc`).

### Found and fixed

- **The memory cap was per memory.** wasmtime 50 has multi-memory on, and the limiter compared each memory with the
  cap, so a component with N memories could use N times it (as could N tables, each to 10 M elements). The limiter now
  counts every memory and table of the plugin together, from their first size, and allows 64 memories, 1000 tables and
  1000 instances. Test: `tlmbench-plugin` `wasm::tests::a_plugins_memories_and_tables_share_one_cap`.
- **The same in the browser.** `capMemory` gave each memory of each core module the whole cap. The plugin's modules
  now share one budget (`wasmcap.ts`), and a memory kind it can't limit (64-bit, custom page sizes) is refused. Test:
  `t_browserplugin` (a module with two memories: the first grows to the cap and no further, the second can't grow).
- **A chunk could hold more rows than asked for.** Bounded only by the plugin's memory; now refused in both
  (`decoder.rs`, the worker). Tests: `decoder` `a_chunk_of_more_rows_than_asked_for_is_refused`, `t_browserplugin`.
- **An estimator's map could overflow.** Knots near the ends of `i64` overflowed `TimeMap`'s arithmetic (a panic in a
  debug build, a wrong map in release); a quality that was NaN went through `clamp` to the page as `null`. The map now
  works in 128 bits with a float fallback and saturates; a quality that isn't a number is 0, and residuals that aren't
  numbers are dropped. Tests: `tlmbench-core` `timemap` `extreme_knots_answer_without_overflowing`, `tlmbench-store`
  `align::tests::a_plugins_estimate_that_is_not_a_number_is_kept_in_range`.
- **A plugin that links sockets failed to load in the browser** (the app's linker has them, refusing every address;
  the worker had none, so the whole plugin failed). The worker now links the shim's sockets, which refuse every call:
  the same plugin loads in both and reaches no network in either.

New hostile modes in `plugins/test-hostile`, each tested in the app and the browser: `SOCK` (a TCP connection to a
listener on 127.0.0.1: refused, and nothing reaches the listener), `ENVS` (no environment variables, arguments or
folders, though the app's own environment has some), `DEEP` (a recursion without end: the engine's stack limit stops
it, the plugin fails, the next file opens), `ROWS` (above).

### Justified

- **Lifting a result can take more host memory than the plugin's.** wasmtime copies a returned list into host memory
  before the checks above see it, and an empty string or list is 8 bytes in the plugin and 24 in Rust: a plugin at its
  1 GiB cap can make one call cost the host about 3 GiB once. It ends with the call (the checks refuse it, or the app
  runs out of memory, never past the sandbox). Capping what a call may return needs a lifting limit wasmtime doesn't
  have; the plugin is one the person installed or trusted.
- **A plugin that never stops returning rows** fills the cache as a very large file would: each call is bounded (time,
  rows, memory) but a decode is as long as the plugin says, and so is the disk it takes. It is one the person chose.
- **Compiling a hostile module** happens only for plugins the person installed or trusted; wasmparser's limits (function
  size, locals, nesting) bound Cranelift's work per function.
- **The sniffer instance is kept between files.** `sniff` sees the first bytes of each file no built-in format claims,
  and one instance answers them all, so it could remember them. It can't send them anywhere (no network, files or
  clock), and a file is decoded by a fresh instance.
- **Browser-only mode runs jco's generated JavaScript** in the plugin's Worker, which has the page's origin. It is code
  jco writes from the component's validated, kebab-case names; the plugin's own code is WebAssembly and reaches only
  the imports above. A Worker the page ends at the CPU limit takes its memory with it.
- **A trusted folder's new plugins load without asking again.** Trust is by folder, as the design says (Settings lists
  each plugin and where it came from); the sandbox is the same for every WASM plugin.

### The longer fuzz

`just fuzz-smoke 300` (Nov 23, nightly-2026-10-31, this Mac): each decoder target five minutes, from the corpus the
earlier runs left. No crash, leak or timeout:

| Target | Runs | Coverage (edges) | Corpus |
| --- | --- | --- | --- |
| csv | 1 380 176 | 3 940 | 2 004 inputs |
| packets | 905 660 | 1 597 | 1 562 inputs |
| parquet | 231 191 | 3 433 | 483 inputs |
| reference | 316 612 | 7 548 | 1 505 inputs |

The parquet target's log shows the `parquet` crate's own assertion on a negative column offset
(`file/metadata/mod.rs:1043`) 93 times: `decode/src/parquet.rs` runs the crate under `catch_unwind` and turns that
into "the file is damaged and can't be read"; the target lets that through and aborts on any panic that escapes it
(the release profile unwinds, so the app does the same).

### Still to review (R6)

Every gate and measurement on the release build of the final commit, and the soak.
