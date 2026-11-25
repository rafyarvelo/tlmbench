You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to v1.0.0.
One agent works at a time: start from a clean `main`, leave a clean `main`. If another Claude session is running in this
repository, agree with it which files each of you touches before you edit.

This prompt is only a hand-off. **Where each step stands is the status table in `docs/release/release-plan.md`; what is
done is `docs/release/ledger.md`.** If this prompt and those disagree, they win, and you fix this prompt.

Written Nov 25 after the Opus run that did R6's gates and the soak, tagged `v1.0.0-rc.1` (owner's OK), and started
coverage (R6c); a Sonnet run then finished most of decode (`00a1f9d`). **Recommended model for the next run: Sonnet**, for R6c.3 below (tests for gaps listed in writing,
crate by crate). Switch to **Opus** when the owner hands back a test pass with Fs to fix, when a coverage test finds a
real bug that needs a design judgement, or for R6c.5 (the coverage CI job). The plan's "Recommended model per step"
table has the same.

## First

1. Read `CLAUDE.md`, then the status table in `docs/release/release-plan.md` and the memory files in
   `/Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/`.
2. `python3 ci/ledger_check.py`: Done 345, Partial 3, Missing 2, Diverged 1, Unverified 19, Owner pass 18 (388 rows)
   at the hand-off.
3. `git log origin/main..main`: only this hand-off's docs commit should be unpushed; it goes up with your first code push.
4. CI was green on `00a1f9d` (run 45670948681; wheels 45670948792).

## Where things stand

- **rc.1** is a draft pre-release on GitHub (release run 45478226369): every installer built and passed install-and-smoke
  on its OS; the draft was checked (checksums, the manual and both licence files inside the app). The owner is testing
  it on macOS now (`docs/release/test-plan-macos.md`, 18 results so far). No Windows or Linux machine yet.
