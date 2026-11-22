# Release app test sheet (owner pass)

**How to fill this in (about 40 minutes).** Open the release app (`target/release/tlmbench-app`, or the installed one) and go
down the list. For each test type `PASS`, `FAIL` or `SKIP` after `Result:` (nothing else on that line), and when it isn't a
clean pass write what you saw after `Notes:` in plain words. Optional: add `Severity: blocker | annoying | cosmetic`.
Then use "Other things I noticed" at the end for anything the list didn't ask about. When done, hand this file to the next
agent (`docs/release/next-agent-prompt.md` says what it does with it). You don't need to explain a fix, only what you saw.

**Test data:** run `just testdata` (or `python3 tools/testdata/make.py`). It writes `~/tlmbench-testdata/` and never touches your
own folders:

| File | Use it for |
| --- | --- |
| `flights/flight-1.csv` | a good run, 60 s at 100 Hz (altitude, speed, battery_v, temp); every check passes |
| `flights/flight-2.csv` | the same channels an hour later: the second file in a tab (D1, D2, F6) |
| `flights/flight-bad.csv` | battery under 12 V for 10 s and a temperature spike: the checks fail here (F7, F9) |
| `flights/flight-gap.csv` | a 30 s hole in the times: gap breaks and "connect gaps" (F5) |
| `flights/.tlmbench/` | shared config: the check set "Flight limits", the layout "Altitude and battery", two widgets (an SVG dial and a code widget), two events on flight-1 (F8) |
| `grow.py` | `python3 ~/tlmbench-testdata/grow.py ~/tlmbench-testdata/live.csv` adds a row a second, for Live (G1) |
| `just testdata big` | also writes a 100M-sample `.tlm` (about 400 MB) under `big/`, for B4 and H5 (needs `cargo build --release -p tlmbench-gen` first) |

Open the folder `~/tlmbench-testdata/flights` in Explorer. Your own folder `/Users/rafy/workspace/test` (AAPL.csv, GOOG.csv, …,
run.parquet) is good for Parquet and for a messier look.

In the terminal commands below, replace `<file>` with `~/tlmbench-testdata/flights/flight-1.csv` and `<set>` with `Flight limits`.

```
Date:
App: release build from commit (agent fills in): `e484d77`, built Nov 17, 2026 (evening): `target/release/bundle/macos/TelemetryBench.app` or `target/release/tlmbench-app` (CLI: `target/release/tlmbench`). New: sections X and W. Everything you confirmed fixed or good is removed: what is here is still open.
Mac / macOS version:
Display (scale, one or two monitors):
Opened as: (the built app / an installed copy)
Started with my usual settings, or a fresh profile?:
```

*For the agent reading this:* every `FAIL` and every non-empty `Notes:` is an item for the triage table. The IDs (`A1`,
`B3`…) are stable; cite them in commit messages and in the ledger row you add or update. `L` = ledger row it exercises.

---

## R. Still open from the Nov 14 changes

R12 and B4 as changed on the Nov 15 build from the commit below: please re-test. 

