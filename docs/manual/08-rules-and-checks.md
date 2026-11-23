# Rules and checks

A *check* says what should be true in a recording and gets a verdict for each file: `motor_rpm < 5000` while the flight mode is Mission, `battery_v > 21` while armed, `max(esc_temp_1) < 80`. A *rule* is the same condition turned around: wherever it holds, it marks an event. Both are written in the expression language (chapter 5) and live together in *sets*. The Rules & checks panel shows what ran on the open file, a folder run and the Report tab compare many files, and `tlmbench check` runs the same sets in CI with the same results.

## Checks and rules

| | Check | Rule |
| --- | --- | --- |
| What it does | Passes or fails. | Marks an event wherever its condition holds. |
| Condition | **Expect**: what should hold. | **Mark when**: where to mark. |
| Result | A verdict. Violations (where the condition doesn't hold) become events tagged **Check**. | Events tagged **Rule**. No pass or fail. |
| Own setting | **Severity**: **Fail** or **Warn**. | An event **category**. |

Both can have:

- **When**: only the samples where this holds count, such as `flight_mode == 'Mission'`.
- **Grace**: how long, in seconds, the condition may break before it counts. Shorter blips are ignored. For a rule, how long the condition must hold before it marks.
- **Settle**: seconds to wait after **When** starts before looking.
- **Missing values fail** (checks only): off by default, so missing values are no data rather than failures.

A check's result is one of **Pass**, **Fail**, **Warn**, **Not applicable** (its **When** never held), **Can't run** (a channel it needs is missing), **Error** or **Off**. A check with a channel missing says "can't run" rather than failing.

**Summary checks** compare one number over the file with a limit: `max(esc_temp_1) < 80`, `pct(gps_sats >= 10) >= 95`, `duration(failsafe) < 5`, `occurrences(brownout) == 0`. Its row shows the value ("max 16.2 (limit 20)").

## The expression language for checks

Checks use the same language as derived channels (chapter 5), plus these helpers that suit conditions:

- `pct(cond)`: percentage of samples where the condition is true.
- `duration(cond)`: seconds it was true.
- `occurrences(cond)`: how many times it became true.
- `since(cond)`: seconds since it last held.
- `past_warn(channel)` and `past_alarm(channel)`: 1 where the channel is at or beyond its warn or alarm limits (chapter 5, Warn and alarm limits). A check or rule that uses them runs again when you change the limits.
- `max(x)`, `min(x)`, `mean(x)` and the other one-number functions.

Combine conditions with `&&`, `||` and `!`. Value labels go in single quotes: `flight_mode == 'Mission'`. Channels at different rates are lined up by holding the last value.

## Sets

A set is a named list of checks and rules, each in a group. A group takes its worst result: Fail beats Warn, which beats Error and Can't run, which beat Pass. **Not applicable** doesn't count. A set takes the worst of its groups. Groups marked **info only** show their results but never fail the set.

A set refers to channels by name, so one set works on any file that has them.

### Make a set

1. Open the **Rules & checks** panel (the rail) and press **New set**.
2. Name the set. Choose where it runs (below).
3. Press **Check** or **Rule** to add a row. Give it a name and a group (a new group name starts a group), then a severity or an event category.
4. Fill in **Expect** (or **Mark when**), and optionally **When**, **Grace** and **Settle**.
5. Read the grey line at the end of the row: it previews the result on the open file, for example "fails · 3× · 6.58 min · worst 7995 rpm · first at 00:25:47".
6. Press **Save**.

<!-- shot: check-editor — the set editor with a check in Lists mode, a check in ƒ mode, a rule row and the preview lines -->

**Lists or ƒ.** **Expect** and **When** can each be written two ways, and both save the same expression:

- **Lists**: choose "every sample of", "the max of", "the min of", "the mean of" or "the magnitude of", then a channel, a comparison and a value. A channel with value labels offers **is** and **is not** with a list of its labels; **outside** takes two numbers; **past its warn limits** and **past its alarm limits** follow the channel's limits.
- **ƒ**: type an expression. Suggestions complete channel names, functions and value labels, and a mistake is flagged as you type. **Lists** comes back when the expression is simple enough.

Other controls in the editor:

- The switch at the start of a row turns it on or off.
- ▲ ▼ move a row; the trash button deletes it.
- **+ From limits** adds a check for each alarm limit in the file.
- **Event categories** (at the bottom) edits colour and name of categories, and **+ Category** adds one.
- **Light export keeps raw data around its violations** takes sizes such as `3x,30s,10min`; it affects Light HTML exports.
- **Delete set** removes the set.

A quick start: right-click a channel and choose **Add check…** or **Mark when…**. The row is prefilled from the channel's limits, and **Mark when…** adds it to the set **Event rules**.

### Where a set runs

In the editor choose any of:

- **Run on every file with these channels.** Files where nothing can run don't show it. Where only some groups can run, those run and the rest are folded into "N checks can't run here".
- **This file**.
- **Whenever the layout … is in front**.

An import profile can also name sets, so every file with that CSV header runs them (in the import dialog: **Rules and checks that run on them**). In the panel, a set that isn't running on the open file is listed under **Not running on this file**, with **Run here**; for a set you attached to the file, **Stop** takes it off.

## The Rules & checks panel

The panel shows the sets that run on the open file: set, then groups, then rows. A badge on the title shows the file's count: ✗ failed, ! warned or ✓ passed. A row shows a symbol for its result (✓ pass, ✗ fail, ! warn, – can't run, ⚑ a rule with its event count), its name and a line such as "Fail · 3× · 6.58 min · worst 7995 at 00:25:47".

