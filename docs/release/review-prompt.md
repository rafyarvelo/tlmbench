# Prompt: review everything built against the roadmap (step R1)

Give this to the next agent as is. It is read-only on code: the output is documents. The release plan it feeds is
`docs/release/release-plan.md`.

---

You are reviewing TelemetryBench (tlmbench) in `/Users/rafy/workspace/tlmbench`, branch `main`, before its v1 release.
Every step in `docs/roadmap.md` (4.1 to 5.10) has been built and each agent that built one wrote its own account of
what is done. Your job is to check those accounts against the repository and say, with evidence, what is really done.
You change no code. You write `docs/release/review.md` and `docs/release/ledger.md` (below) and fix stale numbers in
the documents you check.

## Read first, in this order

1. `CLAUDE.md`. Its hard requirements bind you and everyone after you: the app works from its cache alone on a
   read-only folder; nothing is written next to data except by Share and exports; exports are static files; everything
   in the design is v1 and **you never propose cutting scope** (if something looks infeasible, say so with evidence
   and ask the owner); the app never runs Python or other external programs.
2. `docs/roadmap.md`: the steps, each with bullets and a "Done when", the accepted differences, the status table.
3. `docs/design.md`: the source of truth. Every decision is Proposed, Accepted, Modified or Rejected; Accepted and
   Modified are requirements. The roadmap does not cover every one of them, so check the design directly too.
4. `docs/feature-matrix.md`, `docs/parity-audit.md` and `docs/follow-ups.md`: what earlier agents say is left.
5. `docs/development.md`, `app/ui/tests/README.md`, `prototype/tests/README.md`: how to run things.
6. The memory files in `/Users/rafy/.claude/projects/-Users-rafy-workspace-tlmbench/memory/` (disk space, headless
   tests missing the real window, the viewer architecture).

## Method

**1. Build the list of requirements.** For every step 4.1 to 5.10, take each bullet and each sentence of its
"Done when", one row each. Add one row per Accepted or Modified decision in `docs/design.md` that no roadmap row
covers (search the decision lists under each section and Phase notes). Add one row per entry in the roadmap's
accepted-differences list (that is a fact to confirm, not a gap). Give every row an ID: `S5.3-07` for the seventh row of
step 5.3, `D-live-02` for a design decision. Keep the ID stable: later agents will cite it.

**2. Find the evidence for each row.** Evidence is something a person can re-run or look at:
- the code (`path:line`);
- a test (file and name) that you ran in this session and that passed. Running it is part of the evidence: a test
  that exists but doesn't exercise the behaviour does not count; read it;
- a screenshot (file path), or the output of a command (`just gate5`, `tlmbench …`) pasted with its date;
- for things only a real window shows (Finder drops, native dialogs, WKWebView text, window dragging, platform
  look), the verdict is **Owner pass**, never ✓. Headless tests can't see them (memory:
  `headless-tests-miss-real-window.md`).

**3. Give each row one verdict.**
- **Done**: evidence found and it covers the whole sentence of the requirement.
- **Partial**: some of it; say which part is missing.
- **Missing**: no code or no way to reach it from the UI or CLI as the design says.
- **Diverged**: built differently from the design or prototype and no accepted difference covers it.
- **Unverified**: the claim may be true but you found no evidence you could re-run.
- **Owner pass**: needs the owner's eyes in a real window (list what to look at).

"Built" and "works" in a status table are claims, not evidence. A budget in the design (a time, a size, a frame
rate) is Done only with a measurement on the release build; say what was measured, where and when.

**4. Check the agents' accounts for drift.** Known examples to start from, and look for more of the same kind:
- `docs/feature-matrix.md` says its totals are over 204 rows and Nov 8, and still lists Events as ✗ for step 5.2
  while the roadmap's status table says 5.2 is built; the parity audit counts 283 rows. Reconcile them into one
  list, and say which document is right row by row.
- Status rows say "`just app-parity` and CI have not run on it" for steps that have since been built on; check which
  of those were later run.
- Every "left" item in `docs/follow-ups.md` is either still a gap (a row in your ledger) or was closed (say where).
  There are 23 sections there; read all of them.
