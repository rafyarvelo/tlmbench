# UI audits

One dated section per audit (`docs/release/release-plan.md`, "UI audits with the apple-design skill"). Findings are
ranked; each names the screenshot it comes from. The prototype and CLAUDE.md's UI conventions win over the skill: where
they conflict, or a finding would change the design, it goes to the owner with a recommendation. Kept until R8.

## Nov 14, 2026: end of R2.1a (the owner's first test-sheet list)

Scope: the screens that list touched: dialogs (Export to HTML, Rules and checks, Reset app), the export's progress and
end note, the top bar's Export menu, pane actions and grips, XY's header, Chart and values, the Explorer header, the
viewer's header. Screenshots: headless Chromium at 1440 × 900 against the built UI and the fake core, dark and light
(`dark-*.png`, `light-*.png` from a scratch script; not kept in the repo). The `anthropic-skills:apple-design` skill was
loaded and its headings used: response and feedback, spatial consistency, restraint and simplicity, typography, materials
and depth, motion and reduced motion.

### Fixed in this pass

1. **Feedback kinds: a finished export showed the warning dot** (`*-html-done.png`). Every top-bar note used the
   orange warning dot, so "Exported to … · Show in Finder" read like a problem. A note that reports success now uses
   the accent dot (`ui.noteOk`); failures keep the warning dot. `t_exporthtml`.
2. **Restraint and legibility: the Export menu's hints ran inline and were cut off** (`dark-export-menu.png`:
   "Export to HTML…A page anyone can open, with no…"). The three items are plain names; the hints are tooltips.
   `t_export`.
3. **Grouping: the progress bar sat at the far right, away from its label** (`dark-html-progress.png`). The bar and
   percentage now follow the label ("Exporting flight to HTML… ▬▬▬ 25%"). Screenshot.

### Checked, no change

- **Response.** The export dialog closes as soon as the folder is chosen, and the progress line appears in the same
  frame; pane actions, Duplicate, Split and Close act in one click.
- **Spatial consistency.** Every dialog opens centred near the top over a scrim and rises 8 px as it fades in (the
  prototype's), and closes the same way; the axis popover opens at its button. The Export menu opens from its button.
- **Calm UI.** Pane actions, XY's controls and the Events toggle show on hover or selection only (`*-main-hover.png`,
  `light-xy-cv.png`); the rarer ones give way when they don't fit. The viewer's "Read-only view" pill is the only new
  always-visible element and carries the export's provenance in muted text.
- **Typography and tokens.** Sizes in the new UI are 11.5, 12, 13 and 15 px; radii 4, 8 and 12; monospace only for the
  percentage, the command line and numbers.
- **Reduced motion.** The progress dot's pulse and the bar's width transition stop under `prefers-reduced-motion`.
- **Materials.** The app uses opaque surfaces with a scrim for modal dialogs, as the prototype does; translucent chrome
  is not part of its design, so the skill's material advice doesn't apply.

### For the owner (not changed: they would change the design)

- **The XY header's "Fit" when disabled** is a faint box (`light-xy-cv.png`), as the prototype's `.xyb:disabled`. The
  skill would hide a control that can't act. Recommendation: keep it (the prototype's, and it shows where Fit is).
- **Chart and values' grip at the pane's bottom** follows the owner (sheet B5), not the prototype (whose grip is under
  the plot). Recommendation: keep the owner's; note it for the design doc.

## Nov 15, 2026: R2.2, browser-only mode and the viewer (light and dark)

Scope: browser-only mode (the start page, the Explorer empty state, a CSV open with three panes) and the viewer over an
HTML export of the same file. Screenshots: headless Chromium at 1440 × 900 against `app/browser/dist` and a folder export
made by the release CLI, dark and light (`{dark,light}-browser-empty.png`, `-browser-open.png`, `-viewer.png` from a scratch
script; not kept in the repo). This audit covers what exists: R2.2's binary formats, derived channels, events, checks and
several files in a tab are not built yet (ledger S5.10-04), so their screens are not audited. Headings from the
`anthropic-skills:apple-design` skill: wayfinding, familiarity, simplicity, feedback, typography, materials, motion.

### Fixed in this pass

1. **Wayfinding: the start page and Explorer offered what a browser tab can't do** (`dark-browser-empty.png`). "Open folder…"
   was the primary button, "Connect live source…" sat beside it, the Explorer said "Open a folder" and "drop a CSV or .tlm
   file", and Connections listed "None saved". None of these can work with no app behind the page: a press did nothing. In
   browser-only mode the start page now offers Open recording… as the primary button, no folder or live buttons, no
   Connections section; the Explorer says "Choose a CSV file, or drop one on the page" and its header says "This browser"
   (a label, as the viewer's says "Export"). `t_browseronly`.

### Checked, no change

- **Responsiveness while a file loads.** The parse runs in a Worker; a 600,000-row CSV opens while the page keeps drawing
  frames (`t_browseronly`). The open shows the usual progress note.
- **The viewer is the app, with less** (`dark-viewer.png`, `light-viewer.png`): the same layout, the same tokens, no editing
  controls, a "Read-only view" pill with the export's file, date and version. Light and dark match the app's themes; plots
  read well in both (the series colours are the app's).
- **Typography and tokens.** Nothing new: sizes 11 to 15 px, radii 4, 8 and 12, monospace only for numbers and units.
- **Materials and motion.** Opaque surfaces and a scrim, as the prototype; no new motion. Reduced motion is unchanged.

### For the owner (not changed)

- **The viewer's provenance text is cut with an ellipsis at 1440 px** ("… TelemetryBench 0…."). The full text is in the pill's
  tooltip. Recommendation: keep; at narrower widths it must give way, and the version is the part that matters least.
- **Browser-only mode keeps the rail's Live, Rules and checks, and Layouts icons**, which open panels that mostly can't act
  until the rest of R2.2 lands. Recommendation: leave them, so the layout matches the app, and hide each as its feature
  stays unbuilt only if the owner prefers a smaller rail (a choice about scope, so it is yours).
