# Follow-ups: design-doc notes owed

Everything this file held up to Nov 24 was folded into the design doc's "Final sync (Nov 24, 2026)" section (in Claude Docs,
exported to `docs/design.md`; the older text is in git history: `git show 86ae974:docs/follow-ups.md`). The same sync moved four
decisions from Proposed to Accepted (the descriptor form, `files` patterns, `[packets]` without a length, the app's Export; the
owner's Nov 7 calls, which had only been made in the repository's copy) and added the `just gate4` result.

New notes go here, with the date, when behaviour differs from the design; they are folded at the next export. This file is
deleted at the release tag (R8).

- **Nov 25, new panes (owner, rc.1 test plan 3.5).** Add pane (under the stack, and its ▾ kinds), ⋯ › New pane and ⌘⇧N
  add an empty pane below the selected pane's row (at the bottom when none is selected), and select it; before, Add
  pane and ⋯ New pane always added at the bottom. ⋯ › All panes has New pane in the desktop app again (the prototype's
  group); the design's Panes bar line ("Pane actions (new, clear channels, remove all) live in the Explorer's Panes
  bar") and the ⋯ menu line ("New pane, Remove every channel or pane and Export are the menu bar's and the top bar's")
  need updating to say where these are now.

- **Nov 25, Explorer row actions (owner, rc.1 test plan 3.5: match the prototype).** The selected channel row (while
  pointed at), the open file's row and, on hover, a category row show the prototype's icons over the row's end: add to
  the selected pane, a new chart, a new table, and for a plotted channel remove from every pane. Every "open in a new
  chart / table" (the selection bar, Explorer ⋯, a table's selected columns, a check's row) now puts the new pane below
  the selected one, not at the end. The prototype's "Clicking a row" setting is not in the app (Select first is how the
  app always works). The design's Explorer text could say so.
- **Nov 26, Install command-line tool (owner, rc.1 test plan 1.9).** When the tool's folder isn't on PATH, the line to
  add is shown in a small dialog that stays until Done, read-only and selected, with Copy (it was a note that went in
  12 s). The design's R4 text could say so.
