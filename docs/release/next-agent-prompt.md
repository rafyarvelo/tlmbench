You are continuing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, on the way to v1.0.0.
One agent works at a time: start from a clean `main`, leave a clean `main`. If another Claude session is running in this
repository, agree with it which files each of you touches before you edit.

This prompt is only a hand-off. **Where each step stands is the status table in `docs/release/release-plan.md`; what is
done is `docs/release/ledger.md`.** If this prompt and those disagree, they win, and you fix this prompt.

Written Nov 24 after the Opus run that did R6's gates on the final commit and readied the release candidate (status
table rows R6, RC and CI). **The next run starts after the owner's test pass on `v1.0.0-rc.1`: fix the owner's F results,
then `rc.2`, and so on to 1.0.0.**

## Where things stand

- **Every gate passes on the release build of `63360a1`**, and the hour-long soak at 4M samples/s
  (`docs/developers/gates.md`, "1.0.0-rc.1"; the soak recorded to an external drive through `TMPDIR`, see there).
- **Licences (owner, Nov 24):** the code is MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`), the plugin templates
  MIT-0, the manual CC BY 4.0.
- **The tag:** `v1.0.0-rc.1` is pushed only with the owner's OK (it is the outward step). The release workflow then
  builds a draft pre-release: the macOS universal `.dmg`, Windows NSIS setup, MSI and portable zip, Linux AppImage and
  `.deb`, the CLI archives (five platforms), the Docker image, the browser-only site, wheels (not published to PyPI for
  a candidate), SHA256SUMS and attestations. The manual's PDF, LICENSE and THIRD_PARTY_LICENSES.md are inside each app
  bundle (`tauri.release.conf.json` resources) and the Windows portable zip; each CLI archive has LICENSE and
  THIRD_PARTY_LICENSES.md (not the manual). Neither licence file is attached to the release on its own. **Check the draft has every file before telling the
  owner it is ready**; the MSI's version for a candidate is `1.0.0.N` (`8489939`), which has never run on a runner yet.
- **R6's measurements that no gate takes** are still Unverified in the ledger (rows naming R6): benchmark history,
  table scroll at 60 fps, 100 widgets under 2 ms in the release app, the Canvas 2D budget, idle RAM, rules' cost, live
  latency, the CLI on 10 and 100 GB files, a slow live client, golden images and golden CLI output, gate 5 from a remote
  static host. They don't block the candidate; do them while the owner tests, one ledger row per commit.

## Coverage (R6c, while the owner tests)

The owner's goal (Nov 24): coverage as close to 100% as possible, Rust and the UI, and measured in CI.
`docs/release/release-plan.md`, "R6c Coverage", splits it into sub-steps R6c.1–R6c.5 (one crate or area a run, so
agents can hand it off); the status table's R6c row says where the last run stopped, with the numbers and commit.
Tests only: a test that finds a bug fixes it. What can't be covered is written down with its reason in
`docs/developers/coverage.md`. The CI job (R6c.5) waits for the owner's OK of its first version.

## The owner's test pass (what to do with it)

The owner runs `docs/release/test-plan.md` once per OS on rc.1's installers (data from `tools/testdata/make.py`) and
writes P or F per item. Then:

1. Rank the Fs: crashes, data wrong or lost, and anything that writes next to the data first; then a feature that
   doesn't work; then looks and copy. Ask the owner only where an F is a design question (give a recommendation).
2. Fix each F with a test (a UI test with the fake core where it can show it, a Rust test for the core), and move or add
   its ledger row. A fix that changes a manual screenshot: retake it with `node app/ui/tests/shots.js OUT --manual id`
   (an output folder, never `docs/images` unasked), check it, then copy it in.
3. Rerun the gates a fix could touch (all of them if it touches `decode`, `store` or the live path) on the release build
   of the final commit, and record them in `gates.md` with that commit.
4. Bump to `1.0.0-rc.2` (`tools/check_versions.py --tag v1.0.0-rc.2` must pass; `cargo update --workspace --offline` in
   the workspace, `crates/web`, `crates/web-parquet` and `python`), add its CHANGELOG section (the draft's notes come
   from `## 1.0.0-rc.2`), push, watch CI to green, and ask the owner before the tag. The owner retests only the items
   that were F and what a fix touched.
