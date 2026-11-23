# Getting started

This chapter takes you from a fresh install to your first look at a recording: where to open it from, how to zoom and read values, and where the app keeps what it learns about your files. TelemetryBench reads your data and never changes it, so a read-only folder or network share works as well as a local disk.

## Install

Download the app for your system from the Releases page and follow [Installing TelemetryBench](../install.md). Version 1 is not notarised by Apple or signed for Windows yet, so the first open needs one extra click on each system. The install page says where.

## First open

When the app starts it shows the start page. It is also what you see with no file open, and the logo at the top of the icon rail brings it back at any time (**Start page**). **Esc** closes it.

<!-- shot: start-page — the start page with three cards, Recent sessions and Recent recordings -->

The page has three cards:

- **Open folder…** A folder of runs: CSV, Parquet, `.tlm`, or binary logs with a descriptor.
- **Open recording…** (⌘O / Ctrl+O) One file, in its own tab with a starter layout.
- **Connect live source…** Serial, UDP, TCP, or a file being written. Live sources have their own chapter.

Under the cards:

- **Recent sessions** lists folders you worked in, with the files that were open. A session is a folder plus your tabs, open files, time range and cursors. Click one to pick up where you left off. Each folder keeps its own.
- **Recent recordings** lists single files, with their folder, channel count and length once you have opened them.
- **Live sources** and **Layouts** list your saved connections and layouts. A layout opens on the file in front, so open a recording first.

The switch **Show this page at launch** at the bottom turns the page off at start.

## Open your first recording

1. Press ⌘O / Ctrl+O, or click **Open recording…** on the start page.
2. Choose a CSV, TSV, Parquet or `.tlm` file.
3. For a CSV with a header the app has not seen before, the import dialog opens first. Check the time column and the channel names, then click **Open**. [Opening data](03-opening-data.md) explains each option. Later files with the same header open straight away with the same settings.
4. The file opens in a new tab with a starter layout: one pane per channel up to four, or the channels shared out over four panes.

To work on a whole folder instead, click **Open folder…** (⇧⌘O / Ctrl+Shift+O). Explorer lists the folder's files, and a double-click opens one.

## A first look

<!-- shot: first-look — a file open with four panes, the overview strip below, and the time range in the top bar -->

The panes share one time axis. The strip under them is the overview: the whole recording, with the part you are looking at highlighted.

**To zoom into a stretch**, drag across a plot. This is **Box zoom**, the default mouse mode. The view glides to the range you dragged. Drag wider than a few pixels, or the app says "Drag wider to zoom".

**To zoom with the wheel**, scroll over a plot. The point under the pointer stays put. If you turn off **Scroll wheel zooms the charts** in Settings, the wheel pans instead and Ctrl/⌘ + wheel zooms.

**To pan**, switch to **Pan** (P) and drag. In any mode, Shift-drag or middle-drag pans, and the arrow keys ← → pan by 15% of the view, and ↑ ↓ (or + −) zoom.

**To go back**, click **Back** (⌥← / Alt+←, or Backspace). **Fit** (F, or a double-click on a plot) shows the whole recording again.

**To read a value**, point at a plot. A line follows the pointer, and each channel chip in the pane header shows its value there. The top bar shows the time under the pointer. To keep values on screen, place cursors: press C, then click twice, or press A and B with the pointer over a plot. [Reading data](04-reading-data.md) covers cursors and Measure.

While a large file is still being read for the first time, panes show a **coarse** label and the unread part is hatched. You can zoom and read values meanwhile. See the last section of chapter 4.

## Where the cache lives

The first time you open a file, TelemetryBench reads it once and writes summaries of it (the min/max pyramid, the column caches, import settings) to its own cache. It never writes next to your data. Opening the file again is fast because of that cache.

| System | Cache folder |
| --- | --- |
| macOS | `~/Library/Caches/tlmbench` |
| Windows | `%LOCALAPPDATA%\tlmbench` |
| Linux | `~/.cache/tlmbench` |

The environment variable `TLMBENCH_CACHE` overrides the folder. The desktop app and the command-line tool share it.

To see it, open **Settings** (rail, or ⌘, / Ctrl+,) and find the **Cache** section: it shows the path, the size against its cap (20 GB by default; older files are dropped first), **Open folder** and **Clear cache**. Clearing removes the summaries of files that are not open. They are rebuilt the next time a file opens. Your settings, layouts and import settings are kept.

## The read-only promise

- Everything works from the cache alone, on a read-only folder.
- Nothing is written next to your data unless you ask: **Share** writes into the folder's `.tlmbench/`, and exports go where you choose. The app asks before it writes into a folder.
- The app never runs Python or other programs. Custom decoding runs as sandboxed WASM plugins or native plugins you turn on.

Next: [The window](02-the-window.md).
