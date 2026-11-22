# Follow-ups: design-doc notes owed

What the code settled that `docs/design.md` doesn't say yet, kept until the design is updated in the living doc and
re-exported (release plan R7, "Design doc final sync"). Agents add here; they don't edit the Claude Doc. Remove a note
once the design says it.

Open work is not listed here: every gap is a row in `docs/release/ledger.md`, and work after v1 is in `docs/post-v1.md`.
The step-by-step "Left from step …" notes this file used to hold were mapped into the ledger in review R1 (Nov 11) and
removed Nov 15; git history keeps them.

## Decisions and changes the owner made (Nov 14 and 15)

- ⌘Z and ⇧⌘Z are undo and redo; Backspace goes back a view.
- The copy key on a drag: Option or ⌘ on the Mac, Ctrl elsewhere, with a "+" on the cursor.
- Settings › Charts › Default line: a width (thin, normal, heavy) and a style for every channel.
- Each pane has its own Events toggle; E and the top bar's switch set the default for new panes.
- The top bar has an Export menu (HTML, data, image).
- Pane actions show on hover and give way to ⋯ by the pane's width; Chart and values' grip is at the pane's bottom.
- "Show only these" is an Explorer header icon; the Explorer watches its folder, with Refresh for network shares.
- Export to HTML closes as it starts, with its progress under the top bar.
- Export tiers: Save as Folder (any size, needs a web server) or One file (opens from anywhere), raw samples when they fit,
  a Size limit field (25 MB to start) and `--single-limit`. The viewer draws the app's min/max band under coarse lines.
- The native menu bar (File, Edit, View, Pane, Window, Help) and the shortcuts come from one command table.
- One reset: Reset app… with a list of the things the app remembers (unticked); Reset to defaults is gone.
- The viewer opens Locked with an Unlock pill (provenance in its tooltip), so decision 6's "Read-only view" header is the
  Locked/Unlocked pill; what a viewer or a tab can't do is left out, or disabled with the reason.
- `TLMBENCH_SLOW_BUILD=<ms>` slows a pyramid build so a person can watch it.
- Opening one file twice at once makes one tab.
- A file's right-click in the Explorer has Open with layout ▸ (your and the folder's layouts; those the file lacks
  channels for are greyed with what they need), the manual counterpart of an import profile's starter layout.
- A recipe is duplicated from a derived channel's right-click (Duplicate recipe…) and from the Edit recipe dialog
  (Save as new), besides the Recipes manager.
- A pane can have a name (Rename pane…, or a double-click on its title), saved as an optional `name` per pane in the
  layout format and carried by shared views and exports.
- A saved layout can be renamed (tab menu, Layouts panel); a shared copy is updated on the next Share….
- Open together's first layout overlays the channels the files share (a pane each) and adds each file's own starters; it
  keeps choosing Same clock when the files' times overlap, else Align starts.
- In a tab with several files: a pane's ⋯ menu has Duplicate for ▸ file N, and a channel chip's right-click has From
  file ▸ and Add from every file (channels matched by name).
- In a tab with several files: limits belong to the channel name and every file's copy shares them; a recipe scoped to
  any file with the channels gives each file its own copy; every file's events show through its alignment, labelled
  with the file, and a new event goes to file 1 unless added from another file's pane or channel.
- An unlocked viewer arrangement is not kept across reloads: a reload is the exported view.
- Parity with the prototype is classed (`docs/release/parity-classes.md`): the "App is better" rows are accepted
  differences (ledger A-01 to A-11); the ⋯ menu is trimmed to what the top bar and menu bar don't hold; the time range is
  a fixed-width field.
- Browser-only mode: v1 is raw files (CSV, `.tlm`, Parquet, fixed-width and packets through descriptors), WASM decoder
  plugins and read-only folders, with the cache in OPFS and a reload bringing files back; the parse runs in a Worker; the
  Parquet reader is a separate WASM module (about 1 MB gzipped) fetched only when a Parquet file opens. Several files,
  derived channels, events and checks in a tab are after v1 (`docs/post-v1.md`).

## Decisions the owner made (Nov 16)

- Ingestion (changes the CSV decision "a column with any non-numeric value … is text and left out"): a text column with
  up to 256 distinct values opens as a labelled channel, each text a code, shown as its text on axes (a tick per label),
  in readouts and in checks (`'Mission'`); a column with more is left out and the import dialog says why. Ledger D-ingest-12.
