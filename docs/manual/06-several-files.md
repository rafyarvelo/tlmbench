# Several files in one tab

A tab can hold more than one recording on a shared timeline. Use it to compare two runs, to line up a logger with an external GPS, or to read a channel from one file against another. Each extra file takes a numbered slot, and an alignment method says how its clock maps onto the first file's. Nothing in the data is rewritten: the shift is applied when values are read, so changing it is instant.

## Slots and names

The first file in a tab is the **primary**, slot 1. It defines time zero, and its channels keep their plain names. Each other file takes the next free slot, and its channels read `accel_x·2`, `accel_x·3` and so on. Layouts store these numbered names, so a saved layout works for any pair of runs.

What you see is friendlier: with more than one file in the tab, a channel reads `flight-2 · accel_x` (the file's short name, then the channel) in chips, legends, tables, readouts and event labels. A file's **Channel prefix** (below) changes the short name.

In Explorer the channel tree gains a file level. Each file's row shows its slot number and how it is aligned ("2 · same clock"), and every file closes from its own row.

**Make primary** swaps a file into slot 1. Channel names, sort and filter settings and colours are renamed to match, and the other files are re-measured against the new primary. Closing the primary promotes the lowest-numbered remaining file.

## Put files in one tab

**Open together.** Select two or more files in Explorer (⌘-click or Shift-click on a Mac; Ctrl-click or Shift-click on Windows and Linux), then press **Open together** in the selection bar. The first file is the primary; the others go on its timeline. The new tab is named after its files ("flight-1 + flight-2"). It starts with a pane for each channel the files share, holding every file's copy, then charts for the channels only one file has.

**Add to this tab.** With a file already open, right-click another file in Explorer and choose **Add to this tab**. If the files were logged in one session (their recording clocks overlap) they start on **Same clock**; otherwise they start on **Align starts**.

**Compare runs.** Select exactly two files and press **Compare**, or right-click a file and choose **Compare with** the file in front. It opens a tab with the starts aligned and one pane per channel both files have (the first of each group, up to four). **Save as…** keeps it as a layout.

**Chips.** In a tab with several files, right-click a channel chip for **From file N · name** (read this channel from that file instead) and **Add from every file** (every file's version of it in the pane). A pane's ⋯ menu has **Duplicate for file N**.

<!-- shot: slots — Explorer with two files in a tab, slot numbers and "same clock" on the rows, and a pane with both files' channels -->

## Alignment

Click the row of any file but the first in Explorer, or right-click it and choose **Alignment and channel prefix…**. Its Alignment section opens under the row.

<!-- shot: alignment-section — a file's Alignment section after cross-correlation: method, note, Fit, residuals plot, Now, Nudge, Lock -->

Choose a method in **Alignment**:

| Method | What it does | When to use it |
| --- | --- | --- |
| **Align starts** | Both files start at the same moment. | Comparing separate runs. |
| **Same clock** | No shift: the recording clocks agree. | Files logged in one session. |
| **Offset** | Starts aligned, then shifted by the seconds you type (**Seconds after file 1 starts**, may be negative). | Clocks that were never synced. |
| **Sync points** | You say which moment in file 1 matches which in this file. Two points give an offset and clock drift; more follow the points piece by piece. | Any pair where you can see the same event in both. |
| **Event matching** | Pairs events with the same name in both files, in order, and fits the shift to them. **Event name** picks which; **Automatic** looks for one both have. | An armed flag or a step that both files recorded as an event. |
| **Cross-correlation** | Matches a channel both files have, in windows, to find the offset and track drift. **Channel** picks it, or **Automatic**. | Two loggers that saw the same signal. |
| **Time channel** | Reads a time channel both files record (such as a GPS time of week, in seconds) as the map. **Channel** picks it. | Files that record absolute time as data. |
| *plugin name* **(plugin)** | An alignment plugin you installed. **Parameters** takes the JSON text the plugin expects. | Anything custom. **Settings › Plugins** lists what is installed. |

When a method can't work it says why under the box, for example "Both files need a channel named …", "No clear match on … (r = 0.31)" or "The files don't overlap on that channel".

### Add sync points

1. Choose **Sync points**.
2. In the chart, put cursor A on a moment in file 1.
3. Put cursor B on the same moment in this file as it is drawn now.
4. Press **Add a point from cursors A and B**. The list shows each point as "12.40 s → 12.90 s".
5. Repeat for more points. Each **×** removes one.

### Fit, residuals and preview

Event matching, cross-correlation, Time channel, Sync points and plugins report how well they fit:

- **Fit** is a percentage; a note names what was matched, for example "Matched on accel_x: +1.2 s, r = 0.98".
- The **residuals plot** has time across and the leftover error of each matched point up, in milliseconds, with a line at zero. The two numbers beside it are the highest and lowest residual. Points close to the line mean a good fit.
- The result is drawn straight away with **Showing the result.** Press **Keep** to accept it or **Undo** to go back to the map you had.

**Now** shows the current shift (and clock drift in ppm, when the map has one).

### Nudge, lock and prefix

- **Nudge** moves the file earlier or later on top of any method. Set the step in seconds (default 0.01) and press **−** or **+**.
- **Lock** freezes the file's alignment. While locked, the method, offset and nudge are disabled and nothing re-estimates it.
- **Channel prefix** sets the file's short name for channel labels. Leave it empty to use the file name.

A pair of files you aligned before opens the way you left it, with your knots. **Share…** writes the alignment to the folder's `.tlmbench/timemaps/` so a colleague gets the same one.

## Events, checks and rules across files

Every file's events show on the tab's timeline through its alignment, labelled with the file, with file chips in the Events panel to filter by file. A new event goes to file 1, unless the selected pane holds only another file's channels; the event editor's **File** moves it. Each file's check sets run on that file, and the results are put on the tab's timeline through the map. The Rules & checks panel has **Results for** to pick a file.

## Tables and XY across files

- **Tables.** Rows are counted by the first column's file. A column from another file shows its last value held between its own samples, and a cell that repeats the one above is dimmed. You can sort and filter by those columns.
- **XY.** An XY pane can pair a channel from one file with a channel from another on the shared timeline, at the X channel's samples, holding the Y channel's value.
- **Derived channels.** A recipe can read another file's channel by its numbered name (`"Close·2"`) or its short name; see chapter 5.

## Lock the view

The **Lock** button on the top bar (press **L**) locks the time view so zoom, pan and fit can't change it. With **Settings › Mouse › Also lock the layout** on, panes and their channels stay put too: no moving, adding, closing or resizing. It is separate from a file's alignment lock.
