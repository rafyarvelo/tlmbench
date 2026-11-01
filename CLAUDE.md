# TelemetryBench (tlmbench): notes for Claude

Desktop telemetry workbench: Rust core, Tauri app with a web UI, CLI and Python bindings over the same core.
Today the repo holds the design doc, a single-file HTML prototype of the UI, its tests, and example shared config.
The v1 code has not started; the roadmap (Phases 1–5) is in `docs/design.md`.

## Hard requirements (never trade these away)

- Full capability from the app cache alone: everything works on a read-only folder.
- Nothing is written next to the data except by user action: Share (to the folder's `.tlmbench/`) and exports.
- Exports are static files (HTML export opens from any static host or file share, no install).
- Everything in the design is v1. There is no rush to release; don't propose cutting scope to ship sooner.
- The app never runs Python or other external programs. Custom code runs as WASM plugins (sandboxed) or
  native plugins; Python users work through the bindings outside the app.

## Where things are

- `docs/design.md`: the design, exported from the living doc (links in `docs/README.md`). Every suggestion has
  a decision: Proposed, Accepted, Modified or Rejected. Treat Accepted/Modified as requirements.
- `prototype/plotter.html`: the UI prototype (~11k lines, one file, no build step). Data is synthesized in the
  page; Share and Export are simulated. Published copy: https://claude.ai/artifact/L3Aw4dwFS24oqwfU1yJDv5
- `prototype/tests/`: Playwright tests, one `t_*.js` per area, `./run.sh` to run them (see its README).
- `docs/development.md`: setup (`brew bundle`, `ci/setup.sh`), recipes, pinned versions, CI. `docs/follow-ups.md`:
  deferred work (large recordings, benchmark tracking).
- `justfile`, `ci/`: the same recipes run on the Mac and in CI (`just ci-linux` in the image from `ci/Dockerfile`).
  Tool versions are pinned in `rust-toolchain.toml`, `ci/versions.env`, `requirements-dev.txt` and the tests' package.json.
- `examples/recordings/.tlmbench/`: shared configuration in the formats the app writes (TOML for files people
  edit, JSON for layouts, one small file per object).

## Working on the prototype

- After any edit, check the script parses:
  `python3 -c "import re;s=open('prototype/plotter.html').read();m=re.findall(r'<script>(.*?)</script>',s,re.S);open('/tmp/check.js','w').write(m[-1])" && node --check /tmp/check.js`
- Run the tests for the area you touched, then the whole suite before you finish: `just proto-test`
  (`just proto-test t_checks t_rules` runs some). Timing tests (`t_loading`, `t_perf_gauges`) run alone at the end.
- Add or extend a test for every behaviour change; update the table in `prototype/tests/README.md`.
- Tests talk to the page through small hooks on `window` (`__tlmChecks`, `__tlmDerived`, `__tlmGauges`,
  `__tlmPlay`, `__tlmToml`). Top-level `const`s aren't reachable from Playwright, so add to a hook if needed.
- Charting and table libraries are stubbed in tests (`stub.js`, `tsstub.mjs`): tests check behaviour and state,
  not pixels. Check visual changes with a screenshot.
- No backward compatibility is needed in the prototype: change formats and settings freely.

## UI conventions (the owner cares about these)

- Production-like behaviour: everything configurable is saved in settings.
- Calm UI that still avoids extra clicks: controls appear on selection, hover or focus, not all at once;
  frequent actions are one click; rarer ones go in ⋯ or right-click menus.
- Tokens: font sizes 11, 11.5, 12, 13, 15, 26 px (20 only inside widget SVGs); radii 4, 8, 12, round; monospace
  only for numbers, times and units.
- Tooltips use `data-tip` (a `title` is converted); short, and they retire after 3 uses.
- Copy is plain and short. Say what a control does, in sentence case.

## Owner's preferences

- Concise replies. Lead with what changed and what's left; skip recaps.
- Decisions belong to the owner: when a design question is open, give a short recommendation and ask.
- Keep the design doc the source of truth. When behaviour changes, note it so the doc (and `docs/design.md`)
  can be updated.

## Git

- Small commits with clear messages, one topic each. Don't push or rewrite history unless asked.
- Never commit recordings, caches or test output (see `.gitignore`).
