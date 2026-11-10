# Follow-ups

Work that is planned but deliberately not done yet. Remove an item when it lands.

## Large test recordings (10 GB and 100 GB)

The design's testing section calls for synthetic recordings at 1 GB, 10 GB and 100 GB (sine + noise + spikes + gaps).
Phase 1 needs only the 1 GB set (100M points as fixed-width binary is about 0.4–1.2 GB). The larger sets wait for an
external SSD: the development Mac has about 24 GB free.

When needed:

- The generator (added in Phase 1) is deterministic from a seed, so recordings are rebuilt, never stored or committed.
- Write them outside the repo to a folder named by `TLMBENCH_DATA` (default `~/tlmbench-data`); point it at the
  external drive for 10 GB and 100 GB. Add `just gen <size>`.
- 10 GB serves the first-paint target and the CLI gate (`convert` and `check` at disk-read speed, flat memory);
  100 GB serves `convert`'s flat-memory claim. Neither suits CI runners (about 14 GB free); run them on the Mac.
- Add those folders to `ro_commands` coverage, mounted read-only (`hdiutil` image or a read-only external volume).

## Left from step 5.5 (Nov 9)

- **Measure first:** `just gate3 app` passes on the Mac with these changes; a real Mac run of file playback (`replay://` of a CSV and a .tlm), a trigger and live
  checks against a real TCP or UDP source, and a look at the faint trace and the Sources cards with a screenshot.
- **Derived channels on a live file (built; causal filters left):** Derive… on a live tab gives ƒ rows that follow the source
  (`Store::open_live_derived`: each batch of new samples is computed after a warm-up of the samples its windows need,
  appended to a recording of its own that is followed as it grows, and joined to the live one; the growing pyramids join
  side by side). What can't be rebuilt from a bounded look-back is skipped with the reason: running totals, one-number
  functions and the zero-phase filters (`lowpass`, `highpass`, `bandpass`). A causal (one-pass) form of the filters for
  live data, which carries state from one batch to the next, is not built; a trigger on a filtered channel is refused
  for the same reason. The derived part is recomputed from the start when the source's recording rotates into a new
  part (the derived channels go until Derive… is applied again). Automatic recipes are not applied when a source connects.
- **Add source:** it joins the session's folder (`source-2.tlm` next to `source-1.tlm`) and opens in a tab of its own, with
  its own clock from its own first sample; one tab holding several live files with `·2` channels and one session clock
  is 5.6 (numbered slots and time maps). Until then "N sources" counts live tabs and the cards list every live tab,
  not the sources of one session.
- **Triggers:** only on the file of the tab in front (one trigger per live file); not in the CLI or Python yet (a
  `tlmbench record --trigger` that follows its own recording with `Live::refresh` is the natural place); an expression
  that needs the whole recording (running totals, filters that start at the beginning, one-number functions) is
  refused; a crossing between the last sample before arming and the first after is not seen; the window uses the
  recording's average rate for time windows (`rolling_mean(x, 50ms)`), so a live source with big gaps warms up a
  little short; the trigger's events are kept in the cache against the live file's key and go with the session.
- **Live checks:** each run reads the whole recording so far (about every 2 s), so a long session costs more each
  time; running them over the changed tail only (rules and sample checks are streams; summary checks and `When`
  scopes need the whole) is the next step. The interval is fixed at 2 s, not a setting. Checks flicker the "checking"
  state every run.
- **Pause:** a device's pause lets frames go by (they are not read), so the recording has a gap, as designed; a tailed
  file's pause loses the rows written meanwhile too.
- **Connect dialog:** the forms are the app's (a port or address, a descriptor file); the prototype's framing, timestamp
  source, multicast group, reconnect and poll interval are not built (the core has no such options yet); the preview shows
  fields from the descriptor, not decoded values from the stream; "Start at" is a number of seconds, not the prototype's
  list of suggested starts.
