You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to the v1
release. Step R2.1a (the owner's first test-sheet list) is mostly done: `3c0d3dc..HEAD` on Nov 14 fixed every item of its
section 1 and built seven of the owner's nine decisions. **Your job: the two decisions left (7 and 8, below), then R2.2.**
Nothing is pushed: pushing is the owner's call. One agent works at a time: start from a clean `main`, leave a clean `main`.

## First, in this order

1. Read `CLAUDE.md`. Its hard requirements bind you: the app works from its cache alone on a read-only folder; nothing is
   written next to data except by Share and exports; exports are static files; everything in the design is v1 and you never
   propose cutting scope; the app never runs Python or other programs.
2. Read `docs/release/owner-test-sheet.md` (the owner's words), `docs/release/release-plan.md`, `docs/release/ledger.md`
   and `docs/release/ui-audits.md`. A feature is done only when its ledger row is Done with evidence `ci/ledger_check.py`
   can check.
3. Skim `docs/exports.md`, `docs/follow-ups.md` and the memory files in
   `/Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/` (`owner-decisions-nov14.md`,
   `viewer-architecture.md`, `disk-space.md`, `headless-tests-miss-real-window.md`, `release-plan.md`).

## What the owner decided on Nov 14 (all nine recommendations accepted)

Built (ledger rows D-ui-22 to D-ui-28): ⌘Z / ⇧⌘Z undo and redo (3); Option or ⌘ copies a drag on the Mac, Ctrl elsewhere,
with a "+" (2); Settings › Default line width and style (4); a per-pane Events toggle, E as the default (5); the top bar's
Export menu and the viewer's "Read-only view" header with provenance (6); pane actions on hover, giving way when they don't
fit (9); Reset app… and Settings › Import profiles (1).

## 1. Decision 7: export tiers (F2, F3)

The owner accepted, in full:

- (a) The Export to HTML dialog offers **Folder** (any size; needs a web server or a host) and **One file** (opens from
  anywhere), each with its size. Today One file is CLI-only (`--single-file`). The viewer's file:// note already points to
  One file (`app/ui/viewer.html`).
- (b) One file carries every raw sample when they fit, and Light only when they don't. The 25 MB cap becomes a field in
  the dialog (and a CLI flag) with 25 MB as the default. `crates/export/src/html.rs` has the budget logic and the one-file
  writer.
- (c) The viewer's coarse drawing matches the app's at the same zoom: the min/max band with a midline, not a thinned line.
  The owner said F3's coarse plots were "way too coarse". Compare side by side with screenshots (the app over the same file
  against the viewer over its export), and make the viewer draw the band the app draws (`settings.envelope`).
- (d) "Open folders and CSVs in a browser like the prototype" is browser-only mode, **R2.2**, next. An export is a static
  snapshot; browser-only mode is the app's UI over the core in WASM. Both are v1.

Tests: `t_exporthtml` (the dialog), `t_exportreal` (the real core), `t_viewer`, the export crate's tests and
`cli/tests/export_html.rs`. Ledger: S5.10-01, D-cli-05, and new rows as needed. Owner pass rows from R2.1 still open: F4 to
F8 (fold them in when you touch the same code).

## 2. Decision 8: the native menu bar

The owner accepted a full native menu bar (File, Edit, View, Pane, Window, Help) built from **one command table**, so the
menus and the shortcuts can't drift. Today File has only Close Window. Points found on Nov 14, before you design it:

- The UI handles its own keys (`app/ui/src/app.ts`, the `keydown` listener). If the menu also binds an accelerator, macOS
  sends the key to the menu and the webview never sees it. So either the menu owns a shortcut (and the keydown handler
  drops it), or the menu shows it without binding it. Pick one rule and apply it to the whole table.
- **Don't bind single keys** (E, L, T, Backspace, Space) as menu accelerators: they would stop typing in every text field.
- **⌘Z must stay text undo inside a field.** The app's ⌘Z (`undo()` in `app.ts`) already skips inputs; a menu Undo item
  must do the same (send the native `undo:` to a focused field, else the app's undo).
- Build the menu in Rust from a table the UI sends (or one shared JSON), and have clicks emit an event the UI maps to the
  same action as the shortcut. In daemon mode, events reach clients only if listed in `app/src-tauri/src/daemon.rs`.
- Add ledger rows (one for the menu, one per menu if it helps), a test that every table entry has a menu item and an
  action, and a design-doc note.

## 3. Then R2.2 (browser-only mode), as `release-plan.md` says

Run the apple-design audit at the end of R2.2 (browser-only mode and the viewer, light and dark), into `ui-audits.md`.

## Hand the owner, when you report

- The re-test list (below) if they haven't run it yet, and what you changed.
- **Design-doc notes owed** (list them; don't edit the Claude Doc): BLAKE3 checksums (D-cli-08); checks as their own pass
  (S5.3-04); the check TOML (D-check-08); CSV `#` comments (D-ingest-08); redacted checks leave the export (D-cli-10); group
  routing in `crates/web`; and from Nov 14: ⌘Z is undo (Backspace goes back a view); the copy key (Option or ⌘ on the Mac,
  Ctrl elsewhere); the default line width and style; per-pane Events (E is the default); the top bar's Export menu and the
  viewer's Read-only header; pane actions on hover, giving way by width; Reset app… and Import profiles; Chart and values'
  grip at the pane's bottom (the prototype has it under the plot); "Show only these" as an Explorer header icon (the
  prototype has a pill in the Open strip); the folder watcher with Refresh; the export closes on start with progress under
  the top bar. Add what decisions 7 and 8 change.

## The owner's re-test list (from Nov 14)

Release build from the commit in the sheet's "App: release build from commit" line. Re-test what failed: A2, A3 (Settings ›
Reset app… and Import profiles), A4, A6 (the tab menu says "Folder is read-only"), B1 (⌘Z, now undo), B3 (Settings ›
Default line), B5, C1 (⌘ or Option), E1, E2, F1, F2, "Other" #1 (drag a table column), and the ideas (menu bar: not yet;
folder watcher and Refresh; pane actions on hover). Then the sections not run yet: B6, C2, C3, D1, D2, F4 to F10, G1 to G3,
H1 to H5.

