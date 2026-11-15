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
- `docs/release/owner-test-sheet.md` holds only what the owner has still to try; `docs/release/ui-audits.md` the audits'
  findings; `docs/follow-ups.md` the design-doc notes owed; `docs/post-v1.md` what waits for after the release.
- `docs/roadmap.md`, `docs/feature-matrix.md`, `docs/parity-audit.md` and `docs/release/review.md` are history (Nov 11 and
  earlier). Don't update them; the ledger and the R3.0 classification replace them.

**A step is done when** its ledger rows are Done (or Owner pass with what to look at), `python3 ci/ledger_check.py` passes,
the owner has been asked to push and has, **CI is green on the pushed commit with no rerun**, and the status table says so
with the commit and the run. An agent never pushes without the owner's go-ahead (CLAUDE.md), so "built, not pushed" is a
state of its own in the table, not "done".

## Status (Nov 15, 2026)

| Step | State |
| --- | --- |
| R0 Close out 5.10 | **First clean run Nov 15:** run 43738929926 on `cb228f2`, all four jobs green on the first attempt (Linux, macOS, Windows, the image), after three fixes the runs since Nov 11 found: the check report's export test needed the viewer (`ci-linux` builds it first now), `t_workspaces` pressed ⌘ where Linux uses Ctrl, and the replay tests timed a busy runner's file open. **Done when the next run is clean too** (two in a row, no rerun) |
| R1 Review and the ledger | Done Nov 11 (`ledger.md`, `review.md`, `ci/ledger_check.py`) |
| R2.1 Exports | Built Nov 11 (ledger rows naming R2.1; `docs/exports.md`, `docs/viewer-store.md`); CI with the Nov 15 push |
| R2.1a The owner's first pass | Built Nov 14 and 15: the owner's 11 fixes, all nine decisions (owner-decisions memory, `follow-ups.md`), the sheet's R3 (Reset app), R12 (the viewer Locked, with Unlock) and B4 (`TLMBENCH_SLOW_BUILD`), the first UI audit. **The owner's re-test is open**: R12, B4, S1 to S4 and the older untested items on the sheet |
| R3.0 Classification audit (new, moved up) | **Done Nov 15** (`parity-classes.md`: 11 Port rows, 16 App is better, the rest Noise; apple-design audit in `ui-audits.md`; ledger D-ui-36 to D-ui-41, D-live-11). The owner approved every "App is better" row Nov 15 (ledger A-05 to A-11). The owner decided P3 (trim the ⋯ menu) and P10 (a fixed-width time range) as recommended, Nov 15 |
| R2.2 Browser-only mode | **Next.** Half built (D-brow-01 to -07: Worker, reload, static-host recipe, a tab offering only what it can do, `.tlm`, Parquet). Left, by the owner's scope of Nov 15: descriptors and packets, WASM decoder plugins, read-only folders, leaving out the post-v1 controls (D-brow-05, -08, -09, -10), then the end-of-R2.2 UI audit |
| R2.3 Decoders · R2.4 Python · R2.5 Core's Partial rows | Not started |
| R3.1 to R3.4 Parity | Not started; their lists come from R3.0 |
| R4 to R9 | Not started. Release decisions 1 to 5 and 9 decided Nov 15; 6, 7, 8 and 10 open (needed by R7 and R8) |
| CI on the Nov 15 push | Green: run 43738929926 (`cb228f2`). The push after it (docs only) is R0's second run |

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
| R7 Documentation | Sonnet | The manual and guides from the design, the ledger and the app; generated reference pages and screenshot runs: Haiku |
| R8 Cleanup | Haiku | Deleting the agreed list and pruning; dead code and dependency checks: Sonnet |
| R9 Release candidate and release | Sonnet | Running the checklist; Opus for an RC bug that blocks the tag |
| Status table, ledger counts, test sheet upkeep | Haiku | Bookkeeping, as part of any step |

## Where things stand (checked Nov 15)

- Ledger: Done 235 · Partial 47 · Missing 24 · Diverged 4 · Unverified 26 · Owner pass 17 (353 rows).
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

Each step below says which of these it needs. **Decided Nov 15 by the owner, as recommended: 1, 2, 3, 4, 5 and 9**, and the
CLI ships beside the app (R4). Still open: 6, 7, 8 (partly: the plan documents were reduced Nov 15; the final deletion
list is agreed at R8) and 10. Nothing open is built until the owner answers.

