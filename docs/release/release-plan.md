# Release plan: from step 5.10 to v1.0.0

Written Nov 11, 2026; brought up to date Nov 15. What stands between `main` and a release people can install, and the
order the work runs in. It follows `docs/roadmap.md` (which got the app to feature-complete as far as its own accounts go;
that file is now history) and keeps its rules: one agent at a time, a step runs from a clean `main` to a clean `main`,
tests and docs with every change, decisions belong to the owner. The hard requirements in `CLAUDE.md` still bind. Nothing
here cuts design scope: where the design promises a thing that isn't built, a step builds it. What the owner places after
v1 goes to `docs/post-v1.md`.

## One plan, one status

- **This file is the plan, and its status table (below) is the only record of where each step stands.**
- **`docs/release/ledger.md` is the only feature status**: a feature is done when its row is Done with evidence that
  `ci/ledger_check.py` checks. Every step moves its rows in the same commits as its code.
- `docs/release/next-agent-prompt.md` is a short hand-off: what the next agent does first. It never holds status of its
  own; it points here.
- `docs/release/test-plan.md` is what the owner tries, once per OS on the release candidate (it replaced the owner test
  sheet, Nov 23); `docs/release/ui-audits.md` the audits'
  findings; `docs/follow-ups.md` the design-doc notes owed; `docs/post-v1.md` what waits for after the release.
- `docs/roadmap.md`, `docs/feature-matrix.md`, `docs/parity-audit.md` and `docs/release/review.md` are history (Nov 11 and
  earlier). Don't update them; the ledger and the R3.0 classification replace them.

**A step is done when** its ledger rows are Done (or Owner pass with what to look at), `python3 ci/ledger_check.py` passes,
the owner has been asked to push and has, **CI is green on the pushed commit with no rerun**, and the status table says so
with the commit and the run. Docs-only commits are not pushed on their own; they go up with the next push of code or
tests (owner, Nov 15), since a run takes about 1.5 hours and a push cancels the one in progress. An agent never pushes without the owner's go-ahead (CLAUDE.md), so "built, not pushed" is a
state of its own in the table, not "done".

## Status (Nov 17, 2026)

