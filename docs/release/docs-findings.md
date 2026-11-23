# What the docs passes found in the app

Each documentation pass lists here what it found unclear or broken in the app (file:line), and what was done. The UI is
frozen: only bugs are fixed, each with a test; the rest waits for the owner.

## Screenshots (Nov 24)

| Where | Finding | What was done |
| --- | --- | --- |
| `app/ui/src/shell/WidgetTrust.svelte:24` | The "Run code widgets from this folder?" question read "Level bar (code)by j.kim": Svelte drops the space before an `{#if}`. | Fixed (`{' '}`); `t_widgetshared` checks the sentence. |
| `app/ui/src/shell/Checks.svelte:219` | On a Report tab the Rules & checks panel says "Open a recording to run checks on it.", though a report is open. | Unclear, not broken (no file is in front). Left. |
| `docs/manual/12-exporting.md:70` | The manual described a "Locked pill" in the viewer's header; the app has a lock button (closed when locked) and the file name and date beside it. | The manual now says what the app shows. |