### R12. An export's viewer: locked, with Unlock (F1)
You wrote: the exported view "isn't really 'read-only' since some of the panes 'duplicate' actions work, but the dragging
options don't work … is it possible to just allow exported views to have the user be able to move and drag channels like
the app. Having the exported view start 'locked' and having the user click unlock … Also the Events tab shows the rules
buttons but they don't work and there's no visual indication … either hide them or disable and put some UI indication."
Changed: the viewer opens **Locked** (the pill in the top bar; its tooltip keeps the file, date and app version). Click it to
**Unlock**: drag channels between panes (Option or ⌘ copies), from the Explorer, reorder panes by their number, split,
duplicate, close, resize, change a pane's view. Click again to lock. Nothing is written into the export; a reload is the
exported view again. Locked, those controls are greyed with "Unlock the view to…". Everything else either works or is left
out: the Events panel's Marker, Range and Rules are greyed and say an export is static; Settings shows only display
settings; the tab menu has no Save or Share.
Do: export flight-1 to HTML (One file), open it. Try dragging a channel while Locked, then Unlock and drag it, with Option
held, split, duplicate, close, resize. Open the Events panel and Settings. Open it in Safari too if you can.
Expect: locked nothing moves and the greyed controls say why; unlocked it behaves like the app; no button that does nothing.
You decided (Nov 15): an unlocked arrangement is not kept when you reload; a reload is the exported view.
Result:
Notes:
(Agent, Nov 16: your decision changed this twice; the viewer now has the app's one padlock Lock (D-ui-62). Re-test it with V7.)

---

## S. New in the Nov 15 build (your decisions 7 and 8, and browser-only mode)

For the Nov 15 build only. `Result:` and `Notes:` as above. Browser-only mode needs the page served: run
`python3 -m http.server --directory app/browser/dist` from the repository and open http://localhost:8000 (S4).

### S1. Export to HTML: Folder or One file (decision 7, F2)
You said: "The user should be able to choose between a full export with a server and a standalone HTML / static folder."
Changed: Export to HTML… has "Save as": Folder (any size; needs a web server or a host) or One file (opens from anywhere),
each with its size. One file has a Size limit field (25 MB to start) and the command line shown at the bottom follows.
Do: Export to HTML… on flight-1: look at the Save as list and the summary line; choose One file, Export…, save it on the
Desktop, double-click it. Then choose One file with the limit at 1 MB and look at what the dialog says. Then a Folder.
Expect: One file asks for a file name (not a folder), writes one `.html` that opens from the Desktop with the chart and
nothing fetched; with every raw sample in it when it fits (the end note says "Full"); at 1 MB it says Light and why, and
that a folder has no limit. The Folder choice works as before. Your choice and the limit are there next time.
Result:
Notes:

### S2. The viewer's coarse plots (F3)
You said: "the 'coarse' plots are way too coarse… they should look more like the prototype."
Changed: a small export now carries every raw sample (S1), so it is not coarse at all. When it is coarse (Light, a trimmed
export), the viewer draws the same min/max band under the line that the app does.
Do: (a) flight-1 as One file: zoom in and out in the viewer. (b) Make it coarse:
`target/release/tlmbench export html <file> --layout "Altitude and battery" --mode light --detail 0 --out ~/Desktop/coarse`
(the layout's charts then keep only summaries), serve it (`python3 -m http.server --directory ~/Desktop/coarse`), and compare the viewer with the app over the same file at the
same zoom.
Expect: (a) smooth, exact lines, no "coarse" label. (b) a "coarse" label, and the band and line look like the app's.
Result:
Notes:

### S3. The menu bar (decision 8, R15 idea 1)
Changed: File, Edit, View, Pane, Window and Help, all from one list that also drives the shortcuts. Shortcuts with ⌘ are
bound in the menu; single keys (E, L, T, F, Esc, Backspace) are shown in the item's name, like "Fit to the data (F)", and
still work from the keyboard only. Cut, Copy, Paste and Select All are the system's.
Do: Open each menu. Try: Open recording… (⌘O), Open folder… (⇧⌘O), Save layout (⌘S), Undo and Redo (⌘Z, ⇧⌘Z), Duplicate
pane (⌘D), New pane (⇧⌘N), Settings… (⌘,), Focus mode (⌘.), and a click on Event markers and Lock the time view (the check
mark should follow). With nothing open, look at what is greyed out. Click in a text field (the Explorer's Filter) and use
⌘Z, ⌘C, ⌘V there. Open a Table pane, select cells, press ⌘C and paste into another app. Click in the Explorer and press ⌘A.
Expect: each does what its name says, once (not twice); items that can't act are greyed; text fields keep their own undo,
copy and paste; the Table's ⌘C and the Explorer's ⌘A still work. With two windows open, a menu click acts on the window in
front. Is anything missing that you expected in a menu?
Result:
Notes:

### S4. Browser-only mode (R2.2, partly built)
You said: "I expected Browser support to be almost analogous to what the prototype does: open folders and CSV files."
Changed so far: a CSV is read in a background thread (a big file doesn't freeze the tab); a reload brings the open files
back; the page no longer offers folders or live sources, which a tab can't do. **New (Nov 15, later build): a `.tlm` and a
Parquet file open** (Parquet: snappy, LZ4, gzip, brotli and Zstandard; its reader is a separate download of about 1 MB that
the page fetches only when you open a Parquet file). Not built yet: packets, descriptors, WASM plugins and folders
(next). Several files, derived channels, events and checks in a tab come after v1 (your scope, Nov 15).
Do: Serve the page (see above). Choose or drop `flights/flight-1.csv`. Then `browser/bench.tlm` (200,000 samples; `just
testdata` writes it, and `browser/codecs-snappy.parquet` and `codecs-zstd.parquet` if pyarrow is installed). Open a
Parquet file and look at the Network tab of the browser's developer tools: `tlmbench_parquet.wasm` loads only then. Drop a
bigger CSV (`/Users/rafy/workspace/test` has some; or make a 500,000-row one). Reload the page. Drop a file that isn't
one of these. Look at it in light and dark.
Expect: each opens and draws like the app's (the `.tlm` has altitude in m and battery in V, the time from its own field;
the Parquet file has speed in m/s and Temp in °C, its text column left out); while a big CSV loads, the page still
responds; after a reload the files are back with no picker; the start page offers only Open recording…; a file it can't
read says browser-only mode reads CSV, Parquet and .tlm files.
Result:
Notes:

---

## T. R3.1 and R3.2 (Nov 15 build): what you haven't checked yet

You confirmed (removed): the ⋯ menu, the time range, Explorer's quiet rows, Open with layout ▸, the start page, rates and
filters; on Nov 16 C2 (tabs and the panel overlap), C3 (windows), F10 (exports), V3 (Open with its default app, the menu note), V1, V2, T5, T1 (slot numbers), T2 (Rename, Save layout), T3 (Duplicate recipe), D1 and D2, G1 and H1 to H4. Left:

### T4. The viewer's tab name, when it's long
You wrote (Nov 16): the layout "Altitude and battery" couldn't be found after `just testdata` (ledger D-ui-64).
Agent, Nov 16 night: fixed: the test data named the file `.json`; the app reads `*.tlmview.json`. Run `just testdata` again.
Result: Pass (Nov 16 night): the layout is found after `just testdata`.
Notes:

---

## U. The Nov 15 build after R3.2 (browser-only folders, a tab's hidden controls, R3.3's UI rows)

Rebuild first (`just viewer`, `cargo build --release -p tlmbench`, `cargo tauri build --no-bundle` in `app/src-tauri`) and, for U1
and U2, `just browser-site` and serve it (`python3 -m http.server --directory app/browser/dist`). Fill in as above.

### U1. Browser-only mode opens a folder (D-brow-10)
Do: in Chrome or Edge, open http://localhost:8000, click Open folder…, choose `~/tlmbench-testdata/flights`. Open a CSV from the list.
Then reload the page. Then open it in Safari or Firefox.
Expect: the folder lists in Explorer (sub-folders first, no `.tlmbench`), nothing is written to it, the CSV opens; after a reload the
browser asks permission again (or remembers it); in Safari and Firefox the start page says folders aren't available and offers Open
recording…. The folder's `.tlmbench/` isn't read yet.
Result:
Notes:

### U2. A browser tab shows only what it can do (D-brow-05)
Do: with a file open in the browser page, look at the rail, Explorer (a channel's right-click, the file's right-click, ⋯), the tab
menu, the Events panel, a pane's view menu, Settings › Reset app.
Expect: no Layouts or Rules & checks; no Derive, Compare, Open together, Add to tab, Open with layout, Mark when, Add check; no Save,
Share or window items in the tab menu; Events has no Marker, Range or Rules and says events come after v1. (Question for you:
should the Events panel go from the rail too, since it is empty there?)
Result:
Notes:

### U3. Pane names (D-ui-43)
Do: a pane's ⋯ › Rename pane…, a double-click on its number, a name of 60+ characters, an empty name, Duplicate, Save as a layout,
export to HTML and open it.
Expect: the name shows before the chips, a copy is "… copy", the saved layout and the export keep it.
Result:
Notes:

### U4. Several files in one tab (D-ui-46 to D-ui-49)
Do: Open together on flight-1 and flight-2 (not open yet); a pane's ⋯ › Duplicate for file 2; right-click a chip › From file 2 and
Add from every file; set a limit on a channel and look at the same channel of file 2.
Expect: one pane per shared channel with both traces, then each file's own charts; the copies and swaps do what they say; the
limit applies to both files' copies and the editor says so.
Result:
Notes:

### U5. Event tooltips and Stop at (D-ui-11, S5.2-15)
Do: add a marker and a range at cursors; hover them. Open Play, Stop at: tick two categories, play.
Expect: a tip with name, category, start and length; playback pauses at the first event of either category.
Result:
Notes:

---

## V. The Nov 16 build (your sheet's findings and decisions)

Rebuild first (`just viewer`, then `cargo tauri build --no-bundle` in `app/src-tauri`) and, for V4 to V6, `just browser-site` and
serve it (`python3 -m http.server --directory app/browser/dist`).

### V7. Exports of a tab of several files, and the viewer's lock (your T4 notes, Nov 16)
You wrote (Nov 16): chips still don't drag in Safari after Edit layout (Explorer rows do); and keep the prototype's one Lock
icon, locking both the time view and moving channels, in the app and in exports (ledger D-ui-62, D-ui-63).
Agent, Nov 16 night: built as you chose. The padlock is on the bar in the app and the viewer; L toggles it; locked, zoom, pan,
Fit and Back are refused (a note offers Unlock) and panes and channels stay put, while cursors and table sort and filter work.
Safari: a drag's first moment no longer moves the chip (the drop gaps waited for the next frame; Safari dropped drags whose
chip moved under it). No headless test can show this, so it's yours to confirm.
Do: rebuild (above, and `cargo build --release -p tlmbench`). In the app: press L, try to zoom, drag a chip, add a pane; click the
padlock. Export flight-1 as One file and open it in Safari: it opens locked; try to zoom; click the padlock; drag a chip to
another pane, with Option held too; drag a pane number; press L.
Expect: locked, nothing zooms or moves and the note offers Unlock; unlocked, chips and pane numbers drag in Safari as in Chrome;
a reload of the export is locked again.
Result: Pass (Nov 16 night): the lock in the app, a One file export and a folder export; chips and pane numbers drag in Safari, with Option too.
Notes:

### V4. Browser-only mode: binary files and descriptors (D-brow-08)
Do: in the browser page, open a binary file nothing describes (a `.bin`): the descriptor form opens; Open it. Open a second file
the pattern names. Reload.
Expect: the form previews the records; no Share (a tab never writes to the folder); the second file opens with no form; after a
reload the files are back.
Result:
Notes:

### V5. Browser-only mode: WASM plugins (D-brow-09; your decision Nov 16)
Do: `just plugin-template`, then Settings › Plugins › Add plugin…, choose `plugins/template-decoder/target/wasm32-wasip2/release/my_decoder.wasm`.
Open a `.demo` file (the template's format; `python3` can write one: magic `DEMO`, a u32 count, records of u64 µs, f32, i16).
Expect: the plugin is listed as WASM, sandboxed; the file opens; a `.dylib` is refused as needing the app. The first use fetches
about 5 MB once (jco).
Result:
Notes:

### V6. Browser-only mode: the folder's .tlmbench/ (D-brow-10)
Do: in Chrome, Open folder… on `~/tlmbench-testdata/flights`.
Expect: its categories and widgets are there; a descriptor or plugin it shares names its files; nothing is written to the folder.
The Events panel isn't on the rail (your U2 answer).
Result:
Notes:

---

## B. Reading a chart

### B4. A file that is still building
Why it showed nothing: on this Mac the 100M-sample file builds its pyramid in about a second, so the hatching was gone before
you could look. The app now has a slow-motion build for looking at it: `TLMBENCH_SLOW_BUILD=<ms>` waits that long after each
block of about a million samples (100 ms makes the 100M file take about ten seconds).
Do: quit the app. In a terminal: `target/release/tlmbench cache clear` (a file opened before has its pyramid in the cache and
opens at once), then
`TLMBENCH_SLOW_BUILD=100 target/release/tlmbench-app ~/tlmbench-testdata/big/big-100M.tlm`
(`just testdata big` first if the file isn't there).
Expect (seen in the release window on Nov 15: hatching right of the built part, "Reading 27%" in the middle of it, the overview hatched too): for about ten seconds the chart is hatched where it isn't built yet and fills in left to right; the min/max band shows
on a spiky channel; "coarse" shows after 0.3 s; afterwards zoom is smooth. Without the variable it still opens in about a second. (L: X-win-06, X-win-11)
Result:
Notes:

### B6. Text and fonts
Do: Compare a screen of the app with the prototype (`prototype/plotter.html`, or the published copy) side by side.
Expect: same fonts, sizes, spacing; nothing clipped. (L: X-win-06)
Result:
Notes:

## F. Export (what R2.1 added): the most important section

### F4. Light mode and the raw-window fields
Do: Export to HTML… with Mode: Light.
Expect: three fields appear ("Raw around events", "At least (s)", "At most (s)"); changing one changes the command line shown
at the bottom of the dialog and is remembered next time.
Result:
Notes:

### F5. A Light export, in the viewer
Do: Export a big file as Light (or force a small budget with the CLI `--budget 2MB --mode light`), open it in the viewer.
Expect: charts draw and are labelled coarse where raw samples aren't in the export; zooming in to an event window is exact;
a table or a value at the cursor outside the windows is blank, never a made-up number.
Result:
Notes:

### F6. Two files in one export
Do: With two files in a tab (D1), Export to HTML…, choose "Every channel of the file", open the result in the viewer.
Expect: one tab with both files' channels, the second on the same timeline as in the app.
Result:
Notes:

### F7. A check report that links to the failure
Do: In a terminal, with a folder of CSVs and a failing set: `target/release/tlmbench check ~/tlmbench-testdata/flights --set "Flight limits" --report html --views --out ~/Desktop/report`
then open `report.html` through a web server (`python3 -m http.server --directory ~/Desktop/report`) and click a red cell.
Expect: the cell opens an export of that file, already zoomed on the first violation.
Result:
Notes:

### F8. Widgets of your own in an export
Do: Make an SVG widget (Settings › widgets) and a code widget, put each in a Gauges pane, export, open the viewer.
Expect: the SVG widget draws; the code widget shows as a plain value card; nothing runs.
Result:
Notes:

### F9. Redaction
Do: `target/release/tlmbench export html <file> --all-channels --redact battery_v --checks "Flight limits" --out ~/Desktop/r`
Expect: no `battery_v` data in the folder; the check "Battery above 12 V" is gone from the summary; open it through a web server and confirm.
Result:
Notes:

## G. Live and plugins (skip what you don't use)

### G2. TCP or UDP source with a trigger
Result:
Notes:

### G3. A code widget in the app
Expect: asks once per folder before running shared code; then runs. (L: X-win-07)
You wrote (Nov 16): not obvious how to do this. It is a JSON list in Settings › widgets today; making a code widget from the
app without the JSON is ledger D-ui-59 (not built yet). To try it now: open `~/tlmbench-testdata/flights` (its `.tlmbench/` shares
a code widget), add a Gauges pane, and pick the shared code widget there.
Result:
Notes:

## H. Settings and look

### H5. Memory and speed
Do: Open the 100M-sample file (B4). Watch the status bar memory number and the feel of zoom.
Expect: memory stays reasonable, zoom stays smooth.
Result:
Notes:

## X. Built Nov 17 (the Sonnet run and your Nov 17 decisions): nothing here has been seen in a real window

### X1. Icons and the pinned strip (D-gauge-06)
Do: Settings › Icons: add an SVG. Pin a layout (Layouts panel, right-click › Pin to toolbar) and a recipe (Recipes manager).
Expect: a "Pinned" strip in the top bar only once something is pinned; one click opens the layout or adds the recipe to the
file in front; the icon follows the theme (light and dark).
Result: Adding an SVG as an icon works, but the edit box is tiny and not resizeable. Also it didn't work for shared layouts and the context menu didn't allow the user to easily unpin the layout without having to go through the settings.
Notes:

### X2. Events from the file (D-ui-12)
Do: open a CSV with a text column (a mode or state); in the import dialog tick its Events box. In the descriptor form, the
same box on a field.
Expect: an event at each change of that column, named by the new value, read-only, in the Events panel (group by file).
Result: Works as expected.
Notes: What is a read-only event? How does that work?

### X3. A code widget's inputs and settings (S5.4-10, D-ui-59)
Do: Gauges pane › a code widget › its menu › "Inputs and settings…". Settings › widgets › New code widget.
Expect: a channel picker per role and the widget's parameters; the widget redraws with them; New code widget adds a working
example.
Result: The "Your widgets" button is nearly cutoff in the Gauges dialog. After adding a new widget it becomes unreachable because the dialog has no scroll option. There is seemingly no way to edit the widget?
Notes: This Code widget dialog currently sucks and needs major rework, FAIL

### X4. Explorer: channels onto a category, several categories (D-ui-09)
Do: drag channels onto a category row; Cmd/Ctrl-click and Shift-click category rows, then Enter.
Expect: the channels move into that category (Undo puts them back); Enter plots the picked categories' channels.
Result: This works, but Undo only works for the most recent channel that was dragged. For moving channels between panes undo works for several moves
Notes: Undo should be consistent across the UI

### X5. Connect: Preview a frame on a real source (S5.5-02)
Do: Connect › UDP, TCP or serial with a descriptor › Preview a frame.
Expect: it reads for one second only when clicked, then shows the latest frame's values and the frames a second.
Result: Works as expected
Notes: Nominal behavior.

### X6. Import dialog: "Also for files with this header", and the Recipes manager's Import profiles tab (S5.1-10, S5.1-12)
Do: import a CSV; open "Also for files with this header" (check sets, recipes, starter layout, prefix rule). Then Recipes
and profiles… › Import profiles.
Expect: what you pick applies to the next file with the same header; the profiles tab lists it with rename, recipe chips,
layout and delete.
Result:
Notes:

### X7. Checks: a failing live check, the folded line, Save as rule (D-check-03, D-check-04, S5.3-14)
Do: a live source with a check that starts failing; a set with a group no check can run on; a table filter › Save as rule.
Expect: the tab's badge flashes once when the check starts failing; "N checks can't run here" is one folded line; Save as
rule opens the rule with the filter as its expression.
Result:
Notes:

### X8. Renaming a saved connection (D-ui-01)
Do: start page › a saved connection › the pencil; then open it and change a field › Save changes.
Expect: the new name sticks; Save changes keeps the edit without connecting.
Result:
Notes:

## W. Built Nov 17 (the Opus run)

### W1. A packet file's types in one tab (S4.3-08)
Do: open a packet file whose descriptor has `[[packet_types]]` (two or more types).
Expect: one tab named for the file; in the Explorer the file row says "N packet types" and each type is a slot
(`flight.bin · gps`, `flight.bin · imu`) on the same clock; the first charts draw every type; Close this file closes the tab;
quitting and reopening brings the tab back with every type.
Result: Works as expected
Notes: Nominal behavior

### W2. Every file of a tab: events, checks, recipes, scale (S5.6-12, S5.6-07, D-derive-03)
Do: open `flights/flight-1.csv`, add `flight-2.csv` to the tab (right-click › Add to this tab). Add a marker on each
(select a pane of flight-2's channels before Marker at A). Open Events and Rules & checks.
Expect: both files' events in the list, each with its file's name, and chips to hide one file's; the marker tooltip names
the file; editing an event shows a File picker that moves it. Rules & checks has "Results for" with both files; the Flight
limits set runs on each. Make a derived channel with "Any file that has these channels": both files get it (`name` and
`name·2`). A recipe like `altitude - "flight-2 altitude"` works. Changing a scale in the channel editor redraws both files.
Result: All of these work
Notes: "Results for" has a dropdown box for the files, and the "rules and checks" correctly runs for each one. Not sure if maybe a tree with each file would be better so all files can be seen at once, I'm open to discussion.

### W3. Quiet interface (D-ui-10)
Do: Settings › Quiet interface on. Open two files in one tab and a table pane.
Expect: chips have no empty gap where the value goes until you point at a plot; the folder listing's slot numbers show
only when you point at a file; a table's resize grip shows only when you hover it. Calm, nothing popping under the pointer.
Result: I didn't really see anything change? UI looks fine, I'm not sure what this setting does. UPDATE - after checking with the prototype the setting makes sense in the prototype but it doesn't work right in the app
Notes: Consider deleting?

### W4. Reconnect (D-live-11)
Do: connect a TCP source (a logger, or `nc -l 9000 < frames.bin` with a descriptor) with Reconnect on; stop and start
the sender. Try "Save recordings to" with another folder.
Expect: the strip says RECONNECTING while the sender is down and LIVE again when it is back, recording into the same file;
Stop works while reconnecting; the session folder lands where you chose.
Result: Works as expected
Notes: What happens if time goes "backwards", like if a system resets it's timer? Shouldn't a reconnect still show the fields after it's been parsed? I'm not sure if it makes sense for the time axis of charts in live mode to come from the source's time if that's what they're doing now... shoudn't the X axis just be a history buffer of samples? Let's discuss this, don't make the decision on your own.

### W5. Payload plugins in the browser (D-ingest-09)
Do: in browser-only mode (`just browser-site`, served), add `plugins/template-payload/.../my_payload.wasm` in Settings ›
Plugins, keep a descriptor that names `demo-payload` for a packet type, and drop a matching file.
Expect: the file opens with the plugin's channels; without the plugin added, the note says to add it.
Result:
Notes:

---

## Other things I noticed

```
What I did: Open together on two files, and Compare (Nov 15, release build from 01c8bb4)
What I expected: pick channels from both files in the Explorer
What happened instead: the Explorer only lets me select channels for the first file
Where (screen, tab, setting): Explorer, a tab with two files
How often (always / sometimes / once):
Severity (blocker / annoying / cosmetic): high priority (owner)
```
(Agent: you confirmed it works; the confusion is that only file 1 is marked. Ledger D-ui-45 (mark every file, slot and
alignment) and D-ui-46 (Open together draws every file's starters).)

Copy this block once per thing. Don't worry about being tidy.

```
What I did: Clicked "⌄" on an open folder
What I expected: a space between "Refresh" and "New files show by themselves"
What happened instead: There was no space
Where (screen, tab, setting): on the dialog for the folder
How often (always / sometimes / once): always
Severity (blocker / annoying / cosmetic): cosmetic
```
(Agent, Nov 16: fixed, ledger D-ui-55; you confirmed it in V3.)

```
What I did: Clicked on a chart pane with two channels, dragged a channel and dropped beside to right and to the left.
What I expected: A new pane to be created the channel that was dragged, and the channel removed from the original pane.
What happened instead: The channel was "copied" from the original pane instead of "moved"
Where (screen, tab, setting): main screen
How often (always / sometimes / once): always
Severity (blocker / annoying / cosmetic): blocker
```


## Anything you want built or changed (new ideas)

New ideas go on a list for after the release unless you say they matter for v1 (`docs/post-v1.md`).

```
Idea: The import dialog for a file should have a selector for how the file should open. It should mirror the "Files opened for the first time" setting so that it has the correct default, but it should allow users to quickly adjust per files as desired.
Why it matters: It's too much much to change this per file right now.
Needed for v1? (yes / no): yes
```
(Agent, Nov 16: v1 as you said, ledger D-ui-57, not built yet.)

```
Idea: There needs to be better support for text fields, The prototype supports enumerations and plotting text fields that automatically enumerates labels
Why it matters: It's required for fields that place state transitions and enumerations as text
Needed for v1? (yes / no): yes
```
(Agent, Nov 16: v1, ledger D-ingest-12, decided Nov 16: a text column with up to 256 distinct values becomes a labelled channel. Not built yet.)

```
Idea: The file explorer context menu should have an "Open with System Editor" action to use the OS file opener for a file type
Why it matters: To verify the raw data in a file when testing the app
Needed for v1? (yes / no): yes
```
(Agent, Nov 16: built, ledger D-ui-58: Explorer's file menu › Open with its default app; you confirmed it in V3.)

```
Idea: 
Why it matters:
Needed for v1? (yes / no):
```
