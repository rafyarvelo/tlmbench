# Docs

- `design.md`: the design doc (architecture, data model, UI, checks, CLI, sharing, roadmap), exported from the living doc. Each suggestion carries its decision (Proposed, Accepted, Modified, Rejected).
- `roadmap.md`: the steps from here to v1 (one agent at a time), with what each needs and when it's done.
- `feature-matrix.md`: every feature's state in the design, the prototype and the app, and the step that closes it.
- `parity-audit.md`: the app's UI against the prototype, control by control.
- `follow-ups.md`: deferred work and what's left of finished phases.
- `development.md`: setup, recipes, pinned versions, CI.
- `release/release-plan.md`: from step 5.10 to v1.0.0 (review, gaps, packaging, documentation, cleanup, release), in order.
- `release/review-prompt.md`: the prompt for the agent that reviews every step against the roadmap and writes the ledger.
- `release/ledger.md`: every v1 requirement with its verdict and evidence; `ci/ledger_check.py` checks it.
- `release/review.md`: what the review found: the gaps ranked, the drift corrected, the owner's decisions.
- `exports.md`: HTML exports, a folder or one file; why a folder needs a web server; what a trimmed export shows.
- `release/r2-1-sparse-store.md`: the plan for the viewer's sparse raw store and which data a view reads (R2.1).
- `release/owner-test-sheet.md`: what to try in the release app, with Result and Notes lines the owner fills in and hands to an agent.
- `release/next-agent-prompt.md`: the prompt for the next agent: R2.1 is done; it asks the owner for their list of actions from testing the release app before anything else.

The living versions are edited in Claude Docs and exported here at each milestone:

- Design doc: https://claude.ai/artifact/Vt1EH8gVVeFL9977smjvZy
- Pre-development decisions (log): https://claude.ai/artifact/WCHTg5B27DN2Tuef14Mq1W
- Design audit (Apple-style review of the prototype): https://claude.ai/artifact/XGbUcaDzaC74zPmH44MWXK

When a decision changes, update the design doc first, then re-export.