- Decoder plugins, "a descriptor can name a plugin for one field or packet type": a third world, `payload`, in the same
  WIT package. A `[[packet_types]]` entry with `plugin = "name"` hands the plugin each packet's payload after framing,
  checksum and decompression, and gets rows for the channels the plugin declares; a field with `plugin = "name"` and a
  `size` gets that field's bytes for many records at once and returns one value per record. Ledger D-ingest-09.
  Built Nov 16 (night): the world's exports are `describe`, `packet-schema`, `decode-packets` (a batch of packets, each
  header and payload) and `decode-field`; `params` (a TOML table on the type or field) reaches the plugin as JSON. A
  plugin type has no record layout of its own and needs `packets.length`; a plugin field has no bits, scale or
  value_offset. A plugin type decodes into the cache once, as a plugin file does; a plugin field's columns are cached
  and follow the file's own channels in the channel list. A file a payload plugin decodes needs that plugin loaded to
  open, cached or not. Settings lists such plugins as Payload. Browser-only mode doesn't run them yet (left in D-ingest-09).
- Browser-only mode runs WASM decoder plugins by translating the same `.wasm` component in the page (jco, fetched only
  when a plugin is used); each plugin runs in a Worker of its own, reads the file only through `File.slice`, is ended at
  the app's CPU limit, and has its memory capped at the app's limit. Ledger D-brow-09.
- Browser-only mode reads binary files through descriptors kept in the page's own storage (localStorage), found by their
  file name patterns; a binary file none names opens the descriptor form, which has no Share there. A file with several
  packet types opens a tab per type (several files in one browser tab are after v1). Ledger D-brow-08.
- Browser-only mode reads the folder's `.tlmbench/` through its directory handle: categories, widgets, icons and
  `decoders/` only (the rest of it is for what a tab leaves out until after v1). Ledger D-brow-10.
- Browser-only mode's WASM plugins: added in Settings › Plugins (kept in this browser), or the open folder's
  `.tlmbench/plugins/` once trusted; each runs in a Worker of its own with the app's limits (1 GiB, 30 s a call) and
  clocks at zero. Native plugins are listed as needing the app. Ledger D-brow-09.
