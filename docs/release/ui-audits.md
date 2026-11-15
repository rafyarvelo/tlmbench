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
