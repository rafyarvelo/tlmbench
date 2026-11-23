# Security

## Reporting a problem

Please report a vulnerability privately through GitHub: the repository's **Security** tab › **Report a vulnerability**
(a private advisory). Don't open a public issue for it. Say what an attacker can do, and how to reproduce it (a file, a
plugin, an export or a page that shows it helps most). You'll get an answer within a week; a fix ships in a patch
release, and the advisory names you unless you'd rather it didn't.

## Supported versions

The latest release gets security fixes. Before 1.0.0 there are only release candidates.

## What TelemetryBench defends against

What a stranger can hand you is treated as hostile: recordings and descriptor files (the decoders are fuzzed), a
folder's shared `.tlmbench/` (layouts, checks, widgets, icons, plugins), WASM plugins (a sandbox: no files, network,
environment or clock; a time and memory limit per call), and HTML exports, which escape every name and label and pass
user SVG through a sanitiser. Shared code widgets and shared plugins wait until you trust their folder. Native plugins
are ordinary code with your rights: they load only from your own plugins folder, and only once you turn each one on.

The review behind this, with each finding and how it was fixed or why it stands, is
[`docs/release/security-review.md`](docs/release/security-review.md).