| # | Decision | Recommendation (and state) |
| --- | --- | --- |
| 1 | Licence for the code, and for the manual and screenshots | Apache-2.0 for code (patent grant, plugins and bindings are meant to be extended); CC BY 4.0 for the manual. The owner's call; it changes every `Cargo.toml`, `pyproject.toml` and the README **Decided Nov 15.** |
| 2 | Platforms and architectures to ship | macOS arm64 and x86_64 (universal `.dmg`), Windows x64 (installer + portable zip), Linux x64 (AppImage and `.deb`; `.rpm` if wanted), Linux arm64 for the CLI only. Windows arm64 only if a runner is available **Decided Nov 15.** |
| 3 | Signing: an Apple Developer ID and notarisation; a Windows code-signing certificate (or Azure Trusted Signing) | Sign both for the release. Unsigned builds trigger Gatekeeper and SmartScreen warnings that make a "production" release look broken. The owner holds the credentials and adds them as repository secrets; an agent never sees them **Decided Nov 15.** |
| 4 | Where installers are published | GitHub Releases on the repo, built by a tag-triggered workflow; the same workflow publishes the Python wheels and attaches the CLI archives **Decided Nov 15.** |
| 5 | PyPI: the project name (`tlmbench` is what `python/` uses), a trusted-publisher setup | Register the name now; publish with PyPI trusted publishing (no stored token) **Decided Nov 15.** |
| 6 | How Help opens the manual in the app | Open the bundled PDF with the OS's default viewer through Tauri's opener (the app doesn't run a program of its own: it asks the OS to open a document), plus a "Online manual" link. The alternative is a manual window inside the app, which needs the manual as HTML too |
| 7 | Where the web copy of the manual lives | GitHub Pages from the repo (`docs/site`), next to the browser-only app and a sample HTML export |
| 8 | What happens to the plan documents (the list in R8) | Delete them from `main` once their content is absorbed (git history keeps them); keep the prototype under `prototype/` as the design reference and its tests as they are |
| 9 | Version and tag | `1.0.0`, tag `v1.0.0`; a `v1.0.0-rc.N` tag series for the dry runs **Decided Nov 15.** |
| 10 | Homebrew tap, winget, Flathub, crates.io | Not for v1.0.0; a Homebrew cask is the first to add after |

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
- **R3.4 What the audit can't see (S).** Quiet interface; keyboard coverage compared with the prototype's list; every
  tooltip and empty state; copy review (plain, short, sentence case); the design's motion rules.
- **After each sub-step:** rerun `just app-parity`, update `parity-classes.md`, screenshot with `node app/ui/tests/shots.js`,
  and run the apple-design audit on that sub-step's area.

The UI freezes at the end of R3: after it only bug fixes touch `app/ui/src`. A full apple-design audit runs before the freeze.

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
  clean machine or VM, and the notarised `.dmg` opens with no Gatekeeper warning.

### R5 Real-window passes and fixes (L, with the owner)

Headless tests cannot see Finder drops, native dialogs, WKWebView or WebView2 or WebKitGTK text and fonts, window
dragging between windows, GPU differences, the high-DPI look, or the installers. For each OS the agent hands the owner a
checklist (R1's Owner pass rows, the audit's real-window list, the first-run flow from the installer, and the apple-design
audit on that OS's installed build), the owner runs it on
the installed release candidate, and the agent fixes what they list, ranked. Needs: a Windows and a Linux machine or VM
the owner can use (ask early; if the owner has none, say which checks stay unverified and note it in the release notes
rather than claim them). Include WebGL2 absent or software-only (the Canvas 2D renderer) and a read-only network share.
Done when each OS's checklist is signed off in `docs/release/real-window-passes.md` (kept until R8, then summarised).

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

R0 → R1 → R2.1 → R2.1a → **R3.0** → R2.2 → R2.3 → R2.4 → R2.5 → R3.1 → R3.2 → R3.3 → R3.4 → R4 → R5 → R6 → R7 → R8 → R9.

R0 closes with the first two clean CI runs; R2.1a closes with the owner's re-test, which can run alongside the steps after it.

- R1 comes first because its ledger defines R2 and R3 (the lists above are what the follow-ups and the audit say today;
  the ledger will add and remove rows, and it wins).
- R2.1 settled the store for WASM (it builds behind a `native` feature; the viewer's queries stay in `crates/web`).
- R3.0 runs before the rest of R2 because it is read-only and its "App is better" list protects work R2 might touch.
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
