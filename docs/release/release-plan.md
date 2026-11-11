# Release plan: from step 5.10 to v1.0.0

Nov 11, 2026. What stands between `main` and a release people can install, and the order the work runs in. It follows
`docs/roadmap.md` (which got the app to feature-complete as far as its own accounts go) and keeps its rules: one agent at
a time, a step runs from a clean `main` to a clean `main`, tests and docs with every change, decisions belong to the owner.
The hard requirements in `CLAUDE.md` still bind. Nothing here cuts scope: where the design promises a thing that isn't
built, a step builds it.

## Where things stand (checked Nov 11)

- Steps 4.1 to 5.10 are built. 269 Rust tests pass, clippy and `just lint` are clean, `just gate5` passes on the Mac.
- CI: run 43073397717 on `d45727c`. Linux passed everything but `t_live` ("Stop ends the recording"), a known Linux flake;
  macOS passed on the previous two runs; Windows had two fixes pushed (the Python venv, then an extension link error
  fixed by `opt-level = 2` in `python/Cargo.toml`) and its result is pending. The roadmap's 5.10 row still says
  "pushed and CI-green is still to do".
- The documents disagree about what is done. The feature matrix (204 rows, Nov 8) still marks Events ✗ though 5.2 is
  built; the parity audit counts 283 rows with 251 ported and 20 partial; the roadmap's status table says "built" for
  several steps whose notes say the app-parity harness never ran on them. Step R1 settles this.
- Parity: `just app-parity` reports 214 differences over 23 scenes; most are scene-state mismatches or the app having
  more menu items, but the audit ranks real ones (menus, Explorer, pane geometry, tables).
- Open follow-ups, all v1 scope: Light mode's raw windows and trimmed exports in the viewer, several files in one
  export, `--single-file`, a reduced viewer bundle, redaction inside checks, SVG and code widgets in exports, check
  report links, browser-only mode's worker and formats, Python wheels and the missing calls, time going backwards in
  fixed-width, packet and plugin decoders, an unmeasured Canvas 2D renderer, a static-host recipe.
- **Nothing for release exists yet:** `bundle.active` is `false` in `app/src-tauri/tauri.conf.json`; there is no release
  workflow in `.github/workflows/ci.yml`; no installer, signing or notarisation; no `LICENSE` (the crates say
  `UNLICENSED`, `publish = false`); version is 0.1.0 everywhere; no CHANGELOG, CONTRIBUTING or SECURITY file; no Help
  menu or manual in the app; no user manual or developer guide (the README, `docs/development.md` and `plugins/README.md`
  are the only guides, and the README doubles as a progress report).

## Decisions the owner needs to make first

Each step below says which of these it needs. Recommendations are mine; none is built until the owner answers.