- The prototype is the reference for look and behaviour. For each prototype test in `prototype/tests/t_*.js`
  (29 files) find the app test that ports it (`app/ui/tests/t_*.js`, 88 files), or say none does.

**5. Run the verification you can, one thing at a time** (the Mac has 8 GB; never run two cargo builds or two UI
suites at once; check `df -h /` before big builds; see the working notes below):
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo nextest run --workspace --no-fail-fast`, `just lint`;
- the app UI suite (loop over `app/ui/tests/t_*.js`, one at a time), then the prototype suite (`just proto-test`);
  timing tests (`t_loading`, `t_perf_gauges`) alone at the end. Record each failure and whether it is a known flake;
- the gates that fit: `just gate4`, `just gate5`. Gates 1 to 3 and 4's big file need the 10 GB recordings; list them as
  not re-run unless you have the disk, and say when each last passed (the design's Roadmap section holds the dates);
- `just ro-test` (nothing written next to data);
- `just app-parity`: record the count and the three largest groups.

**6. Look for what the roadmap does not cover.** List, with paths: code, flags, commands or UI that exists but
appears in no roadmap row and no design decision (to decide if it belongs or is leftover); crates or scripts nothing
calls; dead settings; TODO and FIXME comments (`grep -rn "TODO\|FIXME\|XXX"` over source, not docs); `#[allow(...)]`
that hides something; tests that are skipped or always pass.

**7. Check the repository itself** for release: licence and metadata, version numbers (workspace 0.1.0, the Python
package, `tauri.conf.json`), `bundle.active` in `app/src-tauri/tauri.conf.json` (false today), the absence of any
release workflow in `.github/workflows/ci.yml`, files with no purpose any more (`docs/next-agent-5.8.md`, and look for
others), documents that describe the plan rather than the product. Do not delete anything: list it for step R8.

## Output

**`docs/release/ledger.md`**: one table, one row per requirement:

| ID | Source | Requirement (one sentence) | Verdict | Evidence | Gap and the step that closes it |

Sort by source order. Put counts per verdict at the top. A row's evidence must name a test or a command, not a person.
Also write `ci/ledger_check.py` (plain Python 3, no packages): it reads the ledger and fails if any row has no
verdict, if a Done row names no evidence, if an evidence path doesn't exist, or if a test named as evidence is absent
from `app/ui/tests/` or `prototype/tests/`. Later agents run it, and the release gate runs it.

**`docs/release/review.md`**: at most four pages, in this order:
1. The verdict counts and what they mean for the release, in plain words.
2. The gaps ranked by how visible they are to a user, each with its ledger ID.
3. Drift found in the documents, and what you corrected.
4. What is in the repo but not in the plan (step 6).
5. What you could not check and who can (the owner, or a platform you don't have).
6. The decisions the owner needs to make (their questions, each with a short recommendation).

Fix the feature matrix and the parity audit's counts where you can prove them; leave their wording alone otherwise.
Do not edit the roadmap's status table except to correct a claim you disproved, and say so in review.md.

## Rules

- Concise. The owner reads the top of review.md and the decision list; lead with what changed and what's left.
- Decisions belong to the owner: ask, with a recommendation, and don't build on a guess.
- The suite and gates are the evidence. If one fails and isn't a known flake, that is a row, not something to fix.
- Shell: write scripts with the Write tool and run them (the safety check rejects some heredocs); macOS
  `sed -i ''`; never `git stash`; `git add -A` sweeps untracked files, so check `git status` first; don't push or
  rewrite history; commit by topic, small, ending with
  `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` (use your own model's name if different).
- Disk: workspace-wide cargo only (`-p` subsets pile up multi-GB copies); `rm -rf target/debug` when `df` shows
  under 5 GB free; `cargo clean --manifest-path python/Cargo.toml` after a Python build. The owner has OK'd deleting
  caches and builds.
- UI tests run with `node tests/t_x.js` from `app/ui`, after `npx svelte-check --fail-on-warnings && npm run build`.
- Done when `ledger.md` has every row with a verdict and evidence, `ci/ledger_check.py` passes on it, `review.md` is
  written, and you have told the owner the counts and the decisions you need.
