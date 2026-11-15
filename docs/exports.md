# HTML exports: folder or one file

`tlmbench export html` (and the app's Export to HTML…) writes a **folder** of static files: `index.html`, the viewer's
script, the core compiled to WebAssembly, `export.json`, and the data in chunks. Put the folder on any static host or
file share that serves web pages. Nothing runs on the server.

## Why a folder needs a web server

A page opened from your disk (`file://`) is not allowed by the browser to load other files: not the WebAssembly core, not
the data, and not even its own script as a module. That is the browser's rule, not ours, and no page can work around it.
So a folder export opened by double-click shows a short note saying so. Either:

- serve the folder (any static host, or `python3 -m http.server` inside it, then open the address it prints), or
- write **one HTML file**: `tlmbench export html --single-file --out report.html`.

## One file

Export to HTML… offers **Folder** (any size; needs a web server or a host) and **One file** (opens from anywhere), each
with its size. `--single-file` is the same on the command line. One file puts the page, the core and the data in one
file: email, a file share, your disk. It is still a static page that fetches nothing. The price is size: the data is
stored as text, a third bigger, so the file has a limit, **25 MB** to start; change it in the dialog (Size limit) or with
`--single-limit 100MB`.

Within the limit the file carries **every raw sample**. When they don't fit it is a Light export with its finest pyramid
levels left out as needed, and a Light export keeps raw samples only where it was told to (channels shown as a table,
values or XY, and windows around events and check violations). If even that can't fit, the command says so and suggests
fewer channels, a time range (`--from`, `--until`), a higher limit, or a folder.

## What a view shows when the data isn't in the export

Every value the viewer shows is a raw sample or an exact aggregate of samples. Where the export holds only the pyramid,
a chart is drawn from its summaries and labelled coarse; a table, a value at the cursor or exact statistics are left
blank rather than guessed. `--detail` (`30s`, or `3x,30s,10min`; `0` for none) sets the raw windows Light keeps around
events and violations.

## Redaction

`--redact a,b` leaves channels out everywhere: the data, the layout, the check sets that read them (a check can't lose a
channel and keep its meaning, so it leaves the export with its violations), and the channel lists of events.

## Widgets

Widgets of your own travel without code: a declarative SVG widget is sanitised (no scripts, foreign objects, images,
media, event handlers or outside links), and a code widget becomes a card that shows the value, its unit and the
widget's name.
