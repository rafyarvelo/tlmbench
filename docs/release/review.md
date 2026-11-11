# Release review (R1)

Nov 11, 2026, on `main` at `d45727c`. Every roadmap step (4.1 to 5.10) and every Accepted or Modified design decision,
checked against the code and against tests run in this review. The rows are in `docs/release/ledger.md`;
`python3 ci/ledger_check.py` checks them. No code was changed.

## 1. The counts, and what they mean

| Verdict | Rows | Meaning |
| --- | --- | --- |
| Done | 194 | Evidence found and it covers the whole requirement |
| Partial | 46 | Built in part; the row says which part is missing |
| Missing | 18 | No code, or no way to reach it |
| Diverged | 7 | Built differently from the design or prototype, with no accepted difference |
| Unverified | 26 | May be true; nothing re-runnable shows it |
| Owner pass | 14 | Only a real window shows it |
| | 305 | |

In plain words: **about two thirds of v1 is done and proven; the app is not ready for release.** What is built is
solid: every suite is green on this Mac, with no flake in this run. The 71 Partial, Missing and Diverged rows are real
work, most of it already known from `docs/follow-ups.md` (exports, browser-only mode, Python, parity with the
prototype). The 26 Unverified rows are mostly measurements nobody has re-run or ever made, and one lost test. The
"built" in the roadmap's status table was fair for the core of each step and too generous for its edges.

What was run (all pass): `cargo fmt --all --check`, clippy, `just lint`; 269 Rust tests (1 skipped, the timing test
below); all 89 app UI tests, one at a time, none rerun; all 29 prototype tests; 9 Python tests; `just ro-test`;
`just gate4 matrix`; `just gate5` (2 h, 20-channel flight: 117.7 MB Full, first chart view 313 ms from a local static
server; folder check of 3 flights in 0.7 s). Two budgets measured on the release build: a sort of 10M rows in 0.39 s
(one key) and 1.05 s (two keys) against 2 s; derived channels over 100M samples in 0.35 to 0.79 s against a 0.94 s
pyramid build, the zero-phase low-pass 1.52 s, the z-score 1.93 s (2.05×, at the edge of "about 2×").
`just app-parity`: 212 differences in 23 scenes (main 56, table 17, import 11).

## 2. The gaps, ranked by how visible they are to a user