- **Gates and soak:** all pass on the release build of `63360a1` (`docs/developers/gates.md`, "1.0.0-rc.1").
- **Licences (owner, Nov 24):** the code MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`), the plugin templates MIT-0,
  the manual CC BY 4.0.
- **Coverage (R6c):** Rust 79.0% of lines on `00a1f9d` (decode 96.3%, live 93.8%, store 89.4%, core 94.3%, expr 95.3%,
  export 88.5%, plugin 81.4%, cli 79.8%, app 27.6%, tools 0%); the UI 91.7% of lines, 75.9% of branches (13
  self-launching tests not counted yet). Two bugs found so far, both fixed with tests: a packet index evicted on a
  nearly full disk (`644df60`), and a CSV profile's time column ignoring its header's unit (`868883c`); decode's second
  pass found none.

## The next run: R6c.3, coverage crate by crate (Sonnet)

`docs/release/release-plan.md`, "R6c Coverage", has the goal (as close to 100% as can honestly be reached; the owner's,
Nov 24), the sub-steps and the rules; `docs/developers/coverage.md` has how to measure, the figures, what may stay
uncovered (each with a reason) and **decode's remaining list**. Work in this order, one crate per run or less, and stop
where your context or the clock says:

1. **Finish decode** from coverage.md's list (`parquet_pages.rs` at 73%, then the error branches of the other files,
   each a test or a written reason; decide whether `crates/web-parquet`'s own tests are measured). Short: do it in
   under an hour and go on.
2. **live** (93.8%), then **store** (89.4%, `joined.rs` 57%, `config_core.rs` 75%), then export, core, expr, cli, plugin, app/src-tauri (its commands as functions, or
   `tauri::test`), tools/gen. For each: measure, list every uncovered range (a segment of `cargo llvm-cov report --json`
   with a zero count, its `hasCount` set and not a gap region), read each, then a test or a written reason in
   coverage.md.

How to work (each lesson was learned the hard way):

- A test checks behaviour, never just reaches lines. Before trusting a new test of a tricky path, break the code for a
  moment (comment the line out, run, see it fail, restore and `git diff` to confirm only the test changed).
- When a test fails, decide whether the test or the code is wrong before changing either. Two of R6c's failures were
  real bugs; most were the test's own setup (a 16-bit length too small for the payload, summaries that must start on a
  256-sample boundary, a validation that runs before the check you meant to reach). Read the code path first.
- A test that depends on timing waits on state (`health().records_written`, a tab or a note appearing), never a fixed
  pace or sleep: CI's runners are loaded, and three of the Nov 24–25 CI failures were tests timed for this Mac.
- A real bug a test finds: fix it with that test, its own commit, and say in the commit what users saw. If the design
  says otherwise or is silent, stop and ask the owner.
- Remeasure after each crate and write the figures (and commit) into coverage.md's table and the status table's R6c row.
- `decode` and `store` changes rebuild most of the workspace (20–40 minutes); batch a crate's tests before running them.
  `crates/web` and `crates/web-parquet` include some of `decode`'s files as source (csv, descriptor, packets_core,
  backwards, preview, profile, reference, strided, error, parquet_core): `just lint` checks their WASM builds.

R6c.4 (the UI file by file; first bring the 13 tests that launch their own browser into the `TLM_JS_COVERAGE` hook)
and R6c.5 (the CI job: recommended shape in the plan; **the owner OKs its first version**) come after.

## When the owner hands back a test pass (Opus)

The owner fills in the per-OS copies of `docs/release/test-plan.md` (`test-plan-macos.md`, `-windows`, `-linux`).
**Change the plan, never a copy, then run `python3 tools/testplan.py`**: it rewrites the copies with only their own
system's items and keeps every result in them (`--check` says whether they're current). Don't commit the owner's
results without saying so: on Nov 24 a docs commit swept 17 of them in unnoticed.

Notes the owner has written so far on macOS, to look at when the pass comes back (not before, unless asked): 1.9, the
Install command-line tool dialog closed before the line could be copied (the owner suggests a modal); the shared
folder's Rules & Checks looked empty; where the flight events "Takeoff" and "Hard turn" come from (they're `make.py`'s
`flights/.tlmbench/events/flight-1.toml`: the plan should say so); 3.4, Loop A–B only working after a pause.

Then: rank the Fs (crashes, data wrong or lost, anything written next to the data; then features; then looks and
copy), fix each with a test and its ledger row, retake any manual screenshot a fix changes (`node
app/ui/tests/shots.js OUT --manual id`, an output folder, never `docs/images` unasked), rerun the gates a fix could
touch on the final commit's release build, bump to `1.0.0-rc.2` (`tools/check_versions.py --tag v1.0.0-rc.2`;
`cargo update --workspace --offline` in the workspace, `crates/web`, `crates/web-parquet` and `python`), add its
CHANGELOG section, push, CI to green, and **ask the owner before the tag**. At 1.0.0's tag: R8's delete list
(`follow-ups.md`, `ui-audits.md`, `release-plan.md`, `next-agent-prompt.md`, `security-review.md`, the ledger as the
guide's appendix).

## Open with the owner

- Test-plan 4.7 (Quiet interface's default, yes or no) and 1.2 (which first-open way): the owner writes them in.
- No Windows or Linux machine yet: if none by 1.0.0, the notes say which checks stay unverified (rc.1's do).
- The Windows CI cache: if Windows fails at "Restore cache" with "No space left on device", ask before changing the
  workflow (recommendation: delete the cache).
- PyPI's trusted publisher and SignPath (the owner's).
- R6's measurements no gate takes (ledger rows naming R6: table scroll, 100 widgets, idle RAM, live latency, the CLI on
  10 and 100 GB, and others): still Unverified; they don't block a candidate.

## Rules

- **Owner decisions are the owner's.** Never propose cutting scope; post-v1 ideas go to `docs/post-v1.md`. The UI is
  frozen: only bug fixes touch `app/ui/src`, each with a test. Don't touch the design doc (synced Nov 24); notes go to
  `docs/follow-ups.md`.
- **Done means, for each row:** built, a test for it, the ledger row moved with evidence and `python3 ci/ledger_check.py`
  passing with its counts line updated, `docs/follow-ups.md` where behaviour differs from the design, a test-plan item
  when only a real window shows it. Small commits, one topic each.
- **Push** only with code commits, once `just lint`, `cargo nextest run --workspace`, the Python tests (`unset
  CONDA_PREFIX`, `maturin develop`, `pytest python`), `just app-test` and `just proto-test` pass locally (the UI suites
  when the UI or its tests changed, and never while cargo builds). Then watch CI to green (about 1.5 hours). Docs-only
  commits never get a push or a CI watch of their own. **Don't change `ci/` or the CI image.**
- **Memory and shell:** the Mac has 8 GB: `CARGO_BUILD_JOBS=2` for every cargo build (lint, tests, coverage, maturin);
  `sysctl vm.swapusage` before a big build, and if swap is over 4 GB wait or report; never start a cargo command while
  one of yours runs (a command that moves to the background will notify you). The shell's PATH lacks Homebrew and
  cargo: start commands with `export PATH="/opt/homebrew/bin:$HOME/.cargo/bin:$PATH"` (`gh`, `just`, `cmake`: hdf5's
  build fails without cmake). A stale `target/llvm-cov-target` from an interrupted run breaks hdf5's CMake: delete it.
- **Disk** (about 24 GB free): a workspace test build is ~16 GB in `target/debug`, a coverage build ~17
  GB in `target/llvm-cov-target`, the release build ~13 GB; the disk filled twice on Nov 24–25 (a failed `maturin
  develop`, and two test links failing at once). `df -h /` before each build; under ~12 GB free delete what you don't
  need (`target/debug`, `python/target`, `target/release`: the owner OKs it). Cargo workspace-wide only, never `-p`.
  `CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0` for workspace runs. Delete `target/debug` when you're done.
- **Gates** drive the release window: keep it visible, run under `caffeinate -dims` (a run hung when the Mac slept). A
  gate that fails is fixed with a test, never loosened.
- **CI:** GitHub's macOS arm64 runners are sometimes unavailable ("not acquired by Runner of type hosted"): the job is
  cancelled before a step runs; rerun the failed jobs once the run ends. A macOS link error ("Undefined symbols",
  `dc98540`) happened once and didn't come back; if it does twice, ask the owner about `CARGO_INCREMENTAL=0` for that job.
- **Lessons** (older ones are in this file's git history): browser-only tests that drop a file at start-up wait for
  `__tlmApp.dropReady()`; a double-click just after Explorer's refresh can be lost, so a test opens again until a tab
  or a note shows; in zsh `kill $pids` with several PIDs fails (use `kill ${=pids}`); a scratchpad script needs
  `chmod +x` or `bash script`; never run the UI suite while cargo builds; `shots.js` with no argument writes into
  `docs/images`; known load flakes `t_browser`, `t_descriptor`, `t_windows` (pass alone); this Mac's shell has no `jq`
  (use `gh … --jq`); a Monitor command must not contain the text it waits for; never use `window.prompt` or a
  synchronous `confirm`; after a CLI option, settings key, expression function or menu command changes, `just
  reference`; macOS `sed -i ''`; never `git stash`; add files by name and check `git show --stat HEAD`; end commits with
  the attribution line your session's instructions give.

## Report when you stop

1. Update the status table in `docs/release/release-plan.md` (R6c's row: what you covered, the figures, the commit) and
   push as above.
2. Rewrite this prompt for the next run, with your recommendation of the model for it.
3. Tell the owner: what changed, any bug a test found, the ledger counts, questions waiting on them (each with your
   recommendation), what is left, and the CI result. Then stop.
