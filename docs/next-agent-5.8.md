You're continuing TelemetryBench (tlmbench) in /Users/rafy/workspace/tlmbench, branch main (Rust core, Tauri app with Svelte UI, CLI, Python bindings).

STATE
- Phases 1–4 and roadmap steps 5.1–5.7 are done. 5.1–5.6 are pushed and green on CI. 5.7 (shared configuration) is built; see docs/roadmap.md's status table for whether it is pushed and CI is green.
- Next step: **5.8 Multiple windows** (docs/roadmap.md). Before starting, ask the owner to confirm 5.8 as the next step.
- Memory: /Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/ (roadmap-v1.md has the step notes).

READ FIRST
- CLAUDE.md (binding). docs/roadmap.md (the 5.8 section and the status table), docs/feature-matrix.md (rows marked 5.8), docs/design.md (multiple windows, the daemon and its socket, sessions), docs/follow-ups.md ("Left from step 5.7" and anything that says 5.8), docs/development.md.

WHAT 5.7 BUILT (so you don't redo it)
- Core `crates/store/src/config.rs` (items, compare, share, pull, export, validate, overlay, `own.json` mirror), `tlmbench validate`, `tlmbench config export --out`, `--config`, decoders from the nearest `.tlmbench/`.
- Tauri `config_use/scan/share/pull/export/gitignore/mirror`, `timemap_save/find`; app `shell/ShareDialog.svelte`, `share.svelte.ts`, the "Shared configuration" block in app.ts, Settings › Shared configuration.
- Tests: store and CLI `tests/config.rs`, `t_share`, `t_sharedgroup`. Design notes to absorb: docs/follow-ups.md "Left from step 5.7".
- Windows share the app cache and settings: settings live in webview localStorage today (per window origin), the core's overlay (`config::set_overlay`) and the decode config dirs are process-wide globals. Both matter for 5.8.

WORKING NOTES (from earlier sessions)
- After any UI edit: `cd app/ui && npx svelte-check --fail-on-warnings && npm run build`.
- UI tests run from `app/ui`, one at a time with `node tests/t_x.js`, never in parallel and never while a cargo build or the parity harness runs (8 GB machine). They use the fake core (`tests/fakecore.js`, which does not parse expressions; `opts.setup`/`opts.settings` in `tests/lib.js` seed it before the page loads). A failing check leaves no screenshot; a thrown error does (`tests/out/fail_<name>.png`).
- The fake core cannot catch Rust deadlocks or lock-order bugs (5.5 had one). Run `just gate3 app` before pushing anything that touches the Tauri commands. gate3 covers only the live TCP path, so also test new commands at the store level.
- Full UI suite: loop over `tests/t_*.js` (the prototype owns `t_loading`; run `t_perf_gauges` alone at the end). A script is easy to recreate: for each test print the last PASS/FAIL line. Several tests hard-code lists (the view menu, the file right-click menu in t_channels, the All settings count in t_shell, event category counts): update them when you add items.
- Docs screenshots: `node app/ui/tests/shots.js` (fake core) rewrites docs/images (revert files that only changed by noise); README.md describes what works today, update it with the step.
- **Before pushing,** run what CI runs: `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` (clippy over the whole workspace catches matches elsewhere), the Rust tests of the crates you touched, the whole UI suite, and `just app-parity` (it reports differences, it doesn't fail; add a scene for new dialogs and tidy what you introduced; the last total was 211 differences over 23 scenes).
- **Rust:** `CARGO_INCREMENTAL=0` and workspace-wide cargo. Commands over 120 s go in the background. Don't run two cargo commands at once. After a `cargo clean -p` the first build takes 10+ minutes. A release build for gate3 needs several GB.
- **Disk is tight.** `target/` grows to 30 GB. The fix is `cargo clean -p` for each workspace crate (tlmbench-store, -decode, -core, -expr, -export, -live, tlmbench, tlmbench-app, tlmbench-gen) and `rm -rf target/debug/incremental`. The owner has OK'd this.
- **CI:** takes 35–45 minutes. Wait with a background `until` loop on `gh run view <id> --json status`. Check failures with `gh run view <id> --log-failed`. Timing tests and macOS runner cancellations are the usual noise (`gh run rerun <id> --failed`).
- **Shell:** the safety check sometimes rejects compound commands and inline Python heredocs that contain slices or `rm`-like text. Write scripts to the scratchpad directory and run them. `setsid` doesn't exist on this Mac; use the Bash tool's background mode. `git add -A` sweeps in untracked files: check `git status` first.
- **Commits:** small, one topic each, ending with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`. The owner has asked for steps to be pushed and passing CI before they are called done, so push when the step is complete and wait for CI.
- **Finishing a step:** update the roadmap status table, the feature-matrix rows, the tests README table, docs/follow-ups.md, docs/parity-audit.md, README.md and its screenshots; note behaviour the design doc should absorb; update the memory files; write the next hand-off.
- **Owner's preferences:** concise replies that lead with what changed and what's left, no recaps. Decisions belong to the owner: ask with a short recommendation. Everything in the design is v1: don't propose cutting scope. Make the app look more polished as you touch it. Controls appear on selection or hover; rarer actions go in ⋯ or right-click menus (5.6 learned this: a third button on the selected Explorer row shifted under a double-click).