1. **The app doesn't yet look and behave like the prototype.** 212 harness differences (S4.1-19), and what the harness
   can't see: a dragged channel has no "Drop here for a new pane" zone between panes and no "Place on the left / right"
   split, and the drop outline looks different (D-ui-17, D-ui-18, the owner's finding); channel rows always show their
   unit and pane badge (D-ui-10); no tooltip on event markers (D-ui-11); the tab menu (S4.4-04); the table's Event
   column and dimmed slower channels (S5.2-13, S4.2-09); Stop at takes one category (S5.2-15).
2. **HTML exports are a subset of the design.** Light has no raw samples around events and violations; a trimmed
   channel shows nothing; several files open as separate tabs; no gauges, XY or spectrum in the viewer, whose core
   answers views, values, stats and rows only (S5.10-01, D-cli-09, D-cli-11). No `--single-file`, so nothing opens from
   `file://` or an email (D-cli-05). Widgets export as plain panes and `--redact` leaks names into checks (D-cli-10).
3. **Things a user would look for and not find:** Save as rule from a table filter (S5.3-14); recipes and a starter
   layout in an import profile, and the profiles tab (S5.1-10, S5.1-12, D-derive-01); the channel prefix rule
   (D-share-01); packet types in one tab (S4.3-08); a folder that notices new files (D-ui-02); layouts on the start
   page (D-ui-01); icons (D-gauge-06); events, checks and sorting on the second file of a tab (S5.6-12);
   `tlmbench check --watch` (D-check-05); report cells that link to the exported view (D-check-06).
4. **Browser-only mode reads CSV only** and freezes on a big file (S5.10-04). **Python** has no wheels, can't derive
   or add events (S5.10-03, D-cli-12).
5. **Decoders:** time going backwards still fails in fixed-width, packet and plugin files (S5.10-06); no causal
   filters live (S5.1-06); a descriptor can't name a plugin for one field (D-ingest-09).
6. **Trust in a shared export:** no run record in `export.json` (D-cli-07); checksums are written but never verified,
   and are BLAKE3 where the design says SHA-256 (D-cli-08).
7. **Widgets under load:** no 10-per-second limit on text, no turns past 4 ms, traces in SVG where the design says
   canvas (S5.4-05, D-gauge-04, S5.4-06); 100 widgets take 2.1 ms headless against a 2 ms budget, unmeasured in the
   release app (S5.4-12).
8. **Nothing to install:** no bundles, signing, release workflow, CLI binaries or Docker image (D-cli-02; step R4).

## 3. Drift in the documents, and what was corrected

- **A lost test.** Step 5.5 (`d412500`) wrote the file-playback test over the transport's `t_playback`. Play's
  transport (play/pause, Space, speed, scrubbing, the A–B loop, each tab's playhead) has had no app test since; the
  tests README still listed it (D-ui-13, D-ui-14). *Corrected:* the README says so. The test itself is R3.3's.
- **Feature matrix.** It gave totals of 108 ✓, 29 ◐, 67 ✗ from Nov 8; its own rows counted 173, 28, 3. *Corrected:*
  eleven cells the ledger disproves (Events was ✗ though built; five ✓ rows whose own text names a missing part became
  ◐; four ◐ rows are done) and the totals: 172 ✓, 30 ◐, 2 ✗. Where it and the ledger differ, the ledger is right.
- **Parity audit.** It said 251 Ported, 11 Different; its Status column counts 250 and 12. *Corrected.* Its 283 rows
  are controls of the UI; the ledger doesn't repeat them, it carries the ones that are gaps.
- **Roadmap status table.** *Corrected (four claims disproved):* 5.2, 5.3 and 5.4 said CI had not run on them (it has:
  green at `60f0eca`, run 42961892776; what is true is that the parity harness has no scene for their dialogs); 4.2
  listed row hover, the cursor line, keyboard, the lock setting and the view switch as left (built and tested); 5.10
  said it wasn't pushed (it is).
- **Tests READMEs.** *Corrected:* the prototype's said 27 tests (29); the app's had `t_plugins` twice.
- **Not corrected** (the design is exported from the living doc): it says the prototype suite has 21 tests; the wire
  format's trace header is 24 bytes at v2 (the code: 32 at v3, D-render-01); time going backwards "is an error"
  (flagged since 5.10); the check set's TOML example (D-check-08). The "for the design doc" notes in
  `docs/follow-ups.md` are still to fold in (R7).
- **Port of the prototype's 29 tests.** 26 have an app test (the ledger names them). `t_playback`: lost, as above.
  `t_phone`: none (the phone layout is prototype-only; `t_narrow` is the nearest). `t_loading`: `t_open` covers a file
  being read, but not a table's cells filling in (D-load-04). Checks not ported because the feature isn't built: Save
  as rule (`t_filters`), Time channel alignment (`t_align`), dragging widget titles (`t_widgets`), the prefix rule
  (`t_files`), a channel dropped on a pane's edge (`t_explorer`).

## 4. In the repo but not in the plan

- CLI commands in no design row: `tlmbench inspect`, `record` and `plugins` (the design's table has seven; `cache` and
  `query` were accepted). They belong; the design should list them.
- The gate harness ships in the app: `app/ui/src/gate.ts` and the `echo`, `gate_manifest` and `gate_report` commands,
  switched on by `TLMBENCH_GATE*`. Keep (the gates are the proof), but decide whether a release build carries it.
- `tools/winid` (Swift, for `just app-shot`), `.devcontainer/`, `docs/next-agent-5.8.md` (a stale hand-off), the
  README's "What works today" and "Where it's going" (a progress report). For R8; nothing was deleted.
- Clean: no TODO, FIXME or XXX in the source; no setting nothing reads; no command the UI never calls; no crate or
  `ci/` script nothing references. The `#[allow]`s are argument counts and one platform cast, plus four `dead_code` and one unused import in
  `crates/web/src/lib.rs` for the decoder files it includes as source (they go when the decoder builds for WASM, R2.2).
  One ignored test, the sort timing test, which passes in release. Five UI tests skip and report PASS when a binary or
  the viewer isn't built (`t_browser`, `t_livecsv`, `t_exportreal`, `t_viewer`, `t_browseronly`); none skipped here,
  but a release gate should fail on a skip.
- Release state: no `LICENSE` (crates say `UNLICENSED`), 0.1.0 everywhere (workspace, Python, `tauri.conf.json`),
  `bundle.active` false, no release workflow, no CHANGELOG, CONTRIBUTING or SECURITY.

## 5. What could not be checked, and who can

- **Needs the release window or the 10 GB recordings (R6, on this Mac):** Gates 1 and 2 (last passed Nov 1 and 2),
  `gate3 core` (Nov 2), `gate3 app` (Nov 9, before 5.6 to 5.10 changed the commands), `gate4 app` and the 10 GB packet
  file (Nov 8). Also `just fuzz-smoke` and `just bench`.
- **Never measured:** table scrolling at 60 fps, 100 widgets in the release app, idle memory, sample-to-pixel latency,
  `convert` and `check` on a 10 GB file with flat memory, the Canvas 2D renderer, an export from a remote host.
- **The owner, in a real window (14 Owner pass rows):** Finder drops, native dialogs, in-page drags and tab drags
  between windows, fonts and hatching in WKWebView, line styles, limit lines, motion, code widgets' frame, a growing
  `.tlm` and a socket source, a read-only disk image, quitting and restoring windows.
- **A platform this Mac isn't:** Windows and Linux beyond CI's build and unit tests (X-win-10).
- **CI:** run 43073397717 on `d45727c` had macOS and Windows green and Linux still running after two and a half hours
  when this was written. No 5.10 commit has a fully green run yet (R0).

## 6. Decisions for the owner

**Owner, Nov 11: begin with these recommendations.** They are written out as decisions in
`docs/release/next-agent-prompt.md`; number 1 is "one store everywhere, unless it means rewriting the store".

| # | Question | Recommendation |
| --- | --- | --- |
| 1 | The store for WASM: build the needed part of `tlmbench-store` for WASM, or write groups inside `crates/web`? Exports of several files and browser-only mode both wait on it. | Decide before R2.1; the next agent looks at what blocks the build (arrow, zstd, mmap) and recommends. One store everywhere is the design's line. |
| 2 | Export checksums: keep BLAKE3 (already a dependency, fast in WASM) and change the design, or SHA-256 as written? | Keep BLAKE3, change the design's words. Either way the viewer must verify each chunk (D-cli-08). |
| 3 | Fold into the design what the code settled: checks run as their own pass, not the pyramid's (S5.3-04); the check set's TOML keys as the examples have them (D-check-08). | Yes to both: the examples are the format the app writes, and the separate pass costs what the design allowed for. |
| 4 | Widget traces: canvas, as the Accepted decision says, or keep SVG (S5.4-06)? | Measure in the release app first (R6). If 100 widgets fit 2 ms with SVG, accept the difference; if not, canvas. |
| 5 | Live views: whole views at 30 Hz pass Gate 3; the design describes sending only changed buckets (D-live-09). | Accept the difference and say so in the design, unless R6's hour-long soak shows a cost. |
| 6 | CSV `#` comment lines are built and tested but the decision is still Proposed (D-ingest-08). | Accept it. |
| 7 | Packet types: a tab each was allowed "until 5.6"; make them slots of one tab now (S4.3-08)? | Yes, as the design says; it is in R2.5. |
| 8 | Phone layout: the prototype tests one (`t_phone`); the design doesn't mention phones. Port it or leave it? | Leave it: the app is a desktop app, and browser-only mode can revisit it after v1. |
| 9 | Should the release gate fail when a UI test skips, and should a release build carry the gate harness? | Fail on a skip; keep the harness (it is inert without its environment variable). |
| 10 | Order: R0 asks for two green CI runs with no rerun before R2. The Linux job is the open one. | Let the next agent check it first and de-flake before starting R2.1 if it failed. |

The ten release decisions in `release-plan.md` (licence, platforms, signing and the rest) stand as written there; none
is needed before R4.
