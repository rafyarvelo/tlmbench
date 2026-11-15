# After v1

Work the owner has placed after the v1.0.0 release, and ideas found along the way. Nothing here is v1 scope; the owner
ranks it after release. Agents add to it instead of growing v1 (`docs/release/release-plan.md`, "Order and dependencies").
Each item says who placed it here and when.

## Browser-only mode beyond the design (owner, Nov 15, 2026)

The design promises a tab that opens raw files dropped on it, with descriptors and WASM plugins working there, and its
cache in the browser's private storage; the owner added read-only folders, as the prototype has (sheet F2). Those are v1
(ledger S5.10-04, D-brow-01 to D-brow-10). The release plan's earlier wording also asked for these, which the design
doesn't promise for a tab; they wait until after v1, and a tab leaves them out (hidden, not greyed) until then:

- Several files in one tab, with the time map and the alignment methods (the simple ones need no data; Sync points, Event
  matching, Cross-correlation, Time channel and estimator plugins need `tlmbench_store::align` in WASM). Found Nov 15:
  `browser-only.ts` doesn't wire `group_set`, `group_state` and `group_map` as `viewer.ts` does, and `align_estimate` is
  not in the WASM core.
- Derived channels (the expression crate is already in the WASM core), events (your markers and ranges, kept in the
  browser), rules and checks.
- Layouts saved in the browser.
- A phone layout (the prototype's `t_phone`; release review decision 8).

## Distribution (owner, Nov 15)

- A Homebrew cask first, then winget, Flathub and crates.io as the owner ranks them (release decision 10).

## Ideas and measured limits (from the step notes, Nov 2 to Nov 11)

Not gaps: v1 meets its budgets without them. Each would make something faster or remove a known limit.

- **Pyramid and views:** an exact whole view of a cold 10 GB, 20-channel file takes 1.7 to 2.3 s after the 0.22 s coarse
  view; a finer coarse-to-exact handoff could cut it. Masked views (Apply to charts) read every raw sample in view;
  buckets that touch no excluded run could come from the pyramid. A writer thread for builds was tried and gave nothing
  (one SSD).
- **Live:** UDP reads one datagram per call (45% of a core at 4 × 1 MHz; `recvmmsg` would cut it); a tail polls every 20 ms
  (kqueue or inotify would shorten the window a truncation loses); live checks re-read the whole recording every 2 s (the
  changed tail would do for rules and sample checks).
- **Checks:** `tlmbench check` streams each row on one thread (rows could run in parallel).
- **Derived channels:** replay for rolling windows by time; two passes of a one-number recipe in one read.
- **Plugins:** compile once into the app cache (`Engine::precompile_component`) instead of at each scan; reload a plugin
  when its file changes; WASI's secure random still comes from the OS.
- **Several files:** two tabs aligning the same pair differently overwrite one pair's saved time map (the last wins); an XY
  plot across files needs interpolation.
- **Windows and the daemon:** a tab can't be dragged between browser tabs; a Unix socket beside the WebSocket.
- **Copy as CSV** stops at 100,000 rows (owner, Nov 8); a bigger copy would be an export.