| # | Decision | Recommendation |
| --- | --- | --- |
| 1 | Licence for the code, and for the manual and screenshots | Apache-2.0 for code (patent grant, plugins and bindings are meant to be extended); CC BY 4.0 for the manual. The owner's call; it changes every `Cargo.toml`, `pyproject.toml` and the README |
| 2 | Platforms and architectures to ship | macOS arm64 and x86_64 (universal `.dmg`), Windows x64 (installer + portable zip), Linux x64 (AppImage and `.deb`; `.rpm` if wanted), Linux arm64 for the CLI only. Windows arm64 only if a runner is available |
| 3 | Signing: an Apple Developer ID and notarisation; a Windows code-signing certificate (or Azure Trusted Signing) | Sign both for the release. Unsigned builds trigger Gatekeeper and SmartScreen warnings that make a "production" release look broken. The owner holds the credentials and adds them as repository secrets; an agent never sees them |
| 4 | Where installers are published | GitHub Releases on the repo, built by a tag-triggered workflow; the same workflow publishes the Python wheels and attaches the CLI archives |
| 5 | PyPI: the project name (`tlmbench` is what `python/` uses), a trusted-publisher setup | Register the name now; publish with PyPI trusted publishing (no stored token) |
| 6 | How Help opens the manual in the app | Open the bundled PDF with the OS's default viewer through Tauri's opener (the app doesn't run a program of its own: it asks the OS to open a document), plus a "Online manual" link. The alternative is a manual window inside the app, which needs the manual as HTML too |
| 7 | Where the web copy of the manual lives | GitHub Pages from the repo (`docs/site`), next to the browser-only app and a sample HTML export |
| 8 | What happens to the plan documents (the list in R8) | Delete them from `main` once their content is absorbed (git history keeps them); keep the prototype under `prototype/` as the design reference and its tests as they are |
| 9 | Version and tag | `1.0.0`, tag `v1.0.0`; a `v1.0.0-rc.N` tag series for the dry runs |
| 10 | Homebrew tap, winget, Flathub, crates.io | Not for v1.0.0; a Homebrew cask is the first to add after |

## The steps

Sizes as in the roadmap: S a day or two, M a few days, L a week or more. One agent per step; R2 and R3 are split into
sub-steps that run one after another. Every step ends with `ci/ledger_check.py` passing (after R1), CI green, and
`docs/roadmap.md`'s status table updated.

### R0 Close out 5.10 (S, in progress)

Get CI green on `main` (Windows' extension link, Linux's `t_live` flake), then update the roadmap's 5.10 row.
Because releases will be built by a workflow, a test that needs "rerun failed jobs" is a release blocker:
**de-flake `t_checks`, `t_live`, `t_folder`, `t_trigger` and `t_windows` on Linux** (find the race, not a retry; a
retry wrapper is not a fix). Done when two consecutive full CI runs pass with no rerun.

### R1 Review and the ledger (M) — prompt in `docs/release/review-prompt.md`

Check every roadmap step and every Accepted or Modified design decision against evidence; produce
`docs/release/ledger.md`, `docs/release/review.md` and `ci/ledger_check.py`. **This is the strict confirmation:** from here
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
- **R2.2 Browser-only mode (M).** Parse in a Worker (a big CSV must not freeze the tab); read Parquet, `.tlm`, packets
  and descriptors (make `tlmbench-decode` build for WASM instead of including three of its files as source); derived
  channels, events, checks and several files as the design promises for it, which needs the store for WASM (shared with
  R2.1's decision); a reload remembers which files were open. A static-host recipe (GitHub Pages and a release asset)
  for `app/browser/dist`, run by the release workflow.
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

### R3 App parity with the prototype (XL, sub-steps by area)

The target is not zero harness differences (many are scene state), it is **every ledger row Done and every visible
difference either fixed or in the owner's accepted-differences list**. Order by what the owner sees:

- **R3.1 Menus and shell (M).** The tab menu's content (Save layout ⌘S, Revert changes, numbered file chip), item icons,
  separators and group headings a pixel off; the new-tab menu; widths and padding throughout; pane geometry (204 against
  222 px; the overview's pane picker; time range text); the channel tick.
- **R3.2 Explorer, start page, dialogs (M).** Unit and pane badge shown only on hover (as the prototype); the heading row;
  Explorer multi-select of categories, Enter, categories from descriptors; the start page's recent sessions and saved
  sources; Connect's field widths and defaults; the drop-a-file flow.
- **R3.3 Panes, charts and tables (M).** Dragging a channel to pane edges and between rows; event markers' tooltips;
  the table's status line and Event column; slower channels held and dimmed; Stop at several categories; the Recipes and
  profiles manager's profiles tab; Folder runs (several levels, trends tab); This tab shows.
- **R3.4 What the audit can't see (S).** Quiet interface; keyboard coverage compared with the prototype's list; every
  tooltip and empty state; copy review (plain, short, sentence case); the design's motion rules.
- **After each sub-step:** rerun `just app-parity`, update the audit, and screenshot with `node app/ui/tests/shots.js`.

The UI freezes at the end of R3: after it only bug fixes touch `app/ui/src`.

### R4 Packaging and release automation (L)

Needs decisions 1 to 5 and 9. Built before the docs so that every platform's installer exists for R5's real-window passes.

- **Bundles:** turn `bundle.active` on; targets per decision 2 (macOS `.app` and `.dmg`, Windows NSIS and/or MSI, Linux
  AppImage and `.deb`); icons (present); the bundle ships the CLI next to the app (or the app can install its
  command-line link: decide with the owner), the plugin templates, the example `.tlmbench/` and the manual PDF as a
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
checklist (R1's Owner pass rows, the audit's real-window list, the first-run flow from the installer), the owner runs it on
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

- Delete: `docs/next-agent-5.8.md`, `docs/roadmap.md`, `docs/feature-matrix.md`, `docs/parity-audit.md`,
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

R0 → R1 → R2.1 → R2.2 → R2.3 → R2.4 → R2.5 → R3.1 → R3.2 → R3.3 → R3.4 → R4 → R5 → R6 → R7 → R8 → R9.

- R1 comes first because its ledger defines R2 and R3 (the lists above are what the follow-ups and the audit say today;
  the ledger will add and remove rows, and it wins).
- R2.1 and R2.2 share one decision (the store for WASM), so the owner decides it once, before R2.1.
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
