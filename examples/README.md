# Example shared configuration

`recordings/.tlmbench/` is what a team commits next to (or instead of) its recordings: configuration only,
one small file per object, TOML for files people edit and JSON for layouts. These files were written by the
prototype's own Share code, so they match what the app writes.

| Path | What |
| --- | --- |
| `checks/*.toml` | rule-and-check sets: one `[[check]]` table per row (kind = check or rule) |
| `categories.toml` | event categories: id, name, colour, shown on charts |
| `recipes/*.toml` | derived-channel recipes |
| `views/*.tlmview.json` | shared layouts |
| `widgets/level-bar/` | a code widget: `widget.toml` (manifest) and `widget.js` (build/update); runs only after the trust prompt |

Recordings themselves are never committed here (see `.gitignore`); caches never go in `.tlmbench/`.
