You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to v1.0.0.
One agent works at a time: start from a clean `main`, leave a clean `main`. If another Claude session is running in this
repository, agree with it which files each of you touches before you edit.

This prompt is only a hand-off. **Where each step stands is the status table in `docs/release/release-plan.md`; what is
done is `docs/release/ledger.md`.** If this prompt and those disagree, they win, and you fix this prompt.

Written Nov 23 (evening) after the run that did R6's fuzz and plugin review and started R7 and R8. What it built is in
the status table's R6, R7, R5 rows; the CI result is in the CI row.

## The owner's order (Nov 23): agents first, the owner tests once at the end

The owner tests once, near the end, from `docs/release/test-plan.md` (one pass per OS on the release candidate's
installers, P or F per item; data from `tools/testdata/make.py`). Don't hand the owner checklists mid-way: a
real-window item goes into the test plan. The owner test sheet is gone. The order now:

1. **R7, the rest:** screenshots for every `<!-- shot: id — … -->` in `docs/manual/` (extend `app/ui/tests/shots.js`,
   output to a folder you pass, then `docs/images/`; a list of which page uses which); the developers guide
   (`docs/developers/`, absorbing `docs/development.md` and `plugins/README.md`); reference pages generated from the source
   (CLI `--help`, the expression functions, settings keys, shortcuts) with a `--check` in `just lint`; the design doc's
   final sync (fold `docs/follow-ups.md` into `docs/design.md`: the living doc is in Claude Docs, links in
   `docs/README.md`; ask the owner before editing it); the web copy on GitHub Pages (the manual as HTML from
   `just manual --html`, the browser-only site, a sample export) at `https://rafyarvelo.github.io/tlmbench/` (the app's
   Online manual points at `/manual/`).
2. **R8, the rest:** the owner agreed the list Nov 23: the frozen history is deleted; at the tag, `follow-ups.md` (after
   the design sync), `ui-audits.md`, `release-plan.md`, `next-agent-prompt.md`, `security-review.md` (summarised in
   SECURITY.md) and the ledger (as the developers guide's appendix). Plus R8's pruning (scripts, recipes, dead code,
   `cargo machete`, `knip`).
3. **R6, last, on the final commit's release build:** every gate and measurement (`just testdata big`, `gen-gate`,
   `gen-gate2`; check `df -h /` first, delete the 10 GB file after) and the soak (an hour live at 4M samples/s).
4. **The release candidate:** `v1.0.0-rc.1` (ask the owner before pushing the tag: it is the outward step); the draft
   release must have every installer, the CLI archives, the manual inside the app, LICENSE and THIRD_PARTY_LICENSES.md.
   Then the owner runs the test plan per OS; fix every F (ranked), `rc.2`, and so on.

## First

1. Read `CLAUDE.md`, then `docs/release/release-plan.md` (the status table) and the memory files in
   `/Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/` (newest: `owner-order-nov23.md`).
2. Run `python3 ci/ledger_check.py`. Counts at the hand-off: Done 340, Partial 3, Missing 2, Diverged 1, Unverified 24,
   Owner pass 18 (388 rows).
3. Check that the last push is green (see the CI row) and push what is local only with the next code commit.

## Open with the owner

- **Quiet interface's default** (W3): now item 4.7 of the test plan (the owner writes yes or no).
- **Z3's first-open path:** item 1.2 of the test plan; `docs/install.md` already gives both (macOS 15+: Open Anyway;
  14 and earlier: right-click › Open).
- **The Windows CI cache:** if Windows fails at "Restore cache" with "No space left on device", ask before changing the
  workflow (recommendation: delete the cache again).
- **R5's machines:** the owner has no Windows or Linux machine yet (Nov 23). If none by the RC, the release notes say
  which checks stay unverified there.
- PyPI's trusted publisher and SignPath (the owner's), as before.

## The UI is frozen (Nov 22)

Only bug fixes and R7's own items (Help's manual, About) touch `app/ui/src`. A new idea goes to `docs/post-v1.md`.

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

- **Lessons (Nov 23):**
  - The UI tests run Chromium, which has a real `window.prompt` and a synchronous `confirm`; the app's WKWebView has no
    `prompt` (wry answers null) and Tauri's `confirm` returns a promise. Never use either: `askText` (`ask.svelte.ts`)
    asks, and a `confirm` is awaited.
  - Never spread a large array into arguments (`push(...bytes)`): past about 100 KB the stack overflows (it broke
    browser plugins). `t_browserplugin` now checks a 2 MB module and runs with a 240 s limit in `run.sh`.
  - An unchecked wait hides the real failure three steps later: make the wait a check that prints the page's note.
  - Writing the manual found real bugs: have the next docs pass list what it finds unclear or broken, with file:line.
  - Staging only some hunks (git diff -U0 of the file, the hunks you want, git apply --cached --unidiff-zero) keeps commits to one topic
    when a file holds several.

## Report when you stop

1. Update the status table in `docs/release/release-plan.md` and push as above.
2. Rewrite this prompt for the next run.
3. Tell the owner: what changed (rows Done), the ledger counts, the questions waiting on them (each with your
   recommendation), the rows left, and the CI result. Then stop.