- Click a row to select it and go to its first violation. **▲** and **▼** step through the others.
- The chart button on a row opens the channels it reads in a new chart, in the selected pane, or in a new table.
- Double-click a row, or press the pencil on a set, to edit it. Right-click a row for **Edit this check**, **Select** and the channel actions.
- In a tab with several files each file has its own folding row with its slot number and a badge.

Results appear in other places too: a count badge on the file's Explorer row and its tab (it flashes when a check starts failing while live), a red dot on channels a failing check reads, each violation as an event in the charts, the overview and the Events panel, and a gauge card that says "✗ check" while the time it reads falls inside a violation. A **Checks** pane view shows the same summary as a pane, so a layout can carry it; with channels in the pane it shows only the checks that read them.

Results are cached by the file's contents and the set's version. On a read-only folder nothing is written next to the recordings.

## Live checks

When a live tab is following a source, its rules and checks run again every couple of seconds on what has been recorded, and once more when the source ends. A check that starts failing flashes the tab's badge. See chapter 10.

## Run a set on a folder

1. With a folder open in Explorer, press **Run on folder…** in the Rules & checks panel. It opens a **Report** tab in the strip.
2. Choose the **Set**.
3. Press **Run**. It runs on every recording in the folder, three at a time. Each file is opened by the core by itself, so your other tabs are untouched.

The **Grid** tab shows files down and groups across, with a verdict in each cell. Click a cell to open that file at its first violation in that group. **Run again** repeats the run; the report is kept with the set and reopens as it was. **Results as CSV…** saves the grid.

## The Report tab: pass rates and trends

The **Pass rates and trends** tab lists each check with:

- **Pass rate**: how many files passed of those it ran on ("7 of 9 files").
- **Trend (oldest first)**: a small chart with a point per file in date order, red for fail, yellow for warn, green for pass. Up is the check's value when it has one (a summary check's number, else its worst value). Click a point to open that file. The range of values is shown beside it.

Sort by **Check**, **Group** or **Pass rate**, and filter by **Group**. The dates the files span show beside the filter.

<!-- shot: report-tab — the Report tab on Pass rates and trends: several checks with pass rates and sparklines -->

## Save a table filter as a rule

In a table pane, a filter you've set on columns can become an event rule: press **Save as rule** on the filter bar. A new rule opens in the editor in the set **Event rules**, named from the filter, with its tests as the expression; it runs on every file with those channels, and its matches become events. A filter that only tests blank or not blank can't be saved.

## Run checks in CI: `tlmbench check`

`tlmbench check` runs sets on recordings with the same engine and cache as the app, and writes reports. It exits with a code you can use to fail a build.

```
tlmbench check recordings/ --set "Flight acceptance" --report junit,html --out reports
```

Paths can be recordings or folders (the files in a folder, in name order). Sets are found by name in the nearest `.tlmbench/checks/` above the first path, or in `--sets-from`, or in the app's own sets.

| Option | What it does |
| --- | --- |
| `--set NAME` | The set to run (repeat for several). |
| `--sets-from PATH` | A folder of set files (TOML), or one (repeat). |
| `--sets-from-profile` | Also run the sets each CSV file's import profile names, so CI and the app agree. |
| `--report junit,csv,json,html` | The reports to write into `--out`. |
| `--out DIR` | Where reports go (created if missing). |
| `--views` | With `--report html`: also write an HTML export of each file with a failing check, with the violations marked, and link the failing cells to it. |
| `--jobs N` | Files checked at once (default 1). |
| `--fail-on warn` | Count a warning as a failure. |
| `--allow-cant-run` | A check that can't run (a channel is missing) doesn't fail the build. |
| `--watch` | Watch the folders: check each new recording once its size has held still for `--settle` seconds (default 2), writing reports to `<out>/<file name>/`. Runs until stopped, or `--stop-after` seconds. |
| `--config DIR`, `--descriptor FILE`, `--profile FILE`, `--recipes PATH`, `--derived`, `--expr NAME=EXPR` | Shared with the other commands: the configuration, descriptors, import profiles and derived channels to read the files with. |

**Exit codes.** The highest that applies is returned.

| Code | Meaning |
| --- | --- |
| `0` | Everything passed. |
| `1` | A check failed (a warning too with `--fail-on warn`). |
| `2` | A file couldn't be read, or a check couldn't run (a channel is missing). `--allow-cant-run` leaves the second out. |
| `3` | A configuration error: a set that isn't found, a mistake in an expression, or a bad argument. |

`tlmbench validate` lints a `.tlmbench/` folder: it names every file that doesn't read and every recipe or check whose expression doesn't parse, and exits `0` when clean, `3` on problems. Run it in CI before `tlmbench check`.
