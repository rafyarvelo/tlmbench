# Troubleshooting and glossary

Short answers to the questions that come up most, then the words this manual uses. If your problem isn't here, `tlmbench info <file>` on the command line often says what the core makes of a file.

## Where is the cache, and can I clear it?

The cache holds what the app learns about each file (summaries, gap indexes, check results), outside your data folder, so a read-only folder works. **Settings › Cache** shows its path and size, **Open folder** shows it in the file manager, and **Clear cache** removes the summaries of files that aren't open. They are rebuilt when a file opens again. By default it is in the system's cache folder under `tlmbench`: `~/Library/Caches/tlmbench` on a Mac. The command line shares it; `--cache` or `TLMBENCH_CACHE` points it elsewhere. **Reset app…** removes more (profiles, recipes, sets); see [Settings](14-settings.md).

## A view says "coarse"

The first time a file opens, the app draws from the whole summaries alone, so you see the picture straight away. It still shows every extreme. A moment later the exact view replaces it and the label goes. If it stays, the file is still being summarised. In an HTML export, a *coarse* chart means the export holds only summaries there: that is a Light export. See [Exporting](12-exporting.md).

## Time goes backwards in my file

Some files have a time column that steps back: a restarted logger, or a counter that wrapped. The app doesn't reorder the file and doesn't stop. It flags it: the file's note reads *Time goes backwards in N rows (first at row …)*, and the rows after the step stay in file order, each taking the latest time before it, so they stack at one instant. If that isn't what you want, fix the source, or, for a binary file, use a descriptor whose time field is a counter with wraps.

## The core can't read my file

- A binary file the app doesn't know opens the **descriptor form** on its first bytes. Describe the framing, byte order, header, time and fields, check the preview, and press **Open**. **Find frames** proposes a sync word.
- A CSV opens the import dialog the first time: check the time column and the rate.
- A file in a format with a plugin needs the plugin in your plugins folder (**Settings › Plugins** lists them and says why one didn't load).
- On the command line, exit code 2 means a file couldn't be read, and 3 a bad descriptor or argument.

## A plugin fails

**Settings › Plugins** shows the reason under the plugin. Plugins run in a sandbox with limits:

- *ran past its time limit*: a decoder gets 30 seconds of CPU for a call, an alignment plugin 2 seconds. Check the plugin for a loop.
- *needed more memory than it may use*: a decoder may use up to 1 GiB, an alignment plugin 256 MiB.
- *failed: …*: the plugin itself reported an error; the first line of it is shown.

A plugin that a folder shares doesn't load until you trust the folder (**Trust this folder's plugins**). A native plugin stays off until you turn on its **On** switch. In a browser tab only WASM plugins run. The `tlmbench plugins --check plugin.wasm file` command tries one plugin and says what it decodes or why not.

## The first open on macOS or Windows is blocked

v1 isn't notarised by Apple or signed for Windows yet, so the first open needs one extra click. [Installing TelemetryBench](../install.md) says where, for each system.

## My graphics card has no WebGL2

The app draws traces with WebGL2. Where it isn't available (a virtual machine without 3D acceleration, a remote desktop), it draws with Canvas 2D instead, with the same pictures but slower on very busy views. Nothing needs changing.

## My folder is read-only

Everything works: the app keeps its cache elsewhere. You just can't **Share…** into it. Use **Export…** or **Export file…** to write the files where you can, and see [Sharing](11-sharing.md).

## A shared layout, widget or plugin does nothing

A code widget from a folder asks before it runs (**Run code widgets from this folder?**). If you chose **Keep as value cards**, it stays a card; turn **Run code widgets** on in **Settings**. Plugins wait for **Trust this folder's plugins**.

## The exported HTML shows only a note

A folder export opened by double-click can't load its data: the browser forbids it. Serve the folder from a web server, or export **One file** instead. See [Exporting](12-exporting.md).

## Glossary

**Pyramid.** For each channel, summaries of the data at coarser and coarser steps (each holds the first, last, minimum and maximum of a block of 256, 1,024, 4,096 … samples, and the sums for exact statistics). A view reads a few thousand summaries instead of millions of samples. It adds about 11% to the raw data and lives in the cache.

**M4.** How a line is reduced to pixels: each pixel column is drawn from its first, last, minimum and maximum samples. It looks the same as drawing every sample, and no spike is hidden.

**Coarse.** A view drawn from the pyramid alone, before the exact one arrives. It keeps every extreme.

**Gap.** A time jump longer than a channel's gap factor times its median interval, or a missing value. The line breaks there (or is dashed, in **Settings**). Gaps are never interpolated.

**Slot.** A file's number in a tab (file 1, file 2…) when several files share one timeline. Layouts refer to files by slot, so they work on other recordings.

**Time map.** How one file's clock lines up with another's: an offset, or a set of matching points, found by hand, by cross-correlation, or by an alignment plugin.

**Check set.** A named list of checks and rules over channels, each with a severity (Fail or Warn). It runs in the app (**Rules & checks**), on the command line (`tlmbench check`) and in Python, with the same results.

**Recipe.** A derived channel's definition: a name, an expression over other channels, a unit. Recipes marked automatic are added to every file they apply to.

**Descriptor.** A TOML description of a binary format: framing, byte order, header, time and fields. The core decodes the file by it. A descriptor is shared in `.tlmbench/decoders/`.

**Full and Light export.** A Full HTML export carries every raw sample of every channel. A Light one carries the pyramid, with raw samples only for channels shown as tables, values or XY and around events and violations. Automatic picks Full when it fits.

**Lock.** Holds the time view (and, optionally, the layout) so a gesture can't change it. An HTML export's viewer opens Locked; press **Unlock** to move freely.