| Step | State |
| --- | --- |
| R0 Close out 5.10 | **Done Nov 15:** two clean CI runs in a row with no rerun, 43738929926 (`cb228f2`) and 43746056639 (`d0b58d2`, the status-table push), after three fixes: the check report's export test needed the viewer (`ci-linux` builds it first now), `t_workspaces` pressed ⌘ where Linux uses Ctrl, and the replay tests timed a busy runner's file open |
| R1 Review and the ledger | Done Nov 11 (`ledger.md`, `review.md`, `ci/ledger_check.py`) |
| R2.1 Exports | Built Nov 11 (ledger rows naming R2.1; `docs/exports.md`, `docs/viewer-store.md`); CI with the Nov 15 push |
| R2.1a The owner's first pass | Built Nov 14 and 15: the owner's 11 fixes, all nine decisions (owner-decisions memory, `follow-ups.md`), the sheet's R3 (Reset app), R12 (the viewer Locked, with Unlock) and B4 (`TLMBENCH_SLOW_BUILD`), the first UI audit. **The owner's re-test is open**: R12, B4, S1 to S4 and the older untested items on the sheet |
| R3.0 Classification audit (new, moved up) | **Done Nov 15** (`parity-classes.md`: 11 Port rows, 16 App is better, the rest Noise; apple-design audit in `ui-audits.md`; ledger D-ui-36 to D-ui-41, D-live-11). The owner approved every "App is better" row Nov 15 (ledger A-05 to A-11). The owner decided P3 (trim the ⋯ menu) and P10 (a fixed-width time range) as recommended, Nov 15 |
| R2.2 Browser-only mode | **Built Nov 16, CI green on `6bda02c` (run 43908728143):** descriptors and packets (D-brow-08; `packets_core.rs`, a streaming zstd shim), WASM decoder plugins in the page by jco (D-brow-09, owner's decision Nov 16), the folder's `.tlmbench/` (D-brow-10; `config_core.rs`), Events off the rail (D-ui-56); S5.10-04 Done. End-of-R2.2 UI audit in `ui-audits.md` (Nov 16). |
| R2.3 Decoders | **Built Nov 15 and 16, CI with the Nov 16 night push:** time going backwards flagged in fixed-width, packet and plugin files (S5.10-06); plugins reload when their folder changes; the `payload` world (D-ingest-09: a packet type or a field a descriptor names a plugin for, app and CLI, `plugins/template-payload`); templates from `cargo generate` with a `cargo test` harness (D-ingest-10 Done); filters causal on live data (S5.1-06 Done); live packet streams, a session source per packet type from one connection, payload plugins live (D-ingest-11 Done, as the owner chose). **Left:** D-ingest-09 in browser-only mode (payload plugins in the page) · R2.4 Python: Done Nov 15 · R2.5 Core's Partial rows: **started Nov 16 night:** D-ui-58, D-store-11 (the cache's live folders), D-check-02 (missing values can fail), D-check-05 (`check --watch`), D-ingest-12 (CSV text columns as labelled channels) Done; the other R2.5 rows not started |
| R2.5 Core's Partial rows (Sonnet run, Nov 17, pushed with the Nov 17 run) | **Done:** D-ui-57, D-gauge-03, D-check-01, D-derive-01, D-share-01, S5.1-10, S5.3-14, D-cfg-04, D-cfg-07, S4.3-11, and with the owner's Nov 17 decisions D-gauge-06 (icons, pinned strip), D-ui-12 (events from the file), S5.4-10 (widget inputs and parameters). **Opus list, untouched:** S4.3-08, S5.6-07, S5.6-12, D-derive-03, S5.3-06, D-ingest-09's last part |
| R3.1 Menus and shell | **Built and owner-tested Nov 15, not pushed** (D-ui-37, -39, -40, -41, -44, S4.4-04 Done). The owner confirmed the ⋯ menu and the time range; the tab menu, rename and the viewer's tab are on the sheet's T2 and T4. Audit in `ui-audits.md`, Opus pass owed |
| R3.2 Explorer, start page, dialogs | **Built and mostly owner-tested Nov 15, not pushed, with parts waiting on R2.5.** Done D-ui-36, -38, -42, -45, D-derive-02; the owner confirmed the start page, quiet rows, Open with layout ▸, rates and filters; two files in a tab was clearer and its leftover numbers are fixed (sheet T1). Partial D-ui-01 (connections can't be renamed), D-ui-10 (chips and grips, R3.4). Left for core work: the import dialog's sections and Derived list (S5.1-10, D-share-01), Connect's reconnect (D-live-11), the profile's starter layout, the profiles tab (S5.1-12, R3.3) |
| R3.2/R3.3 rows of the Sonnet run (Nov 17) | **R3.2 Done:** D-ui-01, D-ui-09, S5.5-02. **R3.3 Done:** S5.1-12, S5.2-13, S5.4-05, D-ui-59, D-check-03, D-check-04, S5.6-10. **R2.1 Done:** D-cli-06, D-cli-09. **Partial, built further:** S5.2-16 (per-tab filters kept with the session; the Events panel and channel editor parity scenes and the saved layout are left), S5.3-15 (sub-folders, report kept with the set; still a dialog, not the design's separate Report tab), S4.1-21 and S4.3-12 (the Connect scene now opens on TCP in both; what is left is Reconnect, D-live-11, and the import dialog's extra sections). **Not started:** S4.1-19, S4.2-19 (parity classification of the table and main scenes) |
| R3.3 Panes, charts and tables · R3.4 | **R3.3's UI-only rows, pulled forward (owner, Nov 15), built Nov 15, not pushed:** D-ui-43, -46, -47, -48, -49, -11, -13, -14, S5.2-15. **Also built:** the table's dimming (S4.2-09) and Event column (S5.2-13; a tag click doesn't reveal the event yet). **Left of the UI-only rows:** drag outlines and edge drops (D-ui-17, -18). The core-dependent rows follow R2.5; R3.4 after that **R3.4 (Nov 16 night, not pushed):** D-ui-62 Done (one Lock in the app and exports, as the owner chose), D-ui-64 Done (the flights layout's file name), D-ui-63 Done (Safari chip drag: nothing in the stack moves during dragstart; the owner passed V7 and T4 Nov 16 night) |
| Opus run (Nov 17, after the Sonnet run) | **Done, pushed, CI green on `e484d77` (run 44178285025):** S4.3-08 (packet types as slots of one tab), S5.6-12, S5.6-07, D-derive-03 (events, checks, rules, recipes, spectrum and scale on every file of a tab; tables and XY across files split out as S5.6-13, Missing), S5.1-18, S5.3-06 (incremental live checks), D-ingest-09 (payload plugins in browser-only mode), D-live-11 and S4.1-21 (Reconnect, Save recordings to), D-ui-10 (quiet interface), S5.2-16 (Events filters with a layout; Events and channel editor parity scenes, two faults fixed), S4.1-19 and S4.2-19 (every parity scene classified, four small ports). R3.4's whole-app apple-design audit is in `ui-audits.md` (Nov 17). **Waiting on the owner:** S5.4-06 (canvas traces), S5.3-15 (the design's Report tab against decision 8 of Nov 17), the Events panel's top row, four audit items. **Left:** D-gauge-04 (after R6's measurement), S4.3-12 (the import dialog's sections, P5), S5.6-13, the rest of R3.4 (keyboard coverage, tooltips and empty states, copy review) |
| Owner's test notes run (Nov 22, Opus) | **Done, pushed (CI: see the CI row):** the two blockers (D-ui-65 a chip to its own pane's edge moves; D-ui-66 closing a pane keeps the others in their rows), X3 Your widgets reworked as a list and a form (D-ui-59, the owner's layout choice), X1 pins (shared layouts, Unpin on right-click, the icon box; D-gauge-06), X4 one Undo (D-ui-09; toasts step back too), W3 quiet pane headers (D-ui-10). The owner's Nov 22 decisions: W2 results as a tree of files (S5.6-12), X2 read-only events tagged (D-ui-12), W4 a clock reset starts the next part (D-live-13, new). The Nov 17 decisions: S5.3-15 the Report tab (Done), S5.4-06 canvas traces (Done), the two audit items. Owner sheet section Y. Not seen in a real window: Y1 to Y8 |
| Second Nov 22 run (Opus) | **Done, pushed (CI: see the CI row):** S5.6-13 (tables sort, filter and apply by another file's columns, held; XY pairs two files on the shared timeline, in the app and the viewer); S4.3-12 decided App is better (owner); **R3.4 done** (keyboard against the prototype's handlers, End added; shortcuts per platform; layout rename and delete in ⌘Z; tooltips, empty states and copy reviewed, `ui-audits.md` Nov 22; D-cfg-06 tested). **The UI is frozen** from here: only bug fixes and R4's own items (opening files from the OS, Install command-line tool…) touch `app/ui/src`. Owner sheet section Z. Quiet interface's default waits for the owner's Y5 |
| Apple-design audit after the Nov 22 runs (Opus, Nov 22 third run) | **Done, not pushed yet:** the whole app, dark and light, motion and reduced motion (`ui-audits.md`, Nov 22 "after the Nov 22 runs"): nine bug fixes, each with a test (the results tree's cut names, a selected file's vanishing name, Settings' squeezed rows, `<small>` and the steps off the type scale, words in monospace, dead all-panes buttons and the locale date on the Report tab, three missing spaces, the tab strip's smooth scroll under reduced motion); nothing for the owner; two ideas to `post-v1.md` |
| R4 Packaging and release automation | **Built Nov 22, not run yet:** `release.yml` (version gate, app bundles per OS with the CLI beside them, install-and-smoke from the installed files, CLI archives incl. Linux arm64, the CLI's Docker image, the browser-only site, wheels through `python.yml`, SHA256SUMS, attestations, a draft release), `tauri.release.conf.json` and `entitlements.plist`, `tools/check_versions.py`, `tools/smoke.mjs`, file associations (D-ui-72, the owner's choice Nov 22) and Install command-line tool… (D-ui-71, `~/.local/bin`, Nov 22). **Left:** PyPI's trusted publisher for `release.yml` and the SignPath application (the owner's; decision 3 changed Nov 22: no paid signing), SignPath's step in `release.yml` once accepted, a dry run (`workflow_dispatch` or `v0.1.0-rc.1`) and its fixes, the ad-hoc `.dmg` checked on a clean Mac; the manual PDF joins the bundle with R7; the licence (decision 1) with R7's LICENSE |
| R4 dry runs (Nov 22, third run; owner's OK) | **First** (45100863228, `6b1d60c`, hand-started): version, CLI archives on all five platforms, the Linux AppImage and .deb, the Windows NSIS and MSI, the Docker image, the site and the wheels built; the macOS universal bundle failed (tauri looks for the CLI under each architecture's triple), so smoke was skipped everywhere. Fixed in `14ab8d5`. **Second** (45110269772, `14ab8d5`): **green**: every artefact built, and each installer installed and passed `tools/smoke.mjs` from where it was installed (the macOS `.dmg`, the Windows NSIS setup and the portable zip, the Linux `.deb` and AppImage); Gatekeeper rejects the ad-hoc app as expected (a warning); nothing published. D-cli-02 Done; D-ui-72 Owner pass (Z3). **Left of R4:** Z3 on the `.dmg` (owner), PyPI's trusted publisher and SignPath (owner), SignPath's step once accepted, the manual PDF in the bundle (R7) |
| R6 Performance and hardening | **Nov 24 (Opus): the release candidate's pass, every gate passes on the release build of `63360a1`** (results in `docs/developers/gates.md`, "1.0.0-rc.1"; ledger S4.3-10, S4.4-07, S5.5-10, D-live-01, D-perf-01 Done). Gate 4 failed first on `a8d9652` (the 10 GB packet file's reopen 53.5 s: with under 10 GB free, the pyramid's build evicted the packet index); fixed with tests in `644df60`. **The soak passed, an hour** at 4M samples/s on an external drive (450M records verified, 0.08% frames missed, app memory with no growth). **Left of R6:** the measurements no gate takes (ledger rows naming R6: S4.1-11, S4.2-17, S5.4-12, S5.10-05, D-perf-02, D-perf-03, D-live-10, D-cli-04, D-arch-08, D-test-02, D-test-04, a remote static host for gate 5). **Nov 23:** the longer fuzz (5 min a target, clean) and the hostile-plugin review on wasmtime 50.0.3 (`security-review.md`: five findings fixed with tests, six justified). **Left:** every gate and measurement on the release build of the final commit, the soak. **Started Nov 22 (third run):** the security review's first pass (`security-review.md`): dependency audit (wasmtime, libz-sys, url, ruzstd patched; the rest justified), the SVG sanitisers in the page and the export, a file's text labels escaped in Measure, the daemon's token and paths; fuzz smoke clean (4 decoder targets, 30 s each; a 5-minute run was stopped for the owner's release build). **Left:** the longer fuzz, the hostile-plugin review on wasmtime 50.0.3, every gate and measurement on the final commit, the soak |
| R7 Documentation | **Nov 23:** LICENSE (Apache-2.0, every crate), `docs/install.md`, SECURITY.md, CONTRIBUTING.md, CHANGELOG.md, the README for a release, the User Manual's 19 chapters and `just manual` (one PDF, bundled; Help › User manual and Online manual). **Nov 24 (Sonnet run, committed; CI: see the CI row):** the manual's 34 screenshots (`node app/ui/tests/shots.js OUT --manual`, `shots-manual.js`, over make.py-shaped data, the real viewer, browser-only site and CLI over make.py's own files; kept in `docs/images/manual/` with `README.md`, the list of which chapter uses which; the native menu bar and two real linked windows go to test-plan 13.3); the developers guide (`docs/developers/`, ten pages; absorbs `docs/development.md`, `plugins/README.md` is a pointer now); reference pages generated from the source (`tools/reference.py`: CLI help, expression functions, settings keys, menu shortcuts, checked by `just lint`); the web copy (`just pages`, `tools/pages.py`, `.github/workflows/pages.yml`: landing page, manual as HTML, browser-only site, a sample export; runs by hand or on a published release; **Pages is not turned on: the owner's**). What the passes found in the app: `docs-findings.md`. **Design doc final sync done Nov 24 (owner's OK):** a "Final sync" section in Claude Docs folds `follow-ups.md` in, four Nov 7 decisions are Accepted there, `docs/design.md` re-exported; `follow-ups.md` is a stub until the tag. **Left:** About's licence and notices (THIRD_PARTY_LICENSES is generated and checked) |
| R5 | **Nov 23:** the test plan is written (`test-plan.md`, with `tools/testdata/make.py` for the same data on every OS: a binary log and descriptor, a text column, a TCP sender, the big file); the owner test sheet and the macOS checklist are folded into it and deleted. The owner runs it on the RC, once per OS. No Windows or Linux machine yet |
| R8 | **Nov 24 (pruning, committed):** `cargo machete` (the plugin crate's `serde_json` and `thiserror` removed; the WASM crates' false positives named in their `Cargo.toml`), `knip` (`app/ui/knip.json`; five dead UI exports removed), `just unused`. Scripts and recipes: nothing is uncalled; `app-view-menu.png` (nothing used it) was deleted with the owner's OK. **Left:** the delete list at the tag (follow-ups after the design sync, ui-audits, release-plan, next-agent-prompt, security-review, the ledger as the guide's appendix) |
| R6c Coverage | **Goal raised Nov 24 (owner): as close to 100% as possible, and in CI (sub-steps R6c.1–R6c.5 below; hand off between agents as needed).** R6c.1: Rust measured on `ff087fc` (76.5% of lines; the core crates 86–95%; the five gaps and the steps in "R6c Coverage" below). Tests for the five gaps **done** (`6d89931` ring wrap, checked to fail with the stitching off; `07c6d88` alignment; `dadd269` two files in one tab; `623610c` tail sniffing and quoted fields; `b6580c6` a plugin field through the store): no bug found. R6c.2 done: the UI at 91.7% of lines, 75.9% of branches (13 self-launching tests not counted yet). **R6c.3 started Nov 25 with decode** (90.9% → 93.7% on `f9ffec7`; one bug found and fixed, `868883c`: a profile's time column without a unit ignored its header's unit); second pass Nov 25 (`00a1f9d`): decode 96.3%, Rust 79.0% of lines; no bug found in it; what decode has left is listed in `docs/developers/coverage.md` (mainly `parquet_pages.rs` and error branches); next live, then store and the rest. Third pass Nov 25 (Sonnet, `unopenable.rs` commit): the last run's mutation checks done (every break caught; the Parquet, plugin-fault and reference-header tests now collect every wrong case, `456cfc4`); `parquet_pages.rs` 73.1% → 99.2% with native tests (every compression, dictionary on/off, v1/v2 pages, headers by hand, files cut and bit-flipped; `crates/web-parquet`'s own tests are not counted, owner's decision); decode 97.1%, Rust 79.5% of lines; no bug found; what's left in decode is OS-error `?` branches and a list of small behaviours in `coverage.md`; live (93.8%) and store (89.4%) not started. Fourth pass Nov 26 (Sonnet; measured on `93fa262`): **live 93.8% → 99.3%** (`55750c7`; what is left is OS errors, a thread that panics, a pty and the environment variable, listed per file in `coverage.md`) and **store 89.4% → 93.9%** (`bfd16fe`, `61fcee3`, `4b066b0`: events, missing-when, XY, widget runs, in-memory, joined, triggers, recipe library, shared-config reader, damaged and followed cache entries, groups; `adjust`, `joined`, `events`, `widgets`, `xy`, `config_core`, `error` done); Rust 81.5% of lines (429 tests); no bug found; every mutation check caught after one stairs test was added for a trigger level (`4b066b0`); store still has `config.rs` (87.8%), `checks.rs`, `align.rs`, `derived.rs`, `pyr.rs`, `lib.rs`, `live.rs` and `recipes.rs` partly open: the list is in `coverage.md`; Fifth pass Nov 26 (Sonnet; measured on `6f497e4`): **store 93.9% -> 98.8%** and **export 88.5% -> 97.2%** (`d590e7c` to `6f497e4`: the shared folder, alignment, checks, derived channels, live derived, groups, pyramids, every export column type and time format, HTML export, the SVG sanitiser); Rust 83.0% of lines (497 tests); export's mutation checks all caught (store's not redone: a store change relinks about 25 binaries of 700 MB and the disk had 9 GB); **one real bug found** (details in `coverage.md`): a recipe naming a channel of another file was computed on the longer operand's grid, so a faster other file failed it ("computed 492 samples for 50") and equal lengths with different starts wrote it one sample out (fixed Nov 26 with its test: the recording's own grid leads); what is left in store and export is OS-error `?` branches and guards, listed per file in `coverage.md`; next: core (95.1%), expr, cli, plugin, app |
| RC | **For rc.4 (owner, Nov 27):** a recipe saved for an import profile or for any file with its channels is computed for every open tab it applies to, not only the one in front (rc.3 test plan 5.3; note in `follow-ups.md`); the fix for 6.2's stuck "Reading 0%" (`67db7ea`: a small file's whole build progress came before the `open` reply and was dropped); and three more ways a stale percentage could stay, found with it: progress before a new group's `group_set` reply is dropped (`groupApply`), a tab's first file's own progress is ignored once the tab is a group, and `Opened::build` / the wait loop in `pyr.rs` return without a final progress when another process built the file. **Nov 27: `v1.0.0-rc.3` tagged on `9ddc1e5` after CI went green there on the first try (45940157598) and every gate passed on its release build, the soak too (`gates.md`, "1.0.0-rc.3"; `gate` passed on the settled file, its two runs straight after writing the files failed by one long frame, as rc.1's did).** Fixes since rc.2: the browser builds (Gauges, XY and Spectrum in browser-only mode; table sort, filter, ▲ ▼ and Apply to charts in browser-only mode and exports), dates in readouts over days, the version in Settings. GitHub Pages deployed from `9ddc1e5` by hand (owner's OK, Nov 27). **Nov 26: `v1.0.0-rc.2` tagged on `657160f` (owner's OK, at risk: before the window gates and the soak, which wait for an unlocked screen; a bug they find starts rc.3).** CI green there (45899479753, Linux on a rerun: the `t_browserfolder` flake); the headless gates pass (`gates.md`, "1.0.0-rc.2"). Fixes since rc.1: the owner's macOS notes (1.9, 3.4–3.7, 4.5), a cross-file recipe bug the coverage pass found, a CSV profile time unit. **Nov 24: `v1.0.0-rc.1` tagged on `ff087fc` (owner's OK) after CI went green there (45441775732; macOS ran on its third try: twice no hosted runner, and `dc98540` once failed to link, not reproduced). Release run 45478226369 green: every installer built and passed install-and-smoke on its OS; the draft pre-release has the .dmg, NSIS setup, MSI, portable zip, AppImage, .deb, five CLI archives, the site, wheels and sdist, SHA256SUMS (checked); the app bundle and portable zip carry the manual, LICENSE-MIT, LICENSE-APACHE and THIRD_PARTY_LICENSES.md, the CLI archives the licences. Two docker build records also landed in the draft (left out from rc.2, `b93c0e2`).** Next: the owner's test pass. Before:  Version 1.0.0-rc.1 everywhere (`00e2848`, CHANGELOG entry for the draft's notes); the MSI gets a numeric version for a candidate (`8489939`: tauri-bundler refuses `rc.1` for WiX, so the tag would have failed on Windows) |
| R9 | Not started. Every release decision is made (Nov 15) |
| CI | **Nov 25, latest: green on `00a1f9d` (run 45670948681), all four jobs (CI image, Linux, macOS, Windows), no rerun; Python wheels green (45670948792).** Before it: green on `0648fe1` (run 45616413906), all four jobs. Before it, `f9ffec7` (45603590509; decode's coverage tests, the CSV time-unit fix) failed only on Linux: `t_browserfolder`'s double-click on a re-rendering Explorer row was lost (the test opens again now; a real-window check is test-plan 2.10). **Nov 24: green on `17064fe` (run 45529854699), all four jobs; wheels green (45529854765).** Before it, `7b1d373` (45512300632) failed only on Linux: the new ring-wrap test, paced by time, let a loaded runner fill its small ring (fixed: it waits on the recorder). **Earlier Nov 24: green on `c925acb` (run 45395959120), all four jobs; Python wheels green (45395959435).** The licence commits (`a022bac`, `9488452`) went up after it: see the next run. **Nov 24 run:** **green on `7b11c39` (run 45291637370), all four jobs.** `2d89541` (45271159623) failed only on Windows lint: clap prints `tlmbench.exe` in the CLI page's usage lines (fixed in `tools/reference.py`). Pages is on (the owner's OK Nov 24) and the Pages workflow ran by hand (45303351111): `/`, `/manual/`, `/app/` and `/sample/` at https://rafyarvelo.github.io/tlmbench/ load. **Nov 23 run:** **green on `a1789fd` (run 45242330461), all four jobs, no rerun.** Before it, `9f26af0` (45228727085) failed only on Windows lint: `tools/third_party.py` read cargo's output as cp1252 (fixed: UTF-8 everywhere). Earlier the same day **green on `c4593dd` (45211777663)**; Python wheels green (45211777130). **Third Nov 22 run (Nov 23):** **green on `14ab8d5` (run 45110261576), all four jobs, no rerun**; Python wheels green on `2c2fba9` (45104707616) and in the dry run on `14ab8d5`. On the way, Windows failed twice on `6b1d60c` and `2c2fba9` before any test ran: restoring its 9 GB target cache (four versions of each test binary) filled the runner's disk. The owner had that cache deleted (Nov 23); if it grows back, capping it is a CI change for the owner to approve. **Second Nov 22 run:** `d93b278` (run 45076016877) failed only on Windows, at lint: `install::on_path` is dead code there (it serves the Unix install); Linux, macOS and Python wheels (45076016802) green. Fixed in `d9c48d9` (Unix-only): CI then **green on `d9c48d9` (run 45083708381), all four jobs, no rerun**. **First Nov 22 run:** `4b3d318` (run 45051069274) failed only on Linux, `t_browserpayload` (the page's WASM core slower to start than an unchecked 30 s wait on a loaded runner); macOS, Windows and Python wheels (45051069260) green. Fixed in `424ed0e` (a 90 s boot wait that is a check of its own, also in `t_browserplugin`); CI then **green on `0e60834` (run 45057964460), all four jobs, no rerun**. Before: green on `e484d77` (run 44178285025) with the Nov 17 Opus run. |

