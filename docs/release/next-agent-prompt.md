You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to v1.0.0.
One agent works at a time: start from a clean `main`, leave a clean `main`. If another Claude session is running in this
repository, agree with it which files each of you touches before you edit.

This prompt is only a hand-off. **Where each step stands is the status table in `docs/release/release-plan.md`; what is
done is `docs/release/ledger.md`.** If this prompt and those disagree, they win, and you fix this prompt.

Written Nov 23 after the third Nov 22 run (the apple-design audit, R4's dry runs, R6's first security pass). What it
built is in the status table's audit, "R4 dry runs" and "R6" rows; the CI result is in the CI row.

## First

1. Read `CLAUDE.md` (its hard requirements bind you), then `docs/release/release-plan.md` (the status table) and
   `docs/release/owner-test-sheet.md`. **The owner is testing sections Y and Z now** (the local build `dc11da5`, and
   for Z3 the dry run's `.dmg`): every `FAIL` and non-empty `Notes:` comes before anything else, Y1 (the drag and close
   blockers in WKWebView) above all.
2. Skim the memory files in `/Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/`, above all the
   owner-decisions files (newest: `owner-decisions-nov22b.md`).
3. Run `python3 ci/ledger_check.py`. Counts at the hand-off: Done 340, Partial 3, Missing 2, Diverged 1, Unverified 24,
   Owner pass 18 (388 rows).

## The UI is frozen (Nov 22)

Only bug fixes, the owner's sheet results and R4/R6 items touch `app/ui/src`. A new idea or a finding that isn't a bug goes
to `docs/post-v1.md`. The last whole-app apple-design audit ran Nov 22 after the freeze (`ui-audits.md`); the next is R5's,
on each OS's installed build.

## Open questions for the owner (ask, with a recommendation; don't decide)

- **Quiet interface's default** (W3): ask once the owner has filled in Y5 (recommendation: whatever their Y5 note says;
  otherwise keep it off).
- **The Windows CI cache:** deleted Nov 23 (owner's choice) after it filled the runner's disk twice. If Windows fails again
  at "Restore cache" with "No space left on device", ask before changing `.github/workflows/ci.yml` to cap it
  (recommendation: delete it again first).

## Your rows

- **The owner's sheet (Y, Z)** first, as above.
- **R4, the rest:** the dry run is green (45110269772). Left: Z3 on the `.dmg` (owner; the README and R7's manual must say
  how to open an ad-hoc app the first time: note which path the owner reports, right-click › Open or Privacy & Security ›
  Open Anyway); PyPI's trusted publisher for `release.yml` and the SignPath application (owner); SignPath's GitHub action
  in place of the `.pfx` step once accepted (the owner holds its token); the manual PDF in the bundle (R7). A tag
  `v1.0.0-rc.N` is the full rehearsal later (R9).
- **R6, the rest** (`security-review.md` has the first pass): the longer fuzz (`just fuzz-smoke 300`, about 20 minutes;
  stopped Nov 23 for the owner's build: run it when the owner isn't using the Mac, then `rm -rf crates/decode/fuzz/target`),
  the hostile-plugin review on wasmtime 50.0.3, then every gate and measurement on the release build of the final commit
  (they need the big test files: `just testdata big`, `gen-gate`, `gen-gate2`; check the disk first), the soak. D-gauge-04
  waits for its measurement; D-live-09 is the owner's (review decision 5).
- **R5** (real-window passes with the owner, per OS) once the sheet is back; it needs a Windows and a Linux machine or VM:
  ask the owner early.
- R7 (docs, the licence and LICENSE), R8, R9 per the plan.

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
  and lock files too), and `python`. Then watch CI to green. A full run takes about 1.5 hours (the repository is public, so
  it costs nothing, but it holds the runners): don't push per row. **Don't change `ci/` or the CI image.**
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
- **Lessons (Nov 22, third run):**
  - This Mac's shell is bash 3 with no `jq`: a Monitor script using `declare -A` or `jq` fails silently. Use `gh … --jq`
    (no `--arg`) and plain files (a watch script: poll `gh run view --json`, print each newly
    completed job). A backtick in a Monitor command breaks it.
  - The disk filled twice in one run (a workspace rebuild after dependency upgrades on top of a stale `target/debug`).
    Check `df -h /` before a cargo run; under about 12 GB, `rm -rf target/debug` first.
  - The release workflow builds the universal macOS app one architecture at a time: each needs its own
    `binaries/tlmbench-<triple>` (fixed `14ab8d5`).
  - `t_browser` and `t_browserpayload` failed once each under the full suite and passed alone (cold daemon start, slow
    WASM boot): known load flakes. `t_svgsafe` needed a boot wait.
  - `cargo audit` and `cargo deny` are installed now (`~/.cargo/bin`).
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
