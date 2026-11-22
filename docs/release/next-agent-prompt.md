You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to v1.0.0.
One agent works at a time: start from a clean `main`, leave a clean `main`. If another Claude session is running in this
repository, agree with it which files each of you touches before you edit.

This prompt is only a hand-off. **Where each step stands is the status table in `docs/release/release-plan.md`; what is
done is `docs/release/ledger.md`.** If this prompt and those disagree, they win, and you fix this prompt.

Written Nov 22 after the second Nov 22 run (S5.6-13, the rest of R3.4, R4's first part). What it built is in the
status table's "Second Nov 22 run" and "R4" rows; the CI result is in the CI row.

## First

1. Read `CLAUDE.md` (its hard requirements bind you), then `docs/release/release-plan.md` (the status table) and
   `docs/release/owner-test-sheet.md` (sections Y and Z are what the owner hasn't tried yet; fill in anything they wrote).
2. Skim the memory files in `/Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/`, above all the
   owner-decisions files (newest: `owner-decisions-nov22b.md`).
3. Run the open-rows script: `python3 ci/ledger_check.py`, then list rows that aren't Done or Owner pass.
4. Ledger counts at the hand-off: Done 339, Partial 5, Missing 2, Diverged 1, Unverified 24, Owner pass 17 (388 rows).

## The UI is frozen (Nov 22)

R3.4 is done: from here only bug fixes, the owner's sheet results and R4's own items touch `app/ui/src`. A new idea or an
audit finding that isn't a bug goes to `docs/post-v1.md`.

## Built Nov 22 (both runs, all pushed): wait for the owner's sheet before changing them

- First run: the two blockers (D-ui-65, D-ui-66), X1 to X4, W2 to W4, the Report tab, canvas traces. Sheet section Y.
- Second run: S5.6-13 (tables and XY across files), R3.4 (End, shortcuts per platform, layout undo, the review), files
  opened from the OS and Install command-line tool… (D-ui-71, D-ui-72). Sheet section Z.
- **Nothing in Y or Z was seen in a real window by an agent**, the WKWebView drag above all (Y1). If the owner reports a
  failure there, that comes first.

## Open questions for the owner (ask, with a recommendation; don't decide)

- **Quiet interface's default** (W3): asked Nov 22; the owner chose to decide after trying Y5. Ask again once Y5 has a
  result (recommendation then: whatever their Y5 note says; otherwise keep it off).
- **R4's dry run**: it needs the owner's signing secrets (listed in `docs/development.md`, Releases) and PyPI's trusted
  publisher for `release.yml`, and it bills a full matrix build. Ask before starting it (`workflow_dispatch`, or a tag
  `v0.1.0-rc.1`), recommending the hand-started run first (it publishes nothing).

## Your rows

- **First, before anything else in the plan: the full apple-design audit** (owner, Nov 22). The last whole-app audit was
  Nov 17; everything built Nov 22 (Your widgets editor, the Report tab, pins, the results tree, quiet pane headers,
  Install command-line tool…, the shortcut names) has never been audited. Load the `anthropic-skills:apple-design`
  skill (if it isn't available, do the pass by hand against its headings and say so) and run it on the whole app, dark
  and light, with motion and reduced motion: screenshots from `node app/ui/tests/shots.js <out-folder>` (never with no
  argument) and the parity scenes (`just app-parity`), plus the relevant Svelte and CSS. Follow the plan's "Rules for the
  audits": the prototype and CLAUDE.md's conventions win over the skill; findings ranked, each with its screenshot, in a
  dated section of `docs/release/ui-audits.md`, and a line in the status table. The UI is frozen, so fix only what is a
  bug (cite the finding, add a test); anything that would change the design is a question for the owner with a
  recommendation; the rest goes to `docs/post-v1.md`. Then continue with the rows below.
- **R4, the rest:** the dry run once the owner agrees, then fix what it finds (it has never run: expect paths in the
  bundles, the NSIS install folder, the deb's package name and the AppImage's layout to need a look); D-cli-02 and
  D-ui-72 are Done after it. The notarised `.dmg` on a clean Mac (Gatekeeper) is the owner's to check. `just bundle-mac`
  builds the macOS bundles locally (an agent's run of it was refused by the permission check on Nov 22: ask first).
  The Windows code in `app/src-tauri/src/install.rs` (PATH through the registry) is compiled only by CI's Windows job.
- **D-gauge-04** waits for R6's measurement; **D-live-09** is the owner's (review decision 5).
- Then R5 (real-window passes with the owner) and R6, per the plan. The manual PDF (R7) joins the bundle's resources
  when it exists; the licence (decision 1: Apache-2.0, CC BY 4.0) lands with R7's LICENSE.

## What is decided, and what you must not decide

- **Owner decisions are the owner's.** Where the design says "Open" or two readings are reasonable, stop the row and ask.
- **Never propose cutting scope.** Post-v1 items go to `docs/post-v1.md`.
- **"App is better" rows are never ported back.**
- The design (`docs/design.md`) is the source of truth; a behaviour change needs a note in `docs/follow-ups.md` (the
  Nov 22 notes are under "The owner's test notes (Nov 22)").

## Rules

- **Done means, for each row:** built, a test for it (and the tests README's table), the ledger row moved with evidence and
  `python3 ci/ledger_check.py` passing with its counts line updated, `docs/follow-ups.md` where behaviour differs from the
  design, and an owner sheet item when only a real window shows it. Small commits, one row or topic each.
- **Push** once your rows are done and `just app-test`, `just proto-test`, `cargo nextest run --workspace`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` and `npm run typecheck` pass locally,
  plus `cargo clippy --target wasm32-unknown-unknown -- -D warnings` and `cargo fmt --check` in `crates/web` and
  `crates/web-parquet` (they include `decode` files as source: a new dependency in `decode` goes into their `Cargo.toml`
  and lock files too), and `python`. Then watch CI to green. A full run takes about 1.5 hours and bills about 545 minutes:
  don't push per row. **Don't change `ci/` or the CI image.**
- **Lessons (Nov 17):**
  - The UI tests' harness waits a fixed 600 ms for the page; under a cargo build every core is busy and tests fail at boot
    (`__tlmApp` undefined). Never run the UI suite while cargo builds; rerun such failures alone.
  - `node app/ui/tests/shots.js` with no argument writes into `docs/images` (tracked): pass an output folder.
  - The parity harness (`just app-parity`) found two real layout faults on its first run of new scenes: add a scene when
    you touch a panel.
  - Browser-only mode keeps a tab per packet type (several recordings in one browser tab are after v1); the app's slots
    path is skipped there (`ui.browserOnly`).
  - `[profile.test]` has `debug = false` (Windows' PDB limit). Known flakes under full-suite load: `t_browser`,
    `t_descriptor`, `t_windows` (pass alone); `t_browserfolder` failed once on a loaded Linux runner.
- **Never edit Rust sources while a cargo build is going** (tests that only run already-built binaries are fine). Start
  background commands with `</dev/null`. A change to `decode` or `store` means a 20 to 40 minute workspace test build.
- The Mac has 8 GB of memory and a small disk. Never run two cargo builds or two UI suites at once. Check `df -h /`;
  `rm -rf target/debug` is fine (the owner OK'd it). `CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0` keeps a workspace run
  near 8 GB.
- Build order: `just viewer` before the CLI or the app (and before the browser tests: `app/browser/dist`); `cargo build
  --release -p tlmbench` for the release CLI. UI tests: `./run.sh t_name` in `app/ui/tests`; the fake core is `fakecore.js`.
- In zsh write `"${var}:path"` when a colon follows a variable. macOS `sed -i ''`; never `git stash`; add files by name;
  don't rewrite history. End commits with the attribution line your session's instructions give.
- Replies to the owner: concise, lead with what changed and what's left.

- **Lessons (Nov 22):**
  - `tlmbench-plugin::estimator the_template_estimator_lines_two_files_up_through_the_store` failed once under the full
    workspace run and passed alone and on a full rerun: a load flake to watch, like the UI ones above.
  - Run cargo workspace-wide only (`-p` builds another feature set and fills the disk); clean `target/debug` when done.
  - A `git add` with a path that no longer exists (a file you `git mv`ed) aborts an `&&` chain: the commit then holds only
    what was staged before. Check `git show --stat HEAD` after a multi-file commit.
  - Browser-only tests run against `app/browser/dist`, built by `just viewer`: rebuild it after UI changes or a local
    pass proves nothing (CI builds it fresh). A wait whose result isn't checked hides a slow boot as a later failure.
  - The parity scenes `drag-edge*` report a 2 px and 6 px size gap in the drop label box; known noise, not behaviour.
- **Lessons (Nov 22, second run):**
  - `maturin develop` refuses to run when both `VIRTUAL_ENV` and `CONDA_PREFIX` are set: `unset CONDA_PREFIX` first.
  - The fake core (`app/ui/tests/fakecore.js`) already has a field for most names (`opened` is the list of opens):
    grep before adding one, or a test breaks far from your change.
  - A test step that moved the pointer (⌘Z after a click on `#stack`) hides hover-only buttons: hover again first.

## Report when you stop

1. Update the status table in `docs/release/release-plan.md` and push as above.
2. Rewrite this prompt for the next run.
3. Tell the owner: what changed (rows Done), the ledger counts, the questions waiting on them (each with your
   recommendation), the rows left, and the CI result. Then stop.