- **Start page:** saved connections have no rename or edit (open one, change it, save again as new); no layouts list yet.
- **Design notes for `docs/design.md`:** triggers run in the core on ingest, over the new samples with a warm-up for
  windowed expressions, and write an event each time (category Trigger, source rule); "Make a rule…" is the same crossing
  as `prev(x) < L && x >= L`, so it also marks a recorded file; File playback is the endpoint `replay://`; pausing
  a replay is a real pause while a device's leaves a gap; a recording that isn't kept goes to the cache's `live-sessions/`
  and is removed with its source; `Settings › Record live sessions to` sets the folder; checks on a live file run
  afresh about every 2 s and aren't cached.

## Left from step 5.4 (Nov 9)

- **Measure first:** the budget of 100 widgets in under 2 ms a frame, in the release app on the Mac (the headless
  browser gives a median of 2.2 ms for the page's part, `t_perf_gauges`); a visual check of the Gauges view, the XY plot
  and the min/max band with a spiky channel on the Mac; a check that the `tlmwidget` scheme works in the real webviews
  (macOS and Linux `tlmwidget://localhost/host.html`, Windows `http://tlmwidget.localhost/host.html`; the app's policy
  now has `frame-src` for both, and the page carries its own `default-src 'none'` with scripts and eval).
- **Widgets, left:** the half-per-frame turns when a frame of widgets takes over 4 ms (nothing measures it yet); dragging a
  widget's title to reorder it or to move its channel to another pane (the prototype has both); flags naming their bits
  (they read "bit n"; the names come with descriptors that name bits in the app); the Widgets tour layout; a widget's
  hover tooltip.
- **Code widgets, left:** the folder format (`widget.toml` with a name, icon, inputs by role such as a pitch/roll pair, and
  parameters, plus the entry) and reading widgets from `.tlmbench/widgets/` come with Share (5.7); until then a
  widget is inline JSON in settings with `shared` and `by` naming where it came from; code from `shared` asks once per
  folder, remembered in `trustedWidgetSources`; an organisation setting to turn code widgets off; HTML exports drop the
  code and show value cards (5.10).
- **XY, left:** channels of one file share their times, so pairing is by sample; pairing slower channels (a recording
  with several rates, 4.3 and 5.6) needs the time maps; the range of a very long span is a pass over it before the
  reduction pass (the pyramid's statistics would spare it for a view of whole summaries); XY inside a Gauges card
  (the XY dot) asks the core each frame for its trail.
- **Pane views, left:** parity scenes for Gauges, XY, the widget editor and the trust prompt; the Chart and values view puts
  its tiles under the chart (as the prototype); a vertical split with tiles beside the chart is not built.
- **Design notes for `docs/design.md`:** the app's "typed updates" are cards patched field by field from stable reactive
  objects (a frame writes only what changed, so only what reads it wakes: a needle's angle, a number's text) instead
  of build and update functions; the Gauges view reads at the pointer, then the playhead, then cursor A, then the end of
  the view; min and max run from the start of the view to the reading time; `widgets_frame` is one call for every widget
  (value, min and max, run start, trace); `xy_points` takes the zoom and the grid and keeps the neighbours that connect
  points to the rest of the line; yours widgets are kept in settings until Share; code widgets are served from the app's
  own scheme (the app's policy would stop their script) and run with `sandbox="allow-scripts"`; the min/max band is a
  setting (on) and only reduced traces have one.

## Left from step 5.3 (Nov 9)

- **Checks, left:** the gauges' "✗ check" while the playhead is inside a violation (the data is `violationAt` in
  `checks.svelte.ts`; it waits for the Gauges view, 5.4); the option that makes missing values fail a check; "Check
  against limits…" in a sets menu (the editor's + From limits is built); the "files with this import profile"
  attachment in the app (the profile's `check_sets` and `--sets-from-profile` exist in the core and CLI, the import
  dialog doesn't offer it); Share of sets to `.tlmbench/checks/` and of event rules (5.7); the folder run of
  sub-folders (Explorer's folder, one level for now) and its grid cell opening at the first violation of a
  *group* (it picks the earliest first violation of its checks); live files running checks every few seconds (5.5);
  the HTML report's cells linking to an exported view at the violation (5.10); parity scenes for the panel, the
  editor and the folder dialog.
- **Speed:** `tlmbench check` of three rows (two sample checks, one scoped, and a summary) over 100M samples takes
  3.0 s from a built pyramid's cache against 1.25 s to build the pyramid: it streams each row on one thread. Next, if
  wanted: rows in parallel (they are independent), and parallel chunks for a sample check whose scope is none.
- **Design notes for `docs/design.md`:** checks run as their own pass over the data after open (not in the pyramid's
  pass), cached by file contents, set version, recipes it names and, for expressions that use them, labels and limits;
  results are in `<cache>/checks-results/`, sets are TOML files in `<cache>/checks/` (the format of the examples:
  `schema`, `name`, `runs_on_any_file`, `info_groups`, `[[check]]` rows with `kind`, `group`, `expect`, `when`,
  `severity`, `category`, `grace`, `settle`, `enabled`); a violation is one run: samples closer than 0.25 s join and
  runs shorter than the grace are dropped (an instant for a one-sample rule event); a set's badge and line leave out
  info-only groups; the exit code of `tlmbench check` is the highest that applies (3 configuration, 2 can't run or
  unreadable, 1 failed); the category of failures is `check` ("Check failed"); recipes are read by name where a
  channel is missing (a quoted name); an import profile may name sets (`check_sets`); two files with the same
  contents share cache entries, and parallel runs on them no longer collide on temporary folders.

## Left from step 5.2 (Nov 9)

- **Events, left:** events detected in the file (rule and check events are built in 5.3); Share to `.tlmbench/events/` and
  `categories.toml` (5.7); a marker's tooltip on hover (the pointer readout names the event for now); a separate Event
  column and channel-cell tints in tables (needs events that list channels); Stop at for several categories at once;
  the Events panel's file chips and "Charts follow these filters" link (one file per tab for now); events of the live
  tab; the parity scenes for the Events panel and the channel editor (`just app-parity`).


- **Scale, offset and missing values (built).** `tlmbench_core::adjust::Adjusted` wraps a decoder's `Channels`: it
  composes the user's scale and offset with the format's and reads missing samples as NaN, and its layout id carries
  them so each set has its own pyramid. `Store::with_adjust` takes them by channel name (a "missing when" condition is
  evaluated once into a mask, against the file opened plain); the app's `open` and `adjust_apply` pass them, and the
  UI reopens every open file when they change. Left: a missing channel's scan runs at open (about one pass per
  adjusted channel); `tlmbench` CLI flags for them (`--scale`, `--missing`); a descriptor's own missing values;
  "Missing when" can't use derived channels; stored values (before scale) are what the list and range compare to.
- **Axis ticks per value** for a channel with value labels (the prototype's axis shows one tick per labelled value).
- **Line width, dashed and steps are saved and drawn, but headless test browsers draw lines as one pixel and solid**,
  so they are only checked by hand on the Mac.

## Left from step 5.1 (Nov 8)

- **Derived channels: the budget for stateful recipes.** The roadmap asks a derived channel over 100M samples to
  evaluate at least as fast as the pyramid builds. `just derived-bench` (100M samples, written as f64): element-wise
  recipes 0.7 s against a 0.8 s pyramid build (they run in parallel chunks and are bound by the write); rolling
  windows over samples (mean, std, min, max, sum, `median`) 0.5–0.9 s: they also run in parallel, each chunk replaying
  the samples before it (exact; a chunk whose window would reach back past the replay because of gaps sends the whole
  recipe the sequential way). Still sequential, because each carries state: running totals and differences
  1.0–1.1 s, rolling windows by time 1.5 s, a zero-phase low-pass 1.7 s, a z-score (two passes) 2.1 s. All within 2×
  of the pyramid (1.7 s) except the z-score, just over, and the low-pass at the line. Next, if wanted: replay for
  windows by time; two passes of a one-number recipe in one read.
- **App half of 5.1, left:** the Recipes manager's import-profiles tab and a profile's own recipes (the import
  dialog's Derived section, "Files with this import profile" in Derive…); Share writing recipes to
  `.tlmbench/recipes/` and a shared layout embedding the recipes it plots; Settings → recipes (5.2 builds Settings'
  channel list); the parity scenes for the Derive, Recipes and Spectrum dialogs (`just app-parity`); the spectrum's
  frequency axis zoom and a log frequency option; recipes edited in a window that has the file open reaching other
  windows (multiple windows are 5.10); recipes using channels of another file (5.6).
- A one-sample recording's `duration()` is 0 (the interval after the last sample is taken from the one before it).
- Value labels and limits (`state == 'flight'`, `past_warn`) reach the engine through `RecEnv::with_labels` and
  `with_limits`; the app has no labels or limits until step 5.2.
- Causal filters for live data and the recipes' live evaluation are step 5.5.
- Cross-file names (`Close·2`, a file's prefix) and `interp` across files are step 5.6.
- Import profiles carry their recipes in the app (the descriptor's `recipes` list); the core's profile has no field
  for them yet, and `--derived` takes the recipes marked automatic.

## Step 5.1: for the design doc (Nov 8)

Behaviour the core settled that `docs/design.md` doesn't say yet:

- Filters are second-order sections run forward then backward on recorded files (zero phase, 4th-order roll-off,
  as the prototype), not a selectable Butterworth order. A chunk is filtered with a margin past its end of 20 time
  constants, so chunked and whole-file results agree to 1e-6.
- A recipe uses another recipe by its name; a channel of the same name wins. Recipes that use each other in a loop,
  or need a channel the file lacks, don't apply to the file (the CLI says why on stderr; `--expr` is an error).
- A one-number recipe (`mean(x)`) is stored as a flat line; its value is any sample.
- `since(cond)` is a gap until the condition first holds.
- A derived entry is keyed by the recording and each recipe's expanded expression, name, unit, category and
  decimals, so changing a recipe rebuilds only the entries that use it.
- The app shows a recording and its derived channels as one channel list (`Store::join`): the recording's channels,
  then the derived ones, each part with its own pyramid.

## Left from step 4.1 (Nov 8)

- **Parity harness differences left** (`just app-parity`, 157 over 16 scenes): most are how the harness reads the two
  pages, not what they show: the prototype's icon buttons are `font-size: 0`, its menus pad an inner element and
  its selected pane is a `box-shadow` where the app's is a border; item counts differ with the content (the app
  has fewer layouts, events and starters until steps 4.4, 5.2 and 5.4). Left to do for real: the start page's
  layout (the prototype's two columns of sessions, recordings and live sources), Explorer's channel rows (a 24 px
  tick button, 500 weight), the filter's styles on its input rather than a wrapper, the pane header's 4 px gap,
  and the remaining menu widths (380 px tab and + menus).
- Settings → Record live sessions to (the folder for live recordings) is step 5.5, with the Connect dialog's new
  fields.

## Left from step 4.2 (so far)

- **Masked views read every sample in view.** With Apply to charts on, charts are reduced from raw samples (the
  pyramid can't tell which of its samples are left). Stats are exact from the pyramid over each kept stretch. A
  whole-file view of a very large recording with Apply on is slow; answering buckets that touch no excluded run from
  the pyramid would fix it.
- **A mask belongs to the file, not the tab.** Two tabs of one file share it (the tab model keeps one file per
  tab today). Revisit if tabs ever hold different filters for one file.
- **Save as rule** (filter strip) waits for Event rules (step 5).
- **Slower channels hold, dimmed** waits for files with several rates (packet types, step 4.3): within one
  recording every channel shares its times, so a table has nothing to hold.
- **Panes of values keep their view** under the top bar's Charts, Tables, Both; a new pane doesn't follow the last
  choice (the prototype's `newMode`).
- **Copy as CSV** takes at most 100,000 rows, time as nanoseconds (the owner accepted the recommendation, Nov 8). A
  bigger copy would need a save-to-file path (an export).

## Step 4.3 (import and formats in the app), Nov 8

- **Decided by the owner:** a profile is keyed by the header's sorted column names; it applies without asking only for
  an exact match, a header sharing 60% or more of its names is offered in the dialog and never applied unasked;
  "Explorer only" keeps the settings and later opens skip the dialog (Import settings… in the file menu shows them).
- **Left in the dialog:** the prototype's Panes, Channel prefix and Derived channels sections (5.6, 5.1), text columns
  as labelled states (5.2: the core has no text channels, so Text leaves the column out), "All files in one new tab"
  and "Add to the current tab" (shown, disabled: they need several files in one tab, 5.6), the profile's saved layout.
- **Parity harness artefacts in the `import` scene:** the section heading count (the prototype has "When it opens"
  and "Derived channels"), the last field label and select (the prototype's last row holds several fields, ours one),
  the column name and input widths (ours has a Category column), the note size.
- **Done Nov 8:** the descriptor form, bitfields, counter time bases with wraparound, packet types as a tab per type (owner, Nov 8: until 5.6 puts several recordings in one tab), `t_descriptor` and `t_packets`.
- **Still open:** slower channels held in tables (dimmed where unchanged, gaps flagged) need one tab with several rates, so they wait for 5.6. A session remembers a packet file by its path, so restoring it opens every type and keeps only one view for the file. The form doesn't edit `[[packet_types]]` (it keeps them; edit the TOML). The counter's wrap positions are found when a fixed-width file opens (one pass over the counter, like the time check already made) and are kept in the packet index for packet files; a 10 GB fixed-width file with a counter hasn't been timed.
- **Parity harness artefacts in the `descriptor` scene:** the field name's width (the app's added Bits column) and the TOML's height (the fake core writes a short TOML); the hex bytes differ (the prototype's data is random). Fixed: the Find frames link's colour and padding, the × on field rows (hover only, 26 px).


## Phase 1 loose ends

- The CI image doesn't include the app UI's npm packages; the Linux job installs them on each run.

## Phase 2 loose ends

- An exact whole view of a cold 10 GB, 20-channel file takes 1.7-2.3 s (the coarse view draws in 0.22 s). Most of it
  is level 4 at every pixel edge; storing each pixel edge's straddling extremes differently, or a finer
  coarse-to-exact handoff (only the pixels whose edges matter), could cut it.
- A writer thread for pyramid builds was tried and left out: writes take about 0.6 s of a 5–6 s cold build of the
  10 GB file (the CLI, M1, `ci/evict.py` before each run), and overlapping them with the reads gave 5.4–7.5 s
  against 5.7–6.6 s without, within run-to-run noise: reads and writes share the one SSD. Gate 2's first-ever
  build took 9.9 s in its last run (12.5 s before), against 9.5 s to read the file cold.


## Phase 3 (done Nov 2, 2026): what was left for later

- Play's "Stop at" events waits for events in the app (Phase 5's rules and events).
- The pane view switch (chart, table, both, values) and "Axis and scale…" wait for tables and axis settings (Phase 4).
- Live views are whole views at 30 Hz: the design's incremental path (the core sends only the buckets that changed,
  the UI appends them with bufferSubData) would cut the core's work and the replies' size.
- A live session holds one source in the app; "+ Add source" (several sources on one time axis, named `·2`) and
  the session scrubber's faint overview trace are still to come, as is saving a connection to the start page.
- A recorded session isn't listed on a start page yet (the app has none); its files open like any recording.

## Phase 3 loose ends

- A UDP source reads one datagram per call, which costs 45% of a core at 4 × 1 MHz in 8 KB datagrams (TCP: 3.7%).
  Reading several per call (`recvmmsg` on Linux) would cut it; it is reported, not gated.
- A tail polls every 20 ms, so a truncation in place loses what was written in the last moments before it (about
  20 ms). Watching the file (kqueue, inotify) would shorten that window; renaming rotations lose nothing.

## Phase 4 (in progress): where it stands

Decisions are in the design's Roadmap and Data ingestion decisions (Gate 4 = a format matrix plus a 10 GB compressed
packet file; exports CSV, Parquet, Arrow, MAT/HDF5; `[packets]` keys; packet types open one recording each; records
read as header then record). Done and pushed:

- Parquet input through the column cache (`crates/decode/src/parquet.rs`), with a page-header check and a panic guard
  against damaged files (`parquet_pages.rs`, fuzz target `parquet`).
- `tlmbench convert` to CSV, Parquet, Arrow IPC, HDF5 and MAT v7.3 (`crates/export`): wide or long, `--resample`,
  `--time`, `--raw`, channel globs, a span, `<name>.channels.csv`. The CSV reader skips leading `#` lines (Proposed).
- Packet files: `[packets]` (sync, optional length, checksum, zstd/lz4/gzip), a packet index in the cache, packets
  decoded on demand (`crates/decode/src/packets.rs`, fuzz target `packets`); `tlmbench-gen --format zstd,lz4,gzip`.
- `[[packet_types]]` (`[packets] id`, one recording per type; `crates/decode/src/packets.rs`, `Store::open_all`,
  CLI `--type`, `convert` writes `<name>.<type>.<ext>`). The prototype's form doesn't know types yet.
- The descriptor form, drafted in the prototype (`t_descriptor`) for the owner's review; three Proposed decisions
  (the form, `files = [...]` name patterns, packets without a length).

Next: these items are now steps 4.2–4.4 of `docs/roadmap.md`, which orders all remaining work (the descriptor form
and Export were accepted Nov 7). The detail below stays for reference:

1. The app's slots for packet types (one recording per type, `flight.bin · gps`): with the descriptor port in step 2,
   since the app doesn't open descriptor files yet. Done in the core: `[[packet_types]]`, `Store::open_all`/`open_part`,
   CLI `--type`, `info` and `convert` per type.
2. Once the owner accepts the form: port it to the app (descriptor lookup by `files` pattern in the app cache and
   `.tlmbench/decoders/`, the form on an unclaimed binary file, Open through the descriptor); the CLI finds
   descriptors the same way when `--descriptor` is left out.
3. The pane view switch and tables: done are the view menu (Chart, Table, Chart and table, Values), value tiles, and
   table panes over the `rows` command (a window of raw rows read as the table scrolls; row click places A, Shift B,
   double-click centres; Go to A/B; resizable columns and the table's height saved with the layout). Left for
   tables: sort and filter in Rust (a row-id list the table pages through), the Columns list, Autofit, pinned
   columns, copy as CSV, and the rows on screen as a band on the charts. "Axis and scale…" is done (a unit's axis
   each, left and right; range, decimals, label, log).
4. Workspaces (design: Files, workspaces and sharing). Done: your layouts in app data, by channel name (the
   `.tlmview.json` shape): Save as… / Save layout / Revert / Delete (Undo) in a tab's menu, ⌘S, the + menu (empty
   layout, your layouts), workspace tabs on the file in front, restored with the session. Shared views: the + menu
   lists `.tlmbench/views/` of Explorer's folder (opening one opens its file, layout and time range, read-only);
   Share with this folder… / Update shared copy…; Export file… (also offered when the folder can't be written).
   Left: the `.tlmbench` entry at the top of Explorer's tree, the Layouts sidebar, and slots once tabs hold several
   files (a layout then asks for a file per empty slot; shared views open all their files).
5. The app's Export (visible range or selection → the `convert` writers; PNG/SVG of the view): drafted in the
   prototype (`t_export`) as a Proposed decision; port it once the owner accepts it.
6. `just gate4`: the format matrix (one recording in every format with identical per-channel checksums and views;
   `convert` round trips) and the 10 GB zstd packet file (cached reopen, whole view < 1 s). The generator needs a
   mode whose values every format holds exactly (e.g. multiples of 1/512, no NaN gaps for the i16 file).

Known: Linux CI's `t_widgets` failed twice on "the size is kept" and then passed (not reproduced locally, even with
4× CPU throttling). Live sources and `.tlm` headers refuse `[packets]` for now.

## Phase 3 shell port: what is left

The app's UI port of the prototype's shell is done except these (rows in `docs/parity-audit.md`):

- Needs the core: Draw every sample (the core decides today), the live strip's session name and PAUSED/STOPPED, Trigger,
  + Add source, the Connect dialog's Name and recording switch, Record live sessions to, Measure's Missing (count and
  duration).
- Needs a decision: the Overview as a min/max band per channel with a midline (it draws each channel's trace
  scaled to its own range today), per-folder sessions, Samples and Wall clock time formats, dragging a panel to
  reorder it or move it alone, selecting several files in Explorer, the status bar's memory and load figures.
- Phase 4 and 5 rows that touch the shell (the `.tlmbench` entry, the Layouts sidebar, Export in ⋯, the tab menu's
  header and status, event markers) stay with their phases.

## Left from step 5.6 (several files and alignment)

How it works now: a tab's files are a store `Group`, one `Opened` per file with its own pyramid and a time map (knots)
to the tab's timeline. Queries go through the inverse and answers come back through the map, so changing an alignment
rebuilds nothing. Channels are numbered slot after slot (`accel_x·2`); the UI shows a file's short name (`gps · lat`,
Settings `fileLabel`) and the prefix you set. The app keeps the maps in the session (`Session.groups`, with the knots),
not yet per file pair in the cache.

- **Tables.** A table with another file's columns shows them held between their samples (rows are file 1's). Sort,
  filter, Apply to charts and the filter's hit stepping work on file 1's columns only: the core says so. Dimming the
  held values and flagging gaps (4.3) is not built. A table of file 2's columns alone shows file 1's rows.
- **XY** needs both channels in one file (it says so); an XY across files needs interpolation.
- **Events, checks, rules, the spectrum and Adjust** (scale, offset, missing) act on the primary only. Events of other
  files are not read through their maps yet (the design: events are stored in a file's own clock and read through the map).
- **Alignment.** Event matching pairs the events you added by name and order (rule events are not offered). Cross-correlation
  and Time channel pick a channel both files name alike (Automatic: the first); the declared time-source channel of a
  file (`gps_tow`) is not detected, and a time channel is read as seconds. Sync points come from cursors A and B (no
  click-in-both-files picker yet). The residuals plot has no axes.
- **Sharing.** Time maps are not written to `.tlmbench/timemaps/` (5.7). A shared layout does not carry the method and knots.
- **Recipes across files.** Names are `"Close·2"` (quoted); a file's prefix names (`MS Close`) and `2:accel_x` do not
  resolve, and an automatic recipe does not reach other files' channels. Derived channels of a non-primary file are
  listed but their recipes can only be edited in a tab of that file.
- **Packet types** of one file are still a tab each (they could be slots of one tab).
- **Live sources as slots:** a tab of live sources is a group whose members are re-read from the newest snapshot on every query (`Files::group`); each source keeps its own card, health, pause and stop, the strip's readouts follow the first, Remove closes one source and the last closes the tab. Triggers, live checks and live derived channels act on the first source.
- **Design to absorb:** a group's primary always has the identity map (Make primary resets the others to starts or
  clock); Same clock is the primary's map applied unchanged (files on one wall clock); Offset is seconds after the
  primary's start, on top of Align starts; Add to tab picks Same clock when the files' own times overlap, else Align
  starts; Keep and Undo show the result drawn first rather than an overlay of both.

## Left from step 5.7 (shared configuration)

How it works now: `tlmbench_store::config` turns the cache (recipes, check sets, decoders, time maps are already files in
the shared formats; profiles are JSON there and TOML shared) and the app's settings (categories, widgets, layouts,
passed in from the UI) into items, compares them with a folder's `.tlmbench/`, and writes either way. The app reads the
nearest `.tlmbench/` besides its cache (`config_use`): recipes, check sets, import profiles, decoders, events,
categories and widgets. The app mirrors its settings-held objects into the cache (`own.json`) so `tlmbench config export`
can write them too.

- **Python bindings** read the nearest `.tlmbench/` or `--config` when they exist (5.10); the CLI does it now for recipes,
  check sets, decoders and import profiles (`inspect` and `check --sets-from-profile` find the folder's shared profile
  when the cache has none; `--profile x.toml` takes a shared profile file). Opening a CSV with `--derived` or `convert`
  still applies only a profile given with `--profile`: the CLI never picked one up from the cache.
- **Icons:** `.tlmbench/icons/` is read, compared and written by the core and Share, and a category carries an optional
  `icon` name in `categories.toml`, but nothing in the app creates or shows an icon (the design has them for categories,
  widget menus and toolbar buttons that run a recipe or layout). The toolbar buttons need the toolbar to take
  user items.
- **Widgets:** `widget.toml` holds name, kind (`svg` or `code`), entry, icon, angle and who shared it. Inputs by role
  (a pitch/roll pair) and parameters are not in the format yet. A widget read from the folder is not in settings until
  Use the folder's takes it (it is listed and runs from the folder; code asks once per folder).
- **Events:** the folder's events show read-only (source `shared`); Use the folder's copies them into your own so you can
  edit them. Share merges by id and keeps the folder's events you lack. Deleting an event you shared doesn't remove it from
  the folder's file (a person edits the file, or deletes it in git). The CLI's `config export` writes events only for the
  recordings the app had open the last time it saved (`own.json`). "Offer to update the references when a recording is
  renamed": events are matched by identity, then by name, and Share writes the current name and identity; there is no
  separate prompt.
- **Conflicts:** a row that differs from the folder's has no base to tell who changed it, so Differs waits for a choice
  (Share mine, Use the folder's, Leave). A three-way view of two edits to one object is a git merge of the small file.
- **Profiles:** a profile file is named from the first header names and a short key (`time-price-volume-3f2a9c.toml`);
  profiles carry no name of their own yet, so the dialog lists them by header. A profile's recipes and starter layout
  (4.3, 5.1) are not part of the file.
- **Decoders:** the app cache's `decoders/` shares through the same dialog; the descriptor form's own Share… still
  writes to the file's folder. Descriptors found in the folder's `.tlmbench/decoders/` now come from the nearest one
  above a file, not only the file's own folder.
- **Time maps:** kept per pair in the cache as soon as an alignment changes (`timemap_save`, debounced) and found when
  a file is added to a tab (`timemap_find`: yours, then the folder's; by identity, else by relative paths). A shared view
  carries the same alignment in its `files`; the time map file is the pair's own record. Several tabs aligning the same
  pair differently overwrite one another (the last one wins).
- **Read-only check:** a folder is called read-only from its permission bits; a share that fails for another reason
  (a network share that refuses writes) turns the dialog into Export… after the failure.
- **Not verified:** the dialog in the real app on a read-only network share; `just app-parity` (a parity scene for the
  Share dialog is not added: the dialog's rows come from the core, the fake core's rows differ).
- **Design to absorb:** the Share dialog lists objects by kind with Only yours, Differs, Only in the folder and Same;
  new objects are shared by default, the folder's own are taken by default, what differs waits for a choice. The app
  reads the folder's recipes, sets, profiles, decoders and events besides its own and **yours win a name** (a recipe you
  edited from the folder's stays yours; Share makes the folder's the same), while the folder's **categories** win an id.
  A layout compares equal to the shared view when its panes match (the view's `by`, `files` and `view` are kept when
  the layout is written over it). A shared view's `files` carry each other file's alignment (`align`: method, seconds,
  channel, locked, knots, pairs); the knots are the map, the method says how to make it again. Time maps are
  `.tlmbench/timemaps/<primary>--<file>.toml` with `primary`, `file`, their identities, `method`, `knots` and
  `pairs`. `tlmbench validate <folder>` lints every file and every expression in recipes and checks; `--config` replaces
  the nearest `.tlmbench/` (repeatable, later wins).