## Rules

- A row moves to Done only with evidence you ran in your session (a test by name, a file, or a command with its date). Run
  `python3 ci/ledger_check.py` before every commit.
- A test for every behaviour change, the tests README's table, and a design-doc note when behaviour differs from the doc.
- Headless tests don't see the real window. Anything only a real window shows is an Owner pass row with what to look at.
- The Mac has 8 GB of memory and a small disk. Never run two cargo builds or two UI suites at once. Prefer workspace-wide
  cargo; a full `cargo nextest run --workspace` debug build needs about 20 GB, so check `df -h /` first and
  `rm -rf target/debug` when short (the owner has OK'd deleting caches and builds). Release-mode tests reuse release
  artifacts. `cargo clean --manifest-path python/Cargo.toml` after a Python build.
- Build order: `just viewer` before the CLI or the app, or they embed a stale viewer. `cargo tauri build` runs
  `npm run build` and embeds `app/ui/dist` while it compiles the app crate: don't rebuild the UI during that step.
- UI tests run one at a time with `node tests/t_x.js` from `app/ui`, after `npx svelte-check --fail-on-warnings && npm run
  build`. **Check that the build succeeded** (svelte-check warnings stop it, and the tests then run the old `dist`).
  `t_loading` and `t_perf_gauges` run alone at the end. `STRICT_SKIP=1 ./run.sh` turns a skipped test into a failure.
- Real-core tests (`t_exportreal`, `t_sharero`, `t_watchreal`, `t_resetreal`) drive `target/release/tlmbench-app
  --daemon`; they skip without it. `t_viewer` needs `just viewer` and the release CLI.
- If you need the owner's settings, work on a copy of
  `~/Library/Application Support/io.github.rafyarvelo.tlmbench/settings.json`; never change the original.
- Shell: write scripts with the Write tool and run them; macOS `sed -i ''`; never `git stash`; add files by name; don't push
  or rewrite history unless the owner asks. End commits with the attribution line your session's instructions give.
- Concise replies to the owner: lead with what changed and what's left, no recaps. Never propose cutting scope.