5. When a candidate has no F left: 1.0.0 (version `1.0.0`, the CHANGELOG's `## 1.0.0` section dated), the owner's OK for
   the tag; PyPI publishes on a final tag once the owner has set up the trusted publisher. **At the tag, R8's delete
   list** (agreed Nov 23): `follow-ups.md`, `ui-audits.md`, `release-plan.md`, `next-agent-prompt.md`,
   `security-review.md` (summarised in SECURITY.md) and the ledger (as the developers guide's appendix).

## Open with the owner

- **Quiet interface's default** (W3): item 4.7 of the test plan.
- **Z3's first-open path:** item 1.2 of the test plan; `docs/install.md` gives both.
- **R5's machines:** no Windows or Linux machine yet (Nov 23). If none by 1.0.0, the release notes say which checks stay
  unverified there (rc.1's CHANGELOG entry already does).
- **The Windows CI cache:** if Windows fails at "Restore cache" with "No space left on device", ask before changing the
  workflow (recommendation: delete the cache again).
- PyPI's trusted publisher and SignPath (the owner's), as before.

## The UI is frozen (Nov 22)

Only bug fixes touch `app/ui/src`, each with a test. A new idea goes to `docs/post-v1.md`. Don't touch the design doc
(synced Nov 24); notes go to `docs/follow-ups.md`.

## Rules

- **Owner decisions are the owner's.** Never propose cutting scope. "App is better" rows are never ported back.
- **Done means, for each row:** built, a test for it (and the tests README's table), the ledger row moved with evidence and
  `python3 ci/ledger_check.py` passing with its counts line updated, `docs/follow-ups.md` where behaviour differs from the
  design, and a test-plan item when only a real window shows it. Small commits, one row or topic each.
- **Push** only with code commits, once `just lint`, `cargo nextest run --workspace`, the Python tests (`maturin develop`
  then `pytest python`), `just app-test` and `just proto-test` pass locally. Then watch CI to green (about 1.5 hours;
  don't push per row). Docs-only commits never get a push or a CI watch of their own. **Don't change `ci/` or the CI image.**
- **Disk:** the Mac has 8 GB of memory and about 24 GB free. A workspace test build fills `target/debug` to about 16 GB,
  and the Python extension's own target adds 2 GB: `rm -rf target/debug python/target` once they are done (the disk
  filled on Nov 24 during `maturin develop`). Under about 12 GB free, delete `target/debug` first. Cargo workspace-wide
  only, never `-p` (the release recipes in the `justfile` are the exception). `CARGO_PROFILE_DEV_DEBUG=0
  CARGO_INCREMENTAL=0` keeps a workspace run near 8 GB of memory.
- **Gates:** they drive the release window; keep it visible. Run them under `caffeinate -dims` (a run on Nov 24 hung
  when the Mac slept with the Gate 1 window open; `just gate` holds only the display awake). `just gate2` and
  `just gate4 app` rebuild the app (about 10 minutes each) because the CLI's release build has another feature set.
  A gate that fails is fixed with a test, never loosened.
- **Lessons (Nov 24, this run):**
  - The cache's eviction runs when free disk falls under 10 GB and removes every entry no process holds: a packet index
    or plugin decode now names its source in its `opened` marker so it stays while that file is open. On a nearly full
    disk, expect cache entries of closed files to go (by design).
  - `tauri-bundler` 2.10.4 refuses a non-numeric pre-release for the MSI; the release workflow passes
    `bundle.windows.wix.version` for a candidate. NSIS writes `1.0.0.0` by itself.
  - A script in the scratchpad needs `chmod +x` or `bash script`: the soak's memory sampler didn't start the first time.
  - `tools/testdata/make.py --help` used to write the whole set into `./--help` (fixed).
  - Browser-only tests that drop a file at start-up wait for `__tlmApp.dropReady()`: the app's drop listener registers
    asynchronously, and `t_browserpayload` lost its drop on a loaded Linux runner (a5e87b4). Check a new test the same way.
  - A live test that must not drop records waits on the recorder (`src.health().records_written`), never on a fixed
    pace: a loaded CI runner falls behind and a full ring drops records by design (7b1d373).
  - In zsh, `kill $pids` with several PIDs in one variable fails (no word splitting): list them, or use `kill ${=pids}`.
  - GitHub's macOS arm64 runners can be unavailable ("not acquired by Runner of type hosted"): the job is cancelled
    before a step runs. That isn't a failure of ours; rerun the failed jobs once the run ends.
- **Earlier lessons** (still true): never run the UI suite while cargo builds (the harness waits a fixed 600 ms, so boot
  fails); `node app/ui/tests/shots.js` with no argument writes into `docs/images`; known load flakes `t_browser`,
  `t_descriptor`, `t_windows`, `t_browserpayload`, `t_browserfolder` and the estimator test (pass alone); `maturin
  develop` needs `unset CONDA_PREFIX`; this Mac's shell is bash 3 with no `jq` (use `gh … --jq`); a Monitor command must
  not contain the text it waits for; never use `window.prompt` or a synchronous `confirm` (`askText`, an awaited
  `confirm`); never spread a large array into arguments; after a CLI option, settings key, expression function or menu
  command changes, `just reference` and commit `docs/reference/`; in zsh write `"${var}:path"`; macOS `sed -i ''`; never
  `git stash`; add files by name and check `git show --stat HEAD` after a multi-file commit; end commits with the
  attribution line your session's instructions give.

## Report when you stop

1. Update the status table in `docs/release/release-plan.md` and push as above.
2. Rewrite this prompt for the next run.
3. Tell the owner: what changed, the ledger counts, the questions waiting on them (each with your recommendation), what
   is left, and the CI result. Then stop.
