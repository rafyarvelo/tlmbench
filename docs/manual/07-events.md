# Events

An event is a named moment or range in a recording: a name, a start, a duration (0 for an instant) and a category. Events come from four places: you add them, the file itself makes them, rules mark them wherever a condition holds, and checks mark where they fail. The Events panel lists them all, and charts, the overview strip and tables mark them. Events you add are yours; the others are read-only and tagged with where they came from.

## The Events panel

The panel sits in a sidebar (drag its title to move it, or use the swap button). It lists events by start time. Click an event to frame it in the charts. **Expand all** and **Collapse all** in the panel's title row open and close the groups.

Press **E** to show or hide event markers on the charts (View › **Event markers**).

<!-- shot: events-panel — the Events panel with category chips, a grouped list, one event being edited and a read-only rule event with its tag -->

## Add and edit your own events

1. Place cursor A: press **C** for cursors, then click the chart. For a range, place A and B.
2. In the Events panel press **Marker at A** (an instant), or **Range A–B**.
3. The new event opens for editing. Change **Name** (it starts as "Marker" or "Range") and **Category**.
4. Press **Done**. **Delete** removes it.

Reopen an event's editor with its pencil button (**Edit**). In a tab with several files, a new event goes to file 1 unless the selected pane holds only another file's channels. The **File** menu in the editor moves it to another file; each file keeps its events in its own clock.

If cursor A isn't placed, the panel tells you to place it first.

## Categories

A category has a name, a colour, an optional icon and a **Charts** switch. Change them in **Settings › Event categories**:

- Click the colour swatch to change the colour, and type in the name box to rename.
- Pick an **icon** from the ones you've added (see chapter 9, Icons).
- Turn **Charts** off to stop marking that category's events on charts.
- **Add** makes a new category. The trash button deletes one; its events become **Uncategorized**.

Charts mark an event when its category is on and the Events panel shows it.

## Filter, group and find

- **Filter** searches name, category and channel.
- The grouping box offers **No groups**, **By category**, **By name**, **By channel**, **By source** and **By file**. Each group shows a count and collapses.
- **Category chips** appear when there is more than one category. Click a chip to hide or show that category. Double-click it, or Alt-click, to show only that one. **All** and **None** set them all.
- The link button at the start of the chips decides whether the charts follow these filters. On: what's hidden in the list is hidden on the charts too, in this tab. Off: each category's **Charts** setting decides.
- **File chips** (several files in a tab) show or hide each file's events.

The line under the filter counts events: "12 events", or "5 of 12 events", with how many are yours, from rules and from checks.

## Events from the file

A file can make events too. In the import dialog for a CSV, tick **Changes of … are events** on a column, and an event is made at each change of that column's value. A format descriptor can name event fields as well. These events are tagged **File** and are read-only, because they change when the file does.

## Events from rules and checks

A rule marks an event wherever its condition holds; a failed check marks the stretch where it fails. These are tagged **Rule** and **Check**. They are not stored: they are made again whenever the data, the rules or the limits change.

Press **Rules…** in the Events panel to open the rule editor for event rules. A rule or check's own pencil button opens its row in the editor, because that is where you change it. See chapter 8.

## Read-only events and their tags

Hover an event that you can't edit to see a tag and why:

| Tag | Where it comes from | What to change instead |
| --- | --- | --- |
| **File** | A column or field of the file. | Nothing here; it changes with the file. |
| **Rule** | A rule over the data. | The rule. |
| **Check** | A check that fails. | The check. |
| **Shared** | The folder's `.tlmbench/`. | The shared file. |

## Share events

**Share…** (in Explorer's folder menu) lists the events you added along with your other objects. Choosing to share writes them to the folder's `.tlmbench/events/<recording>.toml`, one table per event with a stable ID. Colleagues who open the folder see them as **Shared** events beside their own, and they travel into HTML exports. If the folder is read-only, **Export…** writes the same files to a folder you choose. Nothing is written beside your data until you choose Share.

## In tables and exports

In a table, every row inside an event gets its category's tint and an edge stripe. An **Event** column carries a tag on the event's first row (▸ name), its last row (◂ end) or the single row of an instant (● name). Click a tag to highlight the event in the panel.

In a browser tab (no install), events aren't available yet.
