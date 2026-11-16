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
- Browser-only mode runs WASM decoder plugins by translating the same `.wasm` component in the page (jco, fetched only
  when a plugin is used); each plugin runs in a Worker of its own, reads the file only through `File.slice`, is ended at
  the app's CPU limit, and has its memory capped at the app's limit. Ledger D-brow-09.
- Browser-only mode reads binary files through descriptors kept in the page's own storage (localStorage), found by their
  file name patterns; a binary file none names opens the descriptor form, which has no Share there. A file with several
  packet types opens a tab per type (several files in one browser tab are after v1). Ledger D-brow-08.
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
