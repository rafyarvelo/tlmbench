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

## From the Nov 22 apple-design audit (after the UI freeze)

- **A selected file's actions in a narrow Explorer:** Open and Only this sit on the file's row, so in the 300 px sidebar a
  short name in a two-file tab still ends in "…" (it no longer vanishes: fixed in the audit). Room for the whole name would
  mean moving the two buttons (a second line, or hover only): a design change.
- **The Report tab and the top bar:** Lock and the Events toggle stay live on the Report tab, where they have nothing to act
  on (harmless; Charts, Tables and Both are disabled there since the audit). A top bar that shows only what the tab in
  front can use would be calmer.

## Python (owner, Nov 24)

- **`Recording.show()`:** write an HTML export to a temp folder (`export_html`), serve it on `127.0.0.1` with the standard
  library's `http.server` on a background thread, and open the browser; in a notebook return an iframe (`_repr_html_`).
  About a day: the function, notebook support, tests (the server answers, the first view loads, the temp folder goes away),
  and the manual's Python chapter. The risk is the wheel: it must carry the viewer (`app/viewer/viewer.pack`, 2.7 MB), so
  `python.yml` needs `just viewer` before `maturin`. A remote or cloud notebook can't reach the kernel's `127.0.0.1`; the
  single-file export would, with its size limit. Not a GUI over the Python core: the UI's command handlers live in the
  app crate, and a second server would be a second thing to keep in step.

## Limits (owner, Nov 27, from the rc.3 test plan 5.2)

- **Limits in a shared folder.** Limits go by channel name across every file (the design's value board). Files with the
  same channel name but other hardware need other limits: a folder's `.tlmbench/limits.toml` would set limits for the
  files in it and win over yours, as its other shared kinds do (read-only folders, Share writing only what changed). Per
  import profile and per file were weighed: a profile doesn't tell identical loggers apart, and per file shares nothing.
  Today: set them for the files in front, or derive a renamed copy of the channel for a second set.
