You're continuing TelemetryBench (tlmbench) in /Users/rafy/workspace/tlmbench, branch main (Rust core, Tauri app with Svelte UI, CLI, Python bindings).

STATE
- Phases 1–4 and roadmap steps 5.1–5.5 are done and pushed. CI was green before 5.5; the run for 5.5 is the one after commit c431813 (check `gh run list`). `just gate3 app` passes on the Mac (0 of 3,540 and 3,538 frames missed, every record recorded).
- Next step: **5.6 Several files and alignment** (docs/roadmap.md). Before starting, ask the owner to confirm 5.6 as the next step.
- Memory: /Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/ (roadmap-v1.md is current through 5.5, but says 5.5 is unpushed and gate3 not run; both are done now: fix it).

READ FIRST
- CLAUDE.md (binding).
- docs/roadmap.md: the 5.6 section and the status table. docs/feature-matrix.md: rows marked 5.6 (about 15).
- docs/design.md: multiple files per tab, slots and short names (`accel_x·2`), alignment, time maps, sync points, event matching, cross-correlation, time-source channels, "Open together", "Compare runs", cross-file derived inputs.
- docs/follow-ups.md: "Left from step 5.5" (Add source, live items), 5.1 (cross-file `interp`), 4.3 (packet types as a tab per type; slower channels in tables), and anything that says "5.6".
- docs/development.md.
- The prototype's multi-file tests in prototype/tests (grep for `slot`, `align`, `primary`, `compare`, `sync`): they are what 5.6 must port. The prototype is prototype/plotter.html (one file; search for the alignment code).
- The app's current file handling: app/ui/src/app.ts (one `OpenFile` per tab; `tabData`, `showTab`), tabs.svelte.ts, crates/store (`Opened`, `Joined` in joined.rs is a recording plus derived channels: the closest existing pattern for several recordings as one), crates/core/src/recording.rs (the `Recording` trait, times).

5.6 SCOPE (from the roadmap)
- Several files in one tab with numbered slots (`accel_x·2`) and short names; Make primary; Channel prefix…; the + menu's needs and slot picker.
- Core: time maps (knots, the inverse at query time); Align starts, Same clock, Manual, Sync points, Event matching, Cross-correlation, time-source channels. Cross-file derived inputs (`interp`).
- UI: the Alignment section (residuals, quality, preview, nudge, lock); Open together; Compare runs.
- Done when the prototype's multi-file tests are ported and two files with a known drift align to within a sample.

WHAT 5.5 LEFT THAT 5.6 SHOULD PICK UP
- "+ Add source" joins a live session's folder (source-2.tlm) but opens in its own tab with its own clock. With several recordings in one tab, a live session's sources should be slots of one tab on one session clock (see docs/follow-ups.md "Add source"). The strip's "N sources" counts live tabs today and the Sources cards list every live tab.
- Live derived channels (`Store::open_live_derived`) join a derived part to one live recording; keep that working when a tab holds several files.
- The Explorer names a live file by its session name (set in `pollLive` via `ui.file`).

WORKING NOTES (from earlier sessions)
- After any UI edit: `cd app/ui && npx svelte-check --fail-on-warnings && npm run build`.
- UI tests run from `app/ui`, one at a time with `node tests/t_x.js`, never in parallel. They use the fake core (`tests/fakecore.js`, which does not parse expressions; `opts.setup` and `opts.setupArg` in `tests/lib.js` seed it before the page loads). A failing check leaves no screenshot; a thrown error does (`tests/out/fail_<name>.png`).
- The fake core cannot catch Rust deadlocks or lock-order bugs. 5.5 had one: a MutexGuard temporary in a struct literal (`stopped: l.source.lock()…`) lived to the end of the statement while `health: l.health()` locked the same mutex. Only the real app (`just gate3 app`) showed it. Run `just gate3 app` before pushing anything that touches the Tauri commands.
- Full UI suite: loop over `tests/t_*.js`, skipping `t_loading` and `t_perf_gauges`; run `t_perf_gauges` alone at the end. A script lives at the previous session's scratchpad (suite.sh); recreate it. `run.sh` now gives the timing tests 300 s (t_perf_gauges took 113 of 120 s on the CI runner).
- Several tests hard-code lists (the view menu's items in t_modes and t_tables, the All settings count in t_shell, the event category counts in t_events and t_eventmarks). Update them when you add items.
- Docs screenshots: `node app/ui/tests/shots.js` (fake core) rewrites docs/images; README.md describes what works today, update it with the step.
- **Before pushing,** run what CI's `lint` runs: `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`. Clippy with `--all-targets` over the whole workspace catches matches elsewhere that `cargo check` of one crate misses (5.5 broke `tools/gen` and the app's descriptor preview that way).
- **Rust:** `CARGO_INCREMENTAL=0` and workspace-wide cargo. Commands over 120 s go in the background with a Monitor until-loop. Don't run two cargo commands at once. After a `cargo clean -p` the first build takes 10+ minutes.
- **Disk is tight.** `target/` grows to 30 GB. The fix is `cargo clean -p` for each workspace crate (tlmbench-store, -decode, -core, -expr, -export, -live, tlmbench, tlmbench-app, tlmbench-gen) and `rm -rf target/debug/incremental`. The owner has OK'd this.
- **CI:** takes 30–45 minutes. Wait with a background `until` loop on `gh run view <id> --json status`. Check failures with `gh run view <id> --log-failed`. Timing tests are the usual failure.
- **Shell:** the safety check sometimes rejects compound commands (it did for a plain `python3 … && node …`). Write scripts to the scratchpad directory and run them. BSD `sed -i ''` works but fails on `&` in replacements; prefer Python replace scripts.
- **Commits:** small, one topic each, ending with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`. The owner has asked for steps to be pushed and passing CI before they are called done, so push when the step is complete and wait for CI.
- **Finishing a step:** update the roadmap status table, the feature-matrix rows, the tests README table, docs/follow-ups.md, README.md and its screenshots; note behaviour the design doc should absorb; update the memory files.
- **Owner's preferences:** concise replies that lead with what changed and what's left, no recaps. Decisions belong to the owner: ask with a short recommendation. Everything in the design is v1: don't propose cutting scope. Make the app look more polished as you touch it.

OPEN FOR THE OWNER (ask, don't decide)
- How a tab's files share one clock (a time map per file against the primary's clock) and how the pyramid, views, tables and derived channels see several files: a `Recording` that joins files (like `Joined`) with a time map applied at query time, or a merged pyramid. Recommend the first (no rebuild, the inverse map at query time).
- Whether a live session's sources become slots of one tab in this step (recommended, they are the same mechanism).

NOT VERIFIED IN 5.5 (worth a look on the Mac if you can run the app)
- File playback, triggers, live checks and derived channels on a live tab against the real core (gate3 exercises the TCP live path only). Core tests cover them: `crates/live/src/replay.rs`, `crates/store/tests/trigger.rs`.
- The `tlmwidget` scheme in a real webview and the 100-widget budget in the release app (from 5.4, still unmeasured).