## Recommended model per step

Guidance from the owner's request (Nov 15), not a rule: the owner may run a cheaper model for cost. Opus 5.5 for work that
needs judgement across many files or hard debugging (races, WASM, signing, design calls); Sonnet 5.5 for well-specified
building from a list; Haiku 5.5 for mechanical work with a clear checklist. When a cheaper model than the recommendation
runs a step, keep the step smaller and have it stop and report at the first design question rather than decide.

| Step | Recommended | Why; what a cheaper model can take |
| --- | --- | --- |
| R0 CI de-flake | Opus | Finding races in `t_live`, `t_checks`, `t_windows`… on Linux CI, not adding retries. Sonnet can push and watch runs |
| R3.0 Classification audit | Opus | Telling "the app is better" from "the app is behind" is a design judgement, and the apple-design audit with it. Sonnet can run the harness and take the screenshots |
| R2.1a fixes from the owner's re-test | Sonnet | Each item is described by the owner; Opus for a bug only a real window shows that resists a first fix |
| R2.2 descriptors and packets, WASM plugins | Opus | Splitting `packets.rs` for WASM, a streaming zstd shim, running WIT components in the browser's engine |
| R2.2 folders, leaving out post-v1 controls | Sonnet | `showDirectoryPicker` and hiding controls by a known rule; the UI audit after it: Opus |
| R2.3 Decoders | Sonnet | The CSV and Parquet path already shows the way (`backwards.rs`) |
| R2.4 Python | Sonnet | Wheels, a CI matrix and trusted publishing are well documented; the Arrow C interface: Opus if it fights |
| R2.5 Core's Partial rows | Opus | Mixed rows, several touch the store and the design; Sonnet for rows that are plainly UI |
| R3.1 to R3.3 Parity ports | Sonnet | Ports from R3.0's classified list, with the prototype as the reference; the per-area audit after each: Opus |
| R3.4 What the audit can't see, the pre-freeze audit | Opus | Copy, keyboard and motion review across the app is judgement |
| R4 Packaging and release automation | Opus | Signing, notarisation and a release workflow that must work first time on three OSes |
| R5 Real-window passes and fixes | Opus | Bugs headless tests can't see; writing the owner's checklists: Sonnet |
| R6 Performance and security review | Opus | Measuring budgets honestly, a security review; re-running the gates: Sonnet |
| R6c Coverage | Sonnet | Tests for listed gaps, crate by crate (R6c.3) and file by file (R6c.4), against a written list; Opus when a test finds a real bug, for the UI's harder paths, and for R6c.5 (the CI job and its ratchet) |
| R7 Documentation | Sonnet | The manual and guides from the design, the ledger and the app; generated reference pages and screenshot runs: Haiku |
| R8 Cleanup | Haiku | Deleting the agreed list and pruning; dead code and dependency checks: Sonnet |
| R9 Release candidate and release | Sonnet | Running the checklist; Opus for an RC bug that blocks the tag |
| Status table, ledger counts, test sheet upkeep | Haiku | Bookkeeping, as part of any step |

