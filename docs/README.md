# Docs

**Plan and status** (`docs/release/release-plan.md`, "One plan, one status", says which file holds what):

- `release/release-plan.md`: the steps from here to v1.0.0 and **the status table**, the only record of where each step stands.
- `release/ledger.md`: every v1 requirement with its verdict and evidence, **the only feature status**; `ci/ledger_check.py` checks it.
- `release/next-agent-prompt.md`: the short hand-off for the next agent.
- `release/owner-test-sheet.md`: what the owner still has to try in the release app, with Result and Notes lines.
- `release/ui-audits.md`: the apple-design audits, one dated section each.
- `release/parity-classes.md`: every difference from the prototype as Port, App is better or Noise (release step R3.0).
- `follow-ups.md`: notes owed to the design doc. `post-v1.md`: what the owner placed after the release.

**Reference:**

- `design.md`: the design doc (architecture, data model, UI, checks, CLI, sharing, roadmap), exported from the living doc. Each suggestion carries its decision (Proposed, Accepted, Modified, Rejected).
- `development.md`: setup, recipes, pinned versions, CI, the gates.
- `exports.md`: HTML exports, a folder or one file; why a folder needs a web server; what a trimmed export shows.
- `viewer-store.md`: what an export holds and which data the viewer's views read.
- `browser-only.md`: the app in a browser tab with no install, and how to host it.

**History** (frozen; don't update): `roadmap.md` (steps 4.1 to 5.10), `feature-matrix.md`, `parity-audit.md` (Nov 11),
`release/review.md` (the R1 review).

The living versions are edited in Claude Docs and exported here at each milestone:

- Design doc: https://claude.ai/artifact/Vt1EH8gVVeFL9977smjvZy
- Pre-development decisions (log): https://claude.ai/artifact/WCHTg5B27DN2Tuef14Mq1W
- Design audit (Apple-style review of the prototype): https://claude.ai/artifact/XGbUcaDzaC74zPmH44MWXK

When a decision changes, update the design doc first, then re-export.
