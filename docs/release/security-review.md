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

### Still to review (R6)

Decoders and descriptor TOML against hostile files (the fuzz targets, `just fuzz-smoke` then longer), plugins in the
sandbox (the hostile-plugin test, on wasmtime 50.0.3).