## Where things stand (checked Nov 15)

- Ledger: Done 235 · Partial 48 · Missing 30 · Diverged 4 · Unverified 26 · Owner pass 17 (360 rows).
- Tests at the last full local run (Nov 15): 104 of 104 app UI tests, `cargo nextest run --workspace` 283 passed (1
  skipped), clippy clean. On CI: see the status table.
- **Nothing for release exists yet:** `bundle.active` is `false` in `app/src-tauri/tauri.conf.json`; no release workflow;
  no installer, signing or notarisation; no `LICENSE` (the crates say `UNLICENSED`); version 0.1.0 everywhere; no
  CHANGELOG, CONTRIBUTING or SECURITY file; no manual or developer guide.

### Drift found and corrected Nov 15

- Steps were called done without CI: the rule "every step ends with CI green" (R0, the steps' preamble) stopped being
  applied after Nov 11, and hand-offs began to say pushing was the owner's call without asking for it. The rule is restated
  above.
- R2.1a, R12 and B4 ran outside any plan table, and the roadmap's table stopped at R2.1. The status table above replaces it.
- R2.2 had grown past the design: the plan's "derived channels, events, checks and several files as the design promises
  for it" isn't in the design, which promises raw files, descriptors and WASM plugins in a tab. The owner set the scope
  (below); the rest is in `docs/post-v1.md`.
- The start-of-R3 parity audit risked porting the prototype over places where the app is now better (dialogs, the menu
  bar, the viewer's lock, export tiers). R3 now starts with a classification the owner approves (R3.0).
- Eight documents carried status and disagreed. They are reduced as "One plan, one status" says; `next-agent-5.8.md` and
  `review-prompt.md` are deleted (git keeps them), the R2.1 plan became `docs/viewer-store.md`, and `follow-ups.md` keeps only
  the design-doc notes owed (its open items were in the ledger since R1).

## Release decisions

Each step below says which of these it needs. **All ten are decided, by the owner on Nov 15, as recommended**; the CLI
ships beside the app (R4). The web copy is GitHub Pages (7).

| # | Decision | Recommendation (and state) |
| --- | --- | --- |
| 1 | Licence for the code, and for the manual and screenshots | Apache-2.0 for code (patent grant, plugins and bindings are meant to be extended); CC BY 4.0 for the manual. The owner's call; it changes every `Cargo.toml`, `pyproject.toml` and the README **Decided Nov 15; changed Nov 24 (owner): MIT OR Apache-2.0** (`LICENSE-MIT`, `LICENSE-APACHE`), the manual still CC BY 4.0, the plugin templates MIT-0 (owner, Nov 24). |
| 2 | Platforms and architectures to ship | macOS arm64 and x86_64 (universal `.dmg`), Windows x64 (installer + portable zip), Linux x64 (AppImage and `.deb`; `.rpm` if wanted), Linux arm64 for the CLI only. Windows arm64 only if a runner is available **Decided Nov 15.** |
| 3 | Signing: an Apple Developer ID and notarisation; a Windows code-signing certificate | **Changed Nov 22 (owner): nothing paid for v1.** Windows: free signing from SignPath Foundation for open-source projects (the owner applies; Apache-2.0 qualifies), unsigned until it is accepted. macOS: ad-hoc signed, not notarised (no Apple Developer Program): the README and manual say how to open it the first time (System Settings › Privacy & Security › Open Anyway). Linux needs none. Checksums and build-provenance attestations for every file, as before. Replaces the Nov 15 choice (sign both, paid) |
| 4 | Where installers are published | GitHub Releases on the repo, built by a tag-triggered workflow; the same workflow publishes the Python wheels and attaches the CLI archives **Decided Nov 15.** |
| 5 | PyPI: the project name (`tlmbench` is what `python/` uses), a trusted-publisher setup | Register the name now; publish with PyPI trusted publishing (no stored token) **Decided Nov 15.** |
| 6 | How Help opens the manual in the app | Open the bundled PDF with the OS's default viewer through Tauri's opener (the app doesn't run a program of its own: it asks the OS to open a document), plus a "Online manual" link. The alternative is a manual window inside the app, which needs the manual as HTML too **Decided Nov 15.** |
| 7 | Where the web copy of the manual lives | GitHub Pages from the repo (`docs/site`), next to the browser-only app and a sample HTML export **Decided Nov 15.** |
| 8 | What happens to the plan documents (the list in R8) | Delete them from `main` once their content is absorbed (git history keeps them); keep the prototype under `prototype/` as the design reference and its tests as they are **Decided Nov 15.** |
| 9 | Version and tag | `1.0.0`, tag `v1.0.0`; a `v1.0.0-rc.N` tag series for the dry runs **Decided Nov 15.** |
| 10 | Homebrew tap, winget, Flathub, crates.io | Not for v1.0.0; a Homebrew cask is the first to add after **Decided Nov 15.** |

## The steps

Sizes as in the roadmap: S a day or two, M a few days, L a week or more. One agent per step; R2 and R3 are split into
sub-steps that run one after another. Every step ends with `ci/ledger_check.py` passing (after R1), CI green, and
`docs/roadmap.md`'s status table updated.

### R0 Close out 5.10 (S, open)

Get CI green on `main` (Windows' extension link, Linux's `t_live` flake), then update the roadmap's 5.10 row.
Because releases will be built by a workflow, a test that needs "rerun failed jobs" is a release blocker:
**de-flake `t_checks`, `t_live`, `t_folder`, `t_trigger` and `t_windows` on Linux** (find the race, not a retry; a
retry wrapper is not a fix). Done when two consecutive full CI runs pass with no rerun.

### R1 Review and the ledger (M, done Nov 11)

Checked every roadmap step and every Accepted or Modified design decision against evidence; produced
`docs/release/ledger.md`, `docs/release/review.md` and `ci/ledger_check.py` (the prompt it ran from is in git history). **This is the strict confirmation:** from here
on a feature is done only when its ledger row is Done with evidence a script can check. Needs decisions: none.

### R2 Close the data-side gaps (XL, five sub-steps, in this order)

Each lands in the core and CLI first, tested headlessly, then the UI (`crates/web` and the viewer page for exports).
Sources: `docs/follow-ups.md` "Left from step 5.10" and the ledger's Missing and Partial rows with a core side.

- **R2.1 Exports (L).** Light mode's raw windows around every event and check violation (30 s or 3× the event's
  duration, capped at 10 min); a trimmed export in the viewer ("finer detail not in this export"; the pyramid must be
  able to start at any level); a sparse raw store in `crates/web` with a plan for which chunks a view reads. Several
  files in one export, with alignment and `accel_x·2` (decide: make the needed part of `tlmbench-store` build for WASM, or
  a group in `crates/web`; the owner decides with a recommendation). `--single-file` (one HTML file, 25 MB, Light, coarse
  levels) and say plainly in the docs why a folder export needs a server (a `file://` page can't fetch WASM or data).
  Redaction also removes a redacted channel from check expressions and event channel lists. User SVG widgets sanitised
  and code widgets as value cards. HTML check report cells linking to an exported view at the first violation
  (`tlmbench check --report html` with `export html --checks`). Time-range views in the viewer. The reduced viewer bundle:
  dialogs and menus cut out, not hidden. Writer: `crates/export/src/html.rs`; viewer core: `crates/web/src/{data,ops}.rs`;
  page: `app/ui/src/viewer.ts`. Done when `ci/gate5.py` still passes and every export row in the ledger is Done.
- **R2.1a The owner's first real-window pass (M, built Nov 14 and 15).** The owner's findings in
  `docs/release/owner-test-sheet.md`: the fixes with no open decision, nine decisions (all accepted as recommended), the
  release app rebuilt, the first UI audit, then the sheet's second round (R3 Reset app, R12 the viewer Locked with Unlock,
  B4 a build slow enough to watch). Open: the owner's re-test of what the sheet still lists, and its question on keeping an
  unlocked arrangement across reloads, answered no (Nov 15): a reload is the exported view.
- **R2.2 Browser-only mode (M).** Scope (owner, Nov 15): what the design promises for a tab ("the core compiled to WASM,
  opening raw files dropped into a browser tab. Descriptors and WASM plugins would work there, native plugins would not",
  its cache in the browser's private storage) plus read-only folders as the prototype has them (sheet F2). Built: CSV in a
  Worker, a reload bringing files back, the static-host recipe, `.tlm`, Parquet (D-brow-01 to -07). Left, each tested and
  its ledger row moved before the next:
  1. **Descriptors and packets** (D-brow-08): split the pure part of `packets.rs` (framing, checksums, payload codecs) the
     way `parquet_core.rs` was split; the zstd shim gains a streaming decoder; descriptors kept in the browser and found by
     file name pattern; the descriptor form on a binary file nothing claims.
  2. **WASM decoder plugins** (D-brow-09) in the browser's own engine, with the app's limits; native plugins are named as
     needing the app wherever they would be offered.
  3. **Folders** (D-brow-10): `showDirectoryPicker` where the browser has it, a read-only Explorer listing and the folder's
     `.tlmbench/` read for shared configuration; elsewhere it says so once and offers Open recording…
  4. **Leave out what is after v1** (D-brow-05): several files and alignment, derived channels, events editing, rules and
     checks, layouts. Hidden, not greyed: a tab shows only what it does.
  Then the end-of-R2.2 UI audit (browser-only light and dark, each new screen). Not in R2.2: anything in `docs/post-v1.md`.
- **R2.3 Decoders (S).** Time going backwards flagged and kept in file order in fixed-width, packet and plugin decoders,
  as CSV and Parquet already do (`crates/decode/src/backwards.rs`); the filters' causal form on live data if
  `docs/follow-ups.md` still lists it; any decoder row the ledger found Partial.
- **R2.4 Python (M).** Wheels for each platform (`maturin build`, abi3, manylinux 2_28 on Linux, universal2 or two wheels
  on macOS, Windows x64), a PyPI trusted-publishing job, an sdist; derive channels, add events, read `.tlmbench/` objects
  as objects; Arrow through the C interface rather than a pyarrow copy; docs for the API generated from the module's
  docstrings. Done when `pip install` of the built wheel into a clean venv passes `python/tests` on all three OSes in CI.
- **R2.5 The rest of the core's Partial rows (M)**, whatever the ledger lists that is not UI: packet types in one tab as
  slots, scale and offset from settings and not only descriptors, event-aware stops, "Save as rule", the CSV import
  dialog's Panes and prefix-rule sections, and so on. The ledger is the list; nothing is dropped.

### R3 App parity with the prototype (XL, a classification first, then sub-steps by area)

The target is not zero harness differences (many are scene state), and it is not the prototype's look wherever the two
differ. It is **every ledger row Done and every visible difference either fixed or approved by the owner as the app's
way**. The app has moved past the prototype in places (the owner's decisions of Nov 14 and 15, the shared dialog frame,
the native menu bar, the viewer's lock, export tiers, hover pane actions), and parity work must not undo them.

- **R3.0 Classification (S, runs first, before the rest of R2).** Re-run `just app-parity`, read the parity audit's rows and
  look at both side by side; run the apple-design audit on the whole app in the same pass (the "start of R3" audit). Put
  every difference in one of three classes, in `docs/release/parity-classes.md`:
  - **Port**: the prototype's is better, or the app lacks it. Becomes a ledger row (or an existing row's gap) for R3.1 to
    R3.4.
  - **App is better**: the app's is more mature (a decision the owner made, a dialog or flow that does more, a fix of
    something the prototype got wrong). Listed with a one-line reason and a screenshot of each for the owner. The owner's
    decisions of Nov 14 and 15 and the roadmap's accepted list are already approved (ledger A-01 to A-04); the rest wait
    for the owner, and **nothing in this class is ported back**. What the owner approves becomes an `A-NN` row.
  - **Noise**: harness or scene-state differences (fake data, item counts, how the harness reads a page). Listed once, so
    nobody chases them again.
  No code changes in R3.0. Its output is the ordered lists for R3.1 to R3.4 and the owner's list to approve.
- **R3.1 Menus and shell (M), R3.2 Explorer, start page, dialogs (M), R3.3 Panes, charts and tables (M).** The Port class
  of each area, in the order the owner sees things. The lists written here before R3.0 (the tab menu's Save layout and
  Revert, the new-tab menu, pane geometry, the channel tick; badges on hover, Explorer multi-select of categories, the
  start page's sessions and saved sources, Connect's fields; edge drops, event marker tooltips, the table's status line and
  Event column, slower channels dimmed, Stop at several categories, the profiles tab, folder runs, Play's lost test) are
  candidates: R3.0 decides each one's class.
- **The owner's Nov 15 decisions, by sub-step** (each a ledger row): R3.1: rename a layout (D-ui-44), the ⋯ menu trim
  (D-ui-37), the menu focus ring (D-ui-39), the fixed-width time range (D-ui-40), the viewer's clipped tab (D-ui-41).
  R3.2: the start page (D-ui-36), Open with layout ▸ (D-ui-42), every file marked in the Explorer with its slot and
  alignment (D-ui-45), duplicate a recipe from the channel and Save as new (D-derive-02), readable rates (D-ui-38).
  R3.3: pane names (D-ui-43), Open together draws every file (D-ui-46), Duplicate for ▸ file N and From file ▸ (D-ui-47,
  D-ui-48), limits shared by every file's copy of a channel (D-ui-49). R2.5: recipes reach every file in a tab (D-derive-03)
  and every file's events show, labelled (S5.6-12).
- **R3.4 What the audit can't see (S).** Quiet interface; keyboard coverage compared with the prototype's list; every
  tooltip and empty state; copy review (plain, short, sentence case); the design's motion rules.
- **After each sub-step:** rerun `just app-parity`, update `parity-classes.md`, screenshot with `node app/ui/tests/shots.js`,
  and run the apple-design audit on that sub-step's area.

The UI freezes when both R2.2 and R3.4 are done (R3.4 is last in the order): after it only bug fixes touch `app/ui/src`. A full apple-design audit runs before the freeze.

### UI audits with the apple-design skill

The `anthropic-skills:apple-design` skill reviews interface design and motion: feedback, spatial consistency, restraint,
typography, materials and depth, drag and sheet interactions, springs, and reduced motion. Run it on screenshots and the
relevant Svelte and CSS at these points, where the UI is about to settle:

| When | Scope | Why then |
| --- | --- | --- |
| End of R2.1a (the owner's list) | The screens that list touched: dialogs, the export dialog and its feedback, pane resize, XY, the Explorer header, the viewer | The owner called the export UI "disappointing"; set a baseline before more UI lands |
| End of R2.2 | Browser-only mode and the viewer, light and dark | A new surface people will meet first |
| R3.0, moved before the rest of R2 (Nov 15) | The whole app, against the prototype, in the classification pass | It turns "looks wrong" into ranked items for R3.1 to R3.4, and names where the app is better so parity doesn't undo it |
| End of each R3 sub-step | That sub-step's area only | Catch regressions while the code is fresh |
| R3.4, before the UI freeze | The whole app, with motion and reduced motion | The last chance for anything that isn't a bug fix |
| After the Nov 22 runs, before R4's dry run (owner, Nov 22) | The whole app, dark and light, with motion and reduced motion | The Nov 17 audit predates the Nov 22 UI work; the last whole-app pass after the freeze (findings are bug fixes or post-v1) |
| R5, on each OS's installed build | Fonts, high-DPI, the native window (WKWebView, WebView2, WebKitGTK) | Headless screenshots don't show these |

Rules for the audits:

- The prototype and CLAUDE.md's UI conventions (tokens, calm UI, tooltips, copy) win over the skill's advice. Where they
  conflict, or where a finding would change the design, it is a decision for the owner, with a recommendation.
- Write the findings with the screenshot each comes from, ranked, in `docs/release/ui-audits.md` (one dated section per
  audit; kept until R8). Fixes cite the finding.
- After the freeze (end of R3), a finding is either a bug fix or goes to `docs/post-v1.md`. It never adds scope.
- Each audit is a line in the status table when it runs; a step with an audit point is not done until its audit is in
  `ui-audits.md`.
- If the skill isn't available in a session, say so and do the pass by hand against the same headings. Don't skip it.

### R4 Packaging and release automation (L)

Needs decisions 1 to 5 and 9. Built before the docs so that every platform's installer exists for R5's real-window passes.

- **Bundles:** turn `bundle.active` on; targets per decision 2 (macOS `.app` and `.dmg`, Windows NSIS and/or MSI, Linux
  AppImage and `.deb`); icons (present); the bundle ships the CLI next to the app, with an "Install command-line tool…" item in
  the app (owner, Nov 15), the plugin templates, the example `.tlmbench/` and the manual PDF as a
  resource; file associations for `.tlm`, `.csv` and the descriptor extensions; the viewer pack is already embedded
  (`just viewer` first).
- **Signing:** macOS Developer ID and `notarytool`, hardened runtime and the entitlements the app needs (read-only
  files, the plugin runtime's JIT: check wasmtime's requirements), Windows signing, checksums for every artefact, and
  `SHA256SUMS` signed or attested (GitHub artifact attestation).
- **Workflow (`.github/workflows/release.yml`):** on a `v*` tag, a matrix build (macOS arm64 and x64, Windows, Linux),
  `just viewer` then `cargo tauri build`, CLI archives per platform, Python wheels (R2.4), the browser-only site, the
  manual PDF; the artefacts attach to a draft GitHub Release with notes from the CHANGELOG. A `workflow_dispatch` dry run
  builds everything and publishes nothing.
- **Package smoke test in CI, on each OS, from the installed artefact (not the build tree):** install, run
  `tlmbench --version`, open the example recording through the daemon (`--daemon`, as `t_browser.js` does) and read a view,
  check the manual PDF is present and opens, check the app doesn't write next to a read-only folder (`just ro-test`
  already does this for the build tree). Uninstall cleanly.
- **Versioning in one place:** the workspace version feeds `tauri.conf.json`, `python/pyproject.toml`, the viewer's
  export format version and `--version`; a script fails CI when they differ. 1.0.0 per decision 9.
- **Update path:** v1 ships no auto-updater unless the design says so (it doesn't); the About box links to the releases
  page. Say so in the manual rather than leaving users to wonder.
- Done when a `v1.0.0-rc.1` tag on a fork or a dry run produces every artefact, each installs and passes its smoke test on a
  clean machine or VM, and the ad-hoc signed `.dmg` opens after Open Anyway (decision 3 as changed Nov 22: not notarised).

### R5 Real-window passes and fixes (L, with the owner)

Headless tests cannot see Finder drops, native dialogs, WKWebView or WebView2 or WebKitGTK text and fonts, window
dragging between windows, GPU differences, the high-DPI look, or the installers. For each OS the agent hands the owner a
checklist (R1's Owner pass rows, the audit's real-window list, the first-run flow from the installer, and the apple-design
audit on that OS's installed build), the owner runs it on
the installed release candidate, and the agent fixes what they list, ranked. Needs: a Windows and a Linux machine or VM
the owner can use (ask early; if the owner has none, say which checks stay unverified and note it in the release notes
rather than claim them). Include WebGL2 absent or software-only (the Canvas 2D renderer) and a read-only network share.
Done when each OS's copy of `docs/release/test-plan.md` is signed off (owner, Nov 23: one plan, P or F per item).

### R6 Performance and hardening (M)

- Re-run every gate on the release build of the final commit and write the results with dates into the design's Roadmap
  section: Gates 1 to 5 (`just gate`, `gate2` over the 10 GB file, `gate3 core` and `app`, `gate4`, `gate5`).
- Measure what earlier steps left unmeasured: 100 widgets under 2 ms a frame in the release app; the table scroll at
  60 fps; sort of 10M rows under 2 s; the Canvas 2D renderer (state a budget and measure it); first chart view of a
  Light export from a remote static host, not only localhost; browser-only mode on a large CSV.
- Run the fuzz targets for a fixed time (`just fuzz-smoke`, then longer on the decoders), the read-only proof, and a
  soak: a live tab for an hour at 4M samples/s with memory flat.
- Security review of the surfaces a stranger can reach: files and exports opened from untrusted sources (decoders,
  descriptor TOML, plugins in the sandbox, hostile-plugin test), exported HTML (no script injection through channel
  names, event text or user SVG; the sanitiser), the app's CSP and `tlmwidget` scheme, the daemon's socket and token,
  and dependency audit (`cargo audit`, `cargo deny`, `npm audit`) with each finding fixed or justified in writing.
- Done when every budget in the design is a measured line, with the machine and the date.

### R6c Coverage (L, owner Nov 24: as close to 100% as possible, and in CI; tests only)

**Goal (owner, Nov 24): line coverage as close to 100% as it can honestly get, Rust and the UI, and coverage measured in
CI so it can't slide back.** Tests only: the UI stays frozen, and a test that finds a bug fixes it (with the test). Each
sub-step below is one agent run's worth or less and can be handed off between agents: record where it stopped in the
status table's R6c row (the numbers and the commit they were measured on), and pick up from there.

**Sub-steps, in order:**

- **R6c.1 Rust: the five gaps** (below). Done Nov 24.
- **R6c.2 JavaScript: the first report.** Done Nov 24 on `441c477`'s UI (the 120 UI tests that use `tests/lib.js`; the
  hook is `TLM_JS_COVERAGE=<dir> ./run.sh`, the build `npx vite build --sourcemap inline`): **91.7% of source lines, 75.9%
  of branches, 84.5% of functions** over 82 files of `app/ui/src`. Not yet measured: the 13 tests that start their own
  browser (the browser-only, viewer and real-core tests), so `browser*.ts`, `wasmcore.ts`, `wasmcap.ts`, `parquet-wasm.ts`
  and `viewer*.ts` have no figure. Most lines missed: `app.ts` (182 of 2,361), `gate.ts` (all 99: it runs only in the
  release gates), `CheckEditor`, `stack.svelte.ts`, `Table`, `Widget`, `Settings`, `TopBar` (14–19 each); lowest
  branches: `settings.svelte.ts` 51%, `workspaces.ts` 54%, `CheckEditor` 63%, `DescriptorForm` 64%, `Widget` 65%.
  Converting: merge the tests' V8 coverage per script first (`@bcoe/v8-coverage`'s `mergeProcessCovs`), then
  `monocart-coverage-reports` (both by `npm install` in a scratch folder, not the repo): fed 120 raw files directly it
  runs out of memory even at 5 GB.
- **R6c.3 Rust toward 100%, one crate a run:** decode, live, store, export, core, expr, cli, plugin, then app/src-tauri
  (its Tauri commands: call them as functions, or through `tauri::test`'s mock runtime) and tools/gen (its writers
  against the readers). For each crate: list every uncovered line range (`cargo llvm-cov report --json`, the ranges as
  the Nov 24 script printed them: segments with a zero count), add tests until what is left is only what can't be
  reached in a test, and write that remainder down with the reason (below).
- **R6c.4 The UI toward 100%:** the same for `app/ui/src`, file by file, with UI tests against the fake core (and
  the browser-only tests for what only runs there). Bring the ~15 tests that launch their own browser into the
  coverage hook first.
- **R6c.5 Coverage in CI** (below).

**What may stay uncovered** (written down per file, never silently): code only another OS runs (counted on that OS's
runner once CI measures it), a panic or `unreachable!` that guards an invariant, and a failure a test can't cause
without breaking the machine (a full disk, a lost device mid-write, out of memory). Each such range gets a one-line
reason in `docs/developers/coverage.md` (write it with R6c.3's first crate). Nothing else is excused.

**R6c.5, coverage in CI (recommended shape; the owner asked for it, so build it once R6c.3 has the numbers to hold).**
A `coverage` job on Linux only (the other runners stay as they are): `cargo llvm-cov nextest --workspace` and the UI
suite with the coverage hook, each writing a summary. It fails when a total, or any crate's or UI file's figure,
falls below the figure recorded in a committed baseline (`ci/coverage.json`): a ratchet, raised by the commit that
raises the figure, never a fixed target that tempts a test written for the number. It publishes the HTML reports as
the job's artifact. It adds about 20 minutes to the Linux job's time (a separate job runs alongside, not after). The
owner OKs the job's first version before it lands, as for any change to `ci/` or the workflows.

**Rust (measured Nov 24 on `ff087fc`):** `cargo binstall cargo-llvm-cov` and `rustup component add llvm-tools-preview`,
then `CARGO_INCREMENTAL=0 cargo llvm-cov nextest --workspace --no-report`, `cargo llvm-cov report --summary-only` (and
`--json` for line ranges). It builds into `target/llvm-cov-target` (17 GB: delete it once the report is written; check
`df -h /` first). Result: 76.5% of lines; expr 95%, core 94%, live 92%, decode 90%, export 89%, store 86%, plugin 81%,
cli 80%, app/src-tauri 28% (its commands run under the UI tests' fake core and the gates, which this doesn't count),
tools/gen 0% (the gates' generator). The gaps to close, one test each, in this order:

1. **Live: a record that straddles the ring buffer's wrap** (`crates/live/src/lib.rs`, the `straddle` stitching in the
   drain loop): records whose size doesn't divide the ring, written across the wrap, come out whole and in order.
2. **Aligning two files** (`crates/store/src/align.rs`, 62%): `estimate` with Starts, Clock, Manual, Sync (pairs,
   straight and piecewise), Events (by name and the most common shared name; "no shared event" refused) and the
   time-channel method, each checked against a known offset or drift; bad input (a time that isn't whole nanoseconds)
   refused.
3. **Two files in one tab** (`crates/store/src/group.rs`, 68%): `stats` over the shared span (each file's channels
   through its map), `widgets_frame` (each file answers its own widgets), `status` (progress weighted by samples,
   Missing when a file is gone).
4. **Tailing a growing file** (`crates/live/src/tail.rs`): a growing `.tlm` is recognised by its magic (not read as
   CSV), and a quoted CSV field with a newline split across two reads is held until it completes.
5. **A field a payload plugin decodes** (`crates/decode/src/payload.rs`, `WithPluginFields`): its view, reads,
   summaries and stats through the wrapper, against the plugin's values (the template-payload plugin, as the
   existing payload tests use).

**After the five tests (Nov 24, `b6580c6`, 325 tests):** 77.5% of lines; store 89.4%, live 93.8%, decode 90.9%; `align.rs` 61.8% → 93.6%, `group.rs` 67.5% → 89.2%, `tail.rs` 90.5% → 95.7%, `payload.rs` 69.2% → 82.9%, live `lib.rs` 91.5% → 92.5%. What these files still miss is of the kinds below.

Not worth a test: data-type match arms no sample file uses, the read-ahead for mapped files, plain delegations, and
`parquet_core.rs` (the WASM crate's own tests cover it, outside the workspace run).

**JavaScript (not measured yet):** the app's UI code under the 135 UI tests. Steps: (a) build the UI with source maps
for the run only (`vite build --sourcemap`, not committed as the default); (b) in `app/ui/tests/lib.js` (the shared
`run`) and the ~16 tests that launch their own browser, start `page.coverage.startJSCoverage()` and write
`stopJSCoverage()`'s result to `$TLM_JS_COVERAGE/<test>.json` only when that variable is set; (c) convert with a tool
run by `npx` from the scratchpad (`monocart-coverage-reports` or `v8-to-istanbul` + `c8 report`), so no dependency is
added; (d) report per file of `app/ui/src`, list the real gaps (error paths, dialogs' refusals, settings that change
behaviour) and add UI tests for them as for Rust. Never run it while cargo builds (the UI harness fails at boot).

**Done when** every crate and UI file is as covered as it can be, with what remains written down and reasoned in
`docs/developers/coverage.md`; the coverage job runs in CI with its baseline; and the full suites and CI are green.

### R7 Documentation (L)

Written against the frozen UI. Sources of truth: the design (what is promised), the ledger (what is true), the app.

- **README.md, production level.** What it is in three lines, a hero screenshot, install (each OS, Python, the CLI,
  browser-only), a five-minute first run on the bundled sample recording, the three things it is not (a data logger,
  an editor of recordings, a Python host), the hard requirements as promises, links to the manual and guides, licence,
  how to report a bug. Move the progress report ("What works today", "Where it's going", the phase table) out; it is
  history. Badges only for things that exist (CI, release).
- **User Manual** (`docs/manual/`, Markdown, one chapter per file, built to one PDF). Getting started; the window
  (rail, Explorer, tabs, panes, overview); opening data (formats, the CSV import dialog, descriptors, packets, plugins);
  reading data (zoom, cursors, Measure, tables, filters, time modes, markers); channels (display, limits, missing
  values, derived channels and the function reference); several files and alignment; events; rules and checks (the
  expression language, reports, folder runs); widgets and gauges; live (every source kind, triggers, recording);
  sharing (`.tlmbench/`, Share, read-only folders); exporting (data, images, HTML Full and Light, what a viewer can do);
  multiple windows; settings reference; keyboard and mouse reference; the command line (generated from `--help` so it
  cannot drift); Python; browser-only mode; troubleshooting and FAQ (cache location and clearing, "why is this coarse",
  time going backwards, a file the core can't read); glossary. Every screenshot has a caption saying what to look at.
- **Developers guide** (`docs/developers/`): architecture (a page and a diagram: core, IPC, store, pyramid, renderer,
  daemon, viewer, WASM), repository tour, building and testing (absorbs `docs/development.md`), the test suites and the
  fake core, the gates and how they are judged, the file formats (`.tlm`, the cache, the descriptor TOML, `.tlmbench/`,
  the export folder format and its `manifest`, the wire protocol), writing a decoder plugin and an alignment estimator
  (absorbs `plugins/README.md`; the WIT package is the reference), the C plugin interface, adding a UI control the way
  CLAUDE.md's conventions describe, release process (R4's workflow as a runbook), contribution rules.
- **Reference pages generated from the source** so they stay true: CLI (`tlmbench --help` for every command), Python API
  (docstrings), the function table for expressions (from `tlmbench-expr`'s registry), settings keys, and the keyboard
  list (from the app's shortcut table). A CI check regenerates them and fails on a diff.
- **Screenshots, fresh and repeatable.** One script (`app/ui/tests/shots.js` extended) takes every screenshot used by
  the README and the manual from a fixed demo recording set written by `tlmbench-gen` (same data, same window size, light
  and dark where it matters), writes them to `docs/images/`, and a list says which page uses which; screenshots that only
  changed by noise are reverted. Real-window shots (native dialogs, Finder) are taken by hand on the Mac for the manual
  chapters that need them.
- **PDF build:** Markdown to one PDF with a table of contents, page numbers, working internal links, and the version on
  the cover. Recommendation: render the Markdown to HTML and print with the headless Chromium Playwright already pins
  (no LaTeX or Pandoc dependency), through `just manual`. CI builds it and fails on a broken link or a missing image.
- **In the app (decision 6):** Help in the ⋯ menu and the start page opens the bundled manual; Online manual opens the web
  copy (decision 7); About shows the version, licence, the open-source notices and the link to releases. Add
  `THIRD_PARTY_LICENSES` generated from the dependency tree (`cargo about`, `license-checker`) and bundle it.
- **Design doc final sync.** Fold the "for the design doc" notes at the end of `docs/follow-ups.md` into `docs/design.md`,
  mark every decision's final state, update the roadmap section with the gate results, and export it from the living doc
  (links in `docs/README.md`).
- **CHANGELOG.md** (the 1.0.0 entry, by user-facing area), **CONTRIBUTING.md**, **SECURITY.md** (how to report), **LICENSE**
  (decision 1) and a `docs/README.md` index of what exists.
- Done when: a person who has never seen the project can install it, open the sample recording and reach a check
  result from the README alone; every control in the app is named in the manual (a script lists the UI's controls from
  the tests' selectors and the manual's index and shows what is missing); and CI builds the PDF.

### R8 Cleanup (S)

After R7 has absorbed what the plan documents hold (git history keeps them). Nothing is deleted before the owner agrees to
the list (decision 8).

- Delete (`next-agent-5.8.md` and `review-prompt.md` already went Nov 15): `docs/roadmap.md`, `docs/feature-matrix.md`, `docs/parity-audit.md`,
  `docs/follow-ups.md`, and `docs/release/*` except what the guides link to (the ledger and its checker are kept until
  the tag; then the ledger becomes the shipped "features and where they are tested" appendix of the developers guide or is
  removed). `docs/images/` stays, pruned to what is used.
- Inspect and prune: stray fixtures (`prototype/tests/fold/…` is a test fixture, keep it with its tests), scripts nothing
  calls (`tools/`, `ci/`: check each against the justfile and the workflows), recipes in the `justfile` that only served
  development (keep gates and benchmarks, they are part of the project's proof), unused settings and dead code the ledger
  found, `#[allow]` and TODO comments, ignored tests, `bench/history.json` if present, anything under `.gitignore`d
  paths that the owner keeps by hand. `cargo machete` or `cargo udeps` for unused dependencies; `knip` for the UI.
- The README's repository section and `CLAUDE.md` updated: CLAUDE.md describes how to work in the repo, so remove the
  phase narrative from it and keep the requirements, conventions and recipes.
- Done when `git ls-files` holds nothing the README, the guides or the build doesn't explain, and a clean clone passes
  `just ci-linux` and the native recipes.

### R9 Release candidate and release (M)

- Tag `v1.0.0-rc.1`; the workflow builds everything; the owner installs from the release page on each OS and repeats
  R5's first-run checklist; fix, tag `rc.2`, and so on. The RC is feature-frozen: only bug fixes.
- Final checks: `ci/ledger_check.py` (no row other than Done, or Owner pass signed off), all gates re-run on the tagged
  commit, CI green with no rerun, the package smoke tests green, `SHA256SUMS` and attestations present, links in the
  README, manual and app all resolve (a link checker in CI), the PDF in the installer is the one for this version.
- Publish: tag `v1.0.0`, the workflow publishes the draft release and the wheels; the owner reviews the draft and
  publishes it; deploy the web copy (manual, browser-only app, sample export); announce only after a clean install on
  each OS from the public page.
- Done when the public release page has installers for every supported platform, each verified, and the owner has
  said so.

## Order and dependencies

R0 → R1 → R2.1 → R2.1a → **R3.0** → **R3.1 → R3.2** → R2.2 → R2.3 → R2.4 → R2.5 → R3.3 → R3.4 → R4

(Owner, Nov 15: R3.3's UI-only rows, the ones that need no core work, are pulled forward for Sonnet to take after R2.2's Sonnet part, R2.3 and R2.4, before the Opus steps. Its core-dependent rows stay after R2.5.) → R5 → R6 → R7 → R8 → R9.

**Owner, Nov 23: the order from here.** The owner tests once, near the end. So the agents do everything that needs no
owner first: R6 (security, fuzz, gates and measurements, soak), R7 (README, manual, guides, LICENSE, the release
notes and install instructions per OS), R8 (cleanup; the delete list still goes to the owner), and the release
candidate build (R9's `v1.0.0-rc.1`: installers, binaries and instructions for macOS, Windows and Linux on a draft
release). Then `owner-test-sheet.md` and `real-window-passes.md` are replaced by one test plan
(`docs/release/test-plan.md`) the owner runs on each OS from the RC's installers: short steps, Pass or Fail with an
optional note, and scripts that generate the same test data on every OS. R5 is that run; its findings are fixed and
the next `rc.N` follows. The sheet's open sections (Y2 to Y8, Z1 to Z3) go into the test plan, not lost.

R0 closes with the first two clean CI runs; R2.1a closes with the owner's re-test, which can run alongside the steps after it.

- R1 comes first because its ledger defines R2 and R3 (the lists above are what the follow-ups and the audit say today;
  the ledger will add and remove rows, and it wins).
- R2.1 settled the store for WASM (it builds behind a `native` feature; the viewer's queries stay in `crates/web`).
- R3.0 runs before the rest of R2 because it is read-only and its "App is better" list protects work R2 might touch.
- R3.1 and R3.2 run before R2.2 (owner, Nov 15): the owner is testing the desktop app and every finding of Nov 15 is
  theirs. Two items wait for core work in R2.5 and are finished there: the import profile's starter layout and recipes
  (P5, S5.1-10) and Connect's reconnect (D-live-11). R3.2 builds the dialog around them and leaves those parts.
- R4 comes before the real-window passes so there are installers to test, and before the docs so the manual PDF has a
  place in the bundle (use a placeholder PDF until R7). A bug R5 finds goes back to a short fix step, not to a new plan.
- R7 needs the UI frozen (end of R3) and the packaging known (R4); screenshots are taken last, after R6's fixes.
- R8 is after R7 because the plan documents are inputs to it.
- Anything the owner's passes find after R3 is a bug fix, never new scope. A genuinely new idea goes to
  `docs/post-v1.md` (create it) for the owner to rank after release.

## Release gate: what "done" means

All of these, each checkable:

1. `ci/ledger_check.py` passes and no row is Missing, Partial, Diverged or Unverified; every Owner pass row is signed off.
2. CI is green on `main` and on the tag with no reruns, on Linux, macOS and Windows.
3. Gates 1 to 5 pass on the tagged release build, results dated in the design.
4. `just ro-test` passes (nothing written next to the data) and the hard requirements in `CLAUDE.md` each have a test
   named in the ledger.
5. Every platform in decision 2 has a signed, installed, smoke-tested artefact.
6. The manual PDF is in each installer, opens offline from Help, and matches the app's version.
7. README, manual, developers guide, CHANGELOG, LICENSE, SECURITY and CONTRIBUTING exist and their links resolve.
8. The cleanup list is done and `git ls-files` explains itself.
9. The owner has signed off the real-window checklist on each OS they could test, and the release notes say which they
   could not.