- One Lock, the prototype's padlock, in the app and in exports (owner, Nov 16 evening, superseding that afternoon's Edit
  layout button): it holds the time view (zoom, pan, fit, Back) and the layout (moving, adding, closing, splitting,
  resizing panes and channels); cursors and table sort and filter still work (settings); L toggles it. The app saves it
  and "Also lock the layout" is on by default (the prototype's default); an export always opens locked and an unlock
  isn't kept across reloads; a `#show=` link from a check report still frames its span in a locked export. Exports of a
  tab of several files are named for the tab. Ledger D-ui-61, D-ui-62.
- Open together names the tab after its files ("flight-1 + flight-2", "flight-1 + 2 more"). In browser-only mode the
  Events panel leaves the rail. Export data… and Save image… close when done and say what they wrote, as Export to HTML… does.

## From the release review (Nov 11)

- Export checksums are BLAKE3, and the viewer verifies each file (D-cli-08).
- Checks run as their own pass after open, not in the pyramid's pass (S5.3-04).
- The check set's TOML keys are the examples' (D-check-08): `schema`, `name`, `runs_on_any_file`, `info_groups`, and
  `[[check]]` rows with `kind`, `group`, `expect`, `when`, `severity`, `category`, `grace`, `settle`, `enabled`.
- CSV `#` comment lines before the header are skipped (D-ingest-08, still Proposed in the design).
- Redaction removes a channel from checks and event channel lists too (D-cli-10).
- The CLI commands `inspect`, `record` and `plugins` exist besides the design's seven.
- The prototype suite has 29 tests, not 21; the wire format's trace header is 32 bytes at v3.
- Time going backwards is flagged and kept in file order, not an error.
- Several files in an export open as one viewer tab (groups in `crates/web`).

## Derived channels (step 5.1)

- Filters are second-order sections run forward then backward on recorded files (zero phase, 4th-order roll-off), not a
  selectable Butterworth order; a chunk reads a margin of 20 time constants past its end.
- A recipe uses another recipe by name; a channel of the same name wins. Loops, or a channel the file lacks, keep a recipe
  off that file (the CLI says why).
- A one-number recipe (`mean(x)`) is stored as a flat line. `since(cond)` is a gap until the condition first holds.
- A derived entry is keyed by the recording and each recipe's expanded expression, name, unit, category and decimals.
- A recording and its derived channels show as one channel list (`Store::join`), each part with its own pyramid.

## Checks (step 5.3)

- Results are cached in `<cache>/checks-results/` by file contents, set version, the recipes it names and, where used,
  labels and limits; sets are TOML in `<cache>/checks/`.
- A violation is one run: samples closer than 0.25 s join, and runs shorter than the grace are dropped.
- A set's badge leaves out info-only groups. `tlmbench check` exits with the highest code that applies (3, 2, 1).
- The failure category is `check` ("Check failed"). An import profile may name sets (`check_sets`).

## Widgets and pane views (step 5.4)

- "Typed updates" are cards patched field by field from stable reactive objects, not build and update functions.
- The Gauges view reads at the pointer, then the playhead, then cursor A, then the end of the view; min and max run from
  the start of the view to the reading time. `widgets_frame` is one call for every widget; `xy_points` keeps the
  neighbours that join points to the rest of the line.
- Code widgets are served from the app's own scheme and run with `sandbox="allow-scripts"`. The min/max band is a setting
  (on), drawn under reduced traces only.

## Live (step 5.5)

- Triggers run in the core on ingest over the new samples, with a warm-up for windowed expressions, and write an event in
  the Trigger category; "Make a rule…" is the same crossing (`prev(x) < L && x >= L`).
- File playback is the endpoint `replay://`; pausing a replay is a real pause, while a device's pause leaves a gap.
- A recording that isn't kept goes to the cache's `live-sessions/`; Settings › Record live sessions to sets the folder.
- Checks on a live file run afresh about every 2 s and aren't cached.

## Several files (step 5.6)

- A group's primary always has the identity map; Make primary resets the others to starts or clock.
- Same clock is the primary's map unchanged; Offset is seconds after the primary's start, on top of Align starts; Add to
  tab picks Same clock when the files' own times overlap, else Align starts.
- Keep and Undo show the result drawn first, not an overlay of both.

## Shared configuration (step 5.7)

- The Share dialog lists objects by kind as Only yours, Differs, Only in the folder or Same; new objects are shared by
  default, the folder's own are taken by default, what differs waits for a choice.
- The app reads the folder's recipes, sets, profiles, decoders and events besides its own, and yours win a name; the
  folder's categories win an id.
- A shared view's `files` carry each file's alignment (`align`: method, seconds, channel, locked, knots, pairs). Time maps
  are `.tlmbench/timemaps/<primary>--<file>.toml`.
- `tlmbench validate <folder>` lints every file and expression; `--config` replaces the nearest `.tlmbench/` (repeatable,
  later wins).

## Windows and the daemon (step 5.8)

- The daemon's lifetime in the desktop app is the process (closing the last window quits).
- A tab dropped on another window's strip finds the window by the screen position of its top 84 px.
- Finder drops open files; in-page drags are driven from the mouse in the desktop app (`dnd.ts`), with a text pill as
  the drag ghost.

## Plugins (step 5.9)

- The WASM guest exports `start`, not `open` (a C library symbol collides); `sniff` takes the first bytes and the file
  name; an estimator exports `needs(params)` and may return a `note`.
- Limits are per call: a decoder's chunk 30 s and 1 GB, an estimator 2 s and 256 MB.
- Native plugins load only from your own folder (`plugins/` in the app's data folder) and only once turned on; a folder's
  shared WASM plugins load after you trust that folder.

## Exports and the viewer (step 5.10, R2.1)

- The export folder: `export.json` (format version, what each channel holds and its size, the layout as a shared view,
  events, check results, provenance, a checksum per file) and `data/f<k>/` with `t.<j>.bin` (i64 ns, chunks of 2^20; none
  when times come from a rate), `c<n>.<j>.bin` (f64 shown values, NaN missing) and `c<n>.pyr.bin` (88-byte summaries,
  finest stored level first). Values are shown values (scale, offset and adjustments applied).
- Light picks raw channels from the panes' views (`table`, `both`, `values`, `xy`) and keeps raw windows around events
  and violations (`--detail`). `docs/viewer-store.md` describes the viewer's sparse store.
- `just viewer` packs the page into `app/viewer/viewer.pack`, which `crates/export/build.rs` embeds, so the CLI and the
  app need no files beside them.

- **Rename a layout (D-ui-44, owner Nov 15).** Tab menu Rename… and a double-click in Layouts; open tabs follow, Undo in the note. Not in the design or the prototype: design-doc note owed. A copy shared in a folder keeps its name until the next Share….
- **The ⋯ menu (D-ui-37).** Trimmed to View (the bar's buttons when narrow, links, overview), All panes and Window; New pane, Remove every channel/pane and Export are the menu bar's and the top bar's. A browser page and the viewer have no menu bar and keep New pane and Remove in ⋯.
- **Start page (D-ui-36).** A full page: three cards, then recent sessions and recordings beside live sources and layouts. Recents show their channels and length once a file has been opened (`recentInfo`, remembered). Layouts open on the file in front, so they are greyed with no file open.
- **Open with layout ▸ (D-ui-42).** On a file's right-click, as a list that replaces the menu's items (Back first). A layout that needs channels the file lacks is greyed "needs …" only when the file is the open one; a closed file's channels are unknown.
- **Every file of a tab marked in Explorer (D-ui-45).** The folder tree's rows carry the slot number and "primary" or the alignment word.
- **Duplicate recipe… and Save as new (D-derive-02).** Duplicate opens the Derive dialog prefilled as a new recipe; Save as new keeps the edited recipe and adds the changes under a "… copy" name. The colour is the channel's display setting and isn't copied.
- **Rates (D-ui-38).** "1 kHz", "daily · median interval 1 d", "median interval 2.5 s"; the table's filters show "e.g. > 1.5".

- **Time going backwards in binary files (S5.10-06).** Fixed-width, packet and plugin files no longer fail to open: the records stay in file order and each that reads earlier takes the latest time before it, with a note (as CSV and Parquet). The places are kept, not a time column (`strided::Ordering`), and a packet index remembers them (index version 2: older entries rebuild).
- **Pane names (D-ui-43).** Rename pane… in a pane's ⋯ menu or a double-click on its number or name; the name (up to 60 characters) shows before the chips, is saved with the layout (optional `name` per pane), travels with Share… and HTML exports, and is used by the overview's picker and Duplicate ("… copy").
- **Open together draws every file (D-ui-46).** A new tab gets a pane per shared channel (each holding every file's copy, spread like a starter's), then each file's own starter charts; a tab that was open already keeps its panes.
- **Duplicate for file N, From file N, Add from every file (D-ui-47, -48).** Items in the pane's ⋯ menu and the channel's right-click menu, one per other file that has the channel, rather than a submenu.
- **Limits belong to the channel's own name (D-ui-49).** `temp`, `temp·2` and every file's copy of a derived channel share them.
- Filters on live data (S5.1-06, Nov 16 night): `lowpass`, `highpass` and `bandpass` run forward only (causal) in live
  derived channels and live triggers, so they lag the signal by their group delay; each new chunk replays the filter's
  settling margin first. A recorded file's filters stay zero-phase, so a live channel and the same recipe on the saved
  file differ by that lag. A running total (`cumsum`) still can't follow a live source.
- Live packet streams (D-ingest-11, owner Nov 16): a descriptor with `[packets]` on a socket or serial port records one
  session source per packet type from one connection, each with its own card and tab named "name · type"; stopping one
  ends the connection, so all of them. Each type's `.tlm` holds its records as read (the packet header, then the record),
  so its fields keep their offsets; a payload plugin field's value is appended to each record, and a type a payload
  plugin decodes is written as the plugin's rows. A type with no packet yet when the connection opens gets its tab
  anyway, filling in as its packets come. Packets aren't read live from a tailed file or a replay.
- Checks (D-check-02, Nov 16 night): the option that makes missing values fail is per check, `missing = "fail"` in a
  set's TOML and "Missing values fail" in the check editor; it applies inside the check's When only. Rules don't have it.
- CSV text columns (D-ingest-12, built Nov 16 night): codes follow the order the file first has each text, an empty
  cell is missing, and the channel keeps the column's place among the others. The labels travel with the recording
  (cache, HTML exports, browser-only mode) and a value map the person types is applied over them. An axis gets a tick per
  text only when the text channel is alone on it. This replaces the design's "text columns are left out" (D-ingest-03).
- Import dialog and profiles (Nov 17, R2.5): the dialog has an "Opens as" choice (D-ui-57): Settings' "Files opened for
  the first time" unless you pick another starter for this import only; it is not saved. An import profile now also
  carries check sets, recipes, a starter layout and a channel prefix rule (D-check-01, S5.1-10, D-derive-01, D-share-01;
  `ImportProfile` in `crates/decode/src/profile.rs`, in a "Also for files with this header" section). None of them
  changes how a file decodes or its cache key. A recipe a profile picks applies at open as if attached to each file (a
  file can still turn it off). The prefix rule's "by pattern" is a regular expression: the first group is the prefix,
  else the whole match, else the whole name; it is a reading of "part of the name by pattern" the design leaves open.
  A file's own prefix (Channel prefix…) still wins. A starter layout comes after a file's remembered layout and before
  Settings' choice.
- Code widgets (D-gauge-03, Nov 17): Settings › Run code widgets (on by default). Off, a code widget stays a value card
  and never asks. This is the "organisation setting" as far as the app goes: a setting in `settings.json`; nothing in
  the app locks it.
- Save as rule (S5.3-14, Nov 17): the table's filter strip has it; the rule's expression is the filter's tests joined
  with `&&` and `||`; blank and not-blank tests have no rule form and are left out.
- R2.5 and R3 rows built Nov 17 (Sonnet run), notes owed to the design doc:
  - Import profile (S5.1-12): it has an optional `name` (the Recipes manager's second tab, "Import profiles": rename,
    recipe chips, a starter layout, "Use the tab in front", delete) and `panes` (how its files open, the dialog's "Opens
    as": a starter; the design's "which panes the file opens with"). Neither changes how a file decodes.
  - Recipes in a shared layout (D-cfg-07): Share embeds the recipes a layout's panes plot (`recipes` in the layout
    JSON). Opening such a layout when you lack some says so and offers "Add recipes" (names you have are left alone).
  - Renamed or moved recordings (D-cfg-04): a shared events file matched by identity under another name is offered an
    update in one click (the note's "Update the folder" rewrites only the `recording` name; nothing else in the folder).
    Layouts name their files by path and are not rewritten.
  - Saved connections (D-ui-01): renamed on the start page (the pencil), edited in the Connect dialog (Save changes).
  - Checks (D-check-03, -04): a group in which no check can run is one folded line, "N checks can't run here"; a check
    that starts failing on a live file flashes the tab's badge (1.6 s; a static outline for reduced motion).
  - Folder run (S5.3-15): goes into sub-folders; the report of a set's last run is kept in settings (`folderReports`,
    this computer) and comes back when the dialog opens for that set and folder. It is a dialog tab, not the design's
    separate tab; "re-run in the app" is the Run button.
  - Events panel (S5.2-16): "Charts follow these filters" and the category chips are per tab (kept while the app runs;
    not yet saved with the tab's layout or the session).
  - Widget text (S5.4-05): changes at most ten times a second; the last value always lands.
  - Widget editor (D-ui-59): New SVG widget and New code widget add a working example to the list.
  - "Already open" warning (S4.3-11): the import dialog says when a file in it is open in a tab and will open again.
- Icons and pins (D-gauge-06, Nov 17, owner's decision): Settings › Icons keeps SVGs of your own (`currentColor`; shared to
  `.tlmbench/icons/`, the folder's read back and listed after yours). An event category and a pin can use one. A layout (Layouts
  panel) or a recipe (Recipes manager) pinned to the top bar is one click: the layout opens, the recipe joins the file in front.
  At most eight pins; Settings › Icons lists them with their icon and Unpin.
- Owner's decisions of Nov 17, built (Opus-free run):
  - Events from the file (D-ui-12): an import profile's `event_columns` and a descriptor's `events` (field names) mark
    channels whose changes are events; the core (`events::from_channels`, command `file_events`) makes one read-only
    event of source `file` at each change of the shown value (a text channel: its text), category Mode change, at most
    20,000 per file. The import dialog has an Events box per column, the descriptor form one per field; the Events panel
    can group by file. Format metadata (Parquet, HDF5) is post-v1.
  - Code widget inputs and parameters (S5.4-10): `[[input]]` (role, label) and `[[param]]` (name, label, type, default,
    choices) in `widget.toml`; the widget menu's "Inputs and settings…" binds each role to a channel and values each
    parameter, kept per widget in the layout. The frame message adds `inputs` (per role: value, text, unit, last 64
    samples), `series` (the widget's own last 64 samples) and `params`.
  - Explorer (D-ui-09): a channel dropped on a category row moves into it (the per-file override); Cmd/Ctrl-click and
    Shift-click pick categories, and Enter plots their channels. Descriptor fields take `category`, the same word an
    import profile's column category is.
  - Connect (S5.5-02): "Preview a frame" reads a socket or port for one second (a UDP port is bound, a device opened,
    into a temporary recording that is deleted) and shows the latest frame and the frames a second; never on opening
    the dialog. A file or replay source shows its first values at once.
  - Export detail (D-cli-06): `html::window_spans` is the one place the windows are worked out (CLI and app).
- Events panel filters (S5.2-16, Nov 17): the tab's hidden categories and "Charts follow these filters" are kept in the
  folder's session (`Session.events`, by file), so they come back with the tabs. They are not part of a saved layout.
- Opus run (Nov 17), several files in a tab:
  - Packet types (S4.3-08): built as the design says (slots of one tab on one clock). The tab is named for the file,
    each slot `flight.bin · gps`, the file row "N packet types"; Close this file closes the tab. Browser-only mode keeps a
    tab per type.
  - Events of every file (S5.6-12): the Events panel adds file chips (hide a file's events; charts follow them when they
    follow the category chips; kept in the session as `Session.events[path].files`) and, on an event you added, a File
    picker that moves it to another file of the tab. A new event goes to the file whose channels the selected pane
    holds, if only one; else file 1. A marker's tooltip names its file.
  - Checks of every file (S5.6-12): each file's sets run on that file; the Rules & checks panel shows every file's results
    at once, a row per file (its slot, name and count) folding to its sets, the primary open (owner, W2 Nov 22). Violations and check events are on the tab's timeline; an
    alignment change maps them again without running.
  - Scale, offset and missing values go by channel name, so `accel_x`'s apply to every file's `accel_x` (as limits do,
    D-ui-49); a change reopens every file of the tab.
  - Quiet interface (D-ui-10): as the prototype's quiet rules, it also hides an empty chip readout (it shows while you
    point at a plot), a file's slot number in the folder listing until you point at the file (the channel tree's slot
    heads keep theirs), and a table's resize grip until hovered. The setting stays off by default (the prototype's is on).
  - Events filters and layouts (S5.2-16): a saved layout keeps its tab's hidden categories and "Charts follow these
    filters" (`events` in the layout JSON, only when not the defaults); changing them marks the layout tab unsaved. The
    follow switch is the prototype's link toggle in the chip row.
  - Live checks (S5.3-06): incremental. A live check's filters run forward (causal), as live derived channels' do, so a
    filtered check on a live source can differ from the same check on the saved file by the filter's lag. A summary
    check (one number) on a live source still reads the whole recording at each run.
  - Reconnect (D-live-11): a TCP or serial source with Reconnect (on by default in the Connect dialog; `record
    --reconnect`) connects again after its link drops, every half second until it answers or is stopped, and records
    on into the same file; frames sent meanwhile are lost and a frame cut by the drop counts as malformed. A rate-timed
    descriptor's times run on from the record count, so the gap doesn't show in time; a time field shows it. UDP has no
    link: it listens until stopped. The Connect dialog's "Save recordings to" is per connection, starting from Settings.
  - Payload plugins in browser-only mode (D-ingest-09): built as in the app, in two steps since the plugin runs in a
    Worker: the page's core lists the plugin's work, the plugin decodes it, the core reads the file with the result.
    The plugin is found by its name among the plugins added to the browser (or the trusted folder's). A file with several
    packet types still opens a tab per type in a browser tab.

## The owner's test notes (Nov 22)

- Closing a pane (D-ui-66): every other pane keeps its row and column. One of two side by side widens across its row; a
  column's next pane takes a closed top's place. A chip dropped on its own pane's edge moves to a new pane (D-ui-65).
- Your widgets (X3, owner's choice of layout Nov 22): the editor is a list of your widgets (and the folder's, read-only:
  Duplicate makes one yours) beside a form for the one picked: name, kind (SVG or code, switchable), icon, the SVG with
  its needle angle or the code with its inputs and parameters as rows, and a live preview (a code widget runs in the
  preview's sandbox). "Edit as JSON" shows the whole list as text. A widget keeps its id when renamed, so panes keep it.
  The Gauges ⋯ menu folds your widgets into "Your widgets ▸" (the one in use stays in the main list) and has "Edit this
  widget…"; Settings › Widgets › Edit… opens the editor.
- Pins (X1): a layout shared in the folder pins as well as yours (`kind = "shared"`, by its file). A layout's right-click
  menu in the Layouts panel has Open, Pin or Unpin, and for yours Rename and Delete; a pinned button's right-click has
  Open and Unpin; a pinned layout keeps its pin icon showing in the list. Settings › Icons' SVG box is full width,
  five lines, resizable, with a preview.
- One Undo (X4, owner: "Undo should be consistent across the UI"): channels moved into a category are steps of the same
  ⌘Z history as pane edits (per file in front), so ⌘Z steps back through them one by one and ⇧⌘Z redoes; the toast's
  Undo does the same as ⌘Z when the move is the last step.
- Quiet interface (W3): also the prototype's `.phead` rule: a pane's header keeps its view menu and ⋯; axis, event
  markers, duplicate, split and close are in ⋯ (they already were). The prototype's other quiet rules are the app's
  defaults with the setting off (the Explorer header in one row, a channel's unit on hover, Settings hints behind ⓘ, an
  inactive tab's ×). The setting stays off by default (the prototype's is on): the owner's call if that should change.
- Read-only events (X2, owner Nov 22): events made from the data (a file's column, a rule, a check) and the folder's
  shared ones can't be edited or deleted in the Events panel, since they are made again from the data. Their row shows
  the source as a tag (File, Rule, Check, Shared) on hover, its tooltip saying why and what to change instead; a rule's
  or check's row keeps its pencil, which opens the rule.
- Live time (W4, owner Nov 22, as recommended): a live chart's X axis stays the source's time column, so live matches the
  saved recording. When a socket source's time goes back by more than a second (a device reset its clock; a counter's
  wrap is not one), the source starts its next part (`source-N-2.tlm`…), which the app follows as after a rotation, so
  the chart keeps drawing. A smaller step back stays in the part, held at the latest time. Tailed files and replays keep
  their times as written.
- The Report tab (S5.3-15, owner Nov 17, built Nov 22): Rules & checks › Run on folder… opens a Report tab in the tab
  strip, one per set, on the open folder (no file is in front while it is). It has the set's grid (files × groups) and
  its pass rates and trends: sort by check, group or pass rate (worst first), filter by group, and a trend per check in
  date order, a summary check's value as a line (else its worst; else the verdicts), each point a file that opens on a
  click. Run again runs it; the report is kept with the set (settings, as before) and the tab with the folder's session.
  The tab menu shows only Close. The prototype's dialog is gone.
- Traces on canvas (S5.4-06, owner Nov 17, built Nov 22): the sparkline and the XY dot's trail and cloud (and so the GPS
  track) draw on a canvas under the widget's SVG; text, grid, marks and the dot stay SVG, and dials stay SVG. The
  canvas is sized in device pixels and reads the theme's colours at each draw. Code widgets and SVG widgets of your own
  are unchanged.

## Tables and XY across files, R3.4 (Nov 22, second run)

- A table in a tab of several files sorts, filters, steps through matches and applies to charts by any file's column
  (S5.6-13). Its rows are still the primary's samples; another file's column is read at each row's time, held from that
  file's last sample at or before it (blank before its first), as the table shows it. A filter on such a column passes
  or fails the primary's rows; Apply to charts leaves the primary's rows out.
- An XY plot pairs channels of different files on the shared timeline (S5.6-13): the pairs follow the X channel's file's
  samples, and a Y channel of another file is held at each of them, so nothing is resampled and the X file's rate
  decides how many points there are. Before another file's first sample there are no pairs. The HTML viewer pairs the
  same way.
- A layout's rename and delete are steps of the one ⌘Z history (R3.4). They belong to no file, so they stay when the tab
  in front changes; undoing a delete puts the layout back, its tabs stay closed.
