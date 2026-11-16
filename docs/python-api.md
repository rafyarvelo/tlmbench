# Python API

Generated from the docstrings of the `tlmbench` module by `tools/python_docs.py` (`just python-docs`). Do not edit.

```python
import tlmbench
```

TelemetryBench for Python: the same core as the app and the command line.

    import tlmbench
    rec = tlmbench.open("flight.bin")            # decodes once into the app's cache; reads the nearest .tlmbench/
    rec.channels                                  # names, units, types
    rec.read(["motor_rpm"], start="10 s", until="20 s")   # {"time_ns": ..., "motor_rpm": ...} as NumPy arrays
    rec.to_arrow(["motor_rpm"])                   # a pyarrow Table (needs pyarrow); the columns cross by the Arrow C stream
    polars.DataFrame(rec.arrow_stream())          # anything that reads the Arrow PyCapsule interface takes it directly
    rec.view("motor_rpm", 0, 60, width=1200)      # what the app draws for a 1200 px wide chart
    rec.stats(["motor_rpm"], 0, 60)               # exact count, min, max, mean, std
    rec.check("Flight acceptance")                # a check set's results, as the CLI's JSON report holds them
    rec.derive("speed_kmh", "speed * 3.6", unit="km/h")   # a new Recording with a derived channel (a recipe's expression)
    rec.add_event("Spike", "12.5 s", duration="2 s")       # an event of your own, kept in the app's cache
    rec.events()                                   # yours and the ones shared in .tlmbench/events/
    tlmbench.config("runs/")                       # the nearest .tlmbench/'s recipes, checks, views... as objects
    rec.convert("out/flight.parquet", "parquet")
    rec.export_html("site/", layout=...)          # the same folder as `tlmbench export html`

Times are nanoseconds as integers (`time_ns`) or seconds from the first sample (`t=` / `start=` / `until=` take seconds
as numbers or "10 s" text, or ISO 8601 text). Nothing is written beside the recording: the cache is the app's.

## Functions

### `tlmbench.open(path: 'str', descriptor: 'str | None' = None, cache: 'str | None' = None, config: 'Sequence[str] | None' = None) -> 'Recording'`

Opens a recording (CSV, Parquet, .tlm, packets, ...). `descriptor` names a format descriptor for a binary file;
`config` lists `.tlmbench/` folders to read instead of the nearest one; `cache` overrides the cache folder.

### `tlmbench.config(path: 'str') -> 'Config'`

The `.tlmbench/` folder at `path` (or the nearest one above it, for a recording or a folder), as objects.


## `Recording`

An opened recording. Channels are given by name or index.

### `Recording.add_event(self, name: 'str', start: 'Any', duration: 'Any' = 0, category: 'str' = '', channels: 'Sequence[str] | None' = None) -> 'dict[str, Any]'`

Adds an event: a marker at `start`, or a range when `duration` isn't 0. Times are as for `read` (seconds from the
first sample, "10 s" text or ISO 8601); `duration` is a length ("2 s", "250 ms" or seconds). Kept in the app's cache for
this recording, so the app shows it too; nothing is written beside the data. Returns the event.

### `Recording.arrow_stream(self, channels: 'Sequence[str | int] | None' = None, start: 'Any' = None, until: 'Any' = None) -> "'ArrowStream'"`

The same data as an object with `__arrow_c_stream__` (the Arrow PyCapsule interface): pyarrow, polars and DuckDB
take it directly, e.g. `polars.DataFrame(rec.arrow_stream(["rpm"]))`. It can be read once.

### `Recording.check(self, set: 'str') -> 'dict[str, Any]'`

Runs a check set (its name in the nearest `.tlmbench/checks`, or TOML text); its results as a dict with
`rollup` and `outcomes`, as `tlmbench check --report json` holds them.

### `Recording.config_dirs(self) -> 'list[str]'`

The `.tlmbench/` folders configuration is read from.

### `Recording.convert(self, out: 'str', fmt: 'str', channels: 'Sequence[str | int] | None' = None, start: 'Any' = None, until: 'Any' = None) -> 'tuple[str, int]'`

Writes the recording as csv, parquet, arrow, hdf5 or mat (the CLI's `convert`): `(path, rows)`.

### `Recording.delete_event(self, id: 'str') -> 'None'`

Removes one of the events you added, by its id.

### `Recording.derive(self, name: 'str | None' = None, expr: 'str | None' = None, *, unit: 'str | None' = None, category: 'str | None' = None, decimals: 'int | None' = None, recipes: 'Sequence[dict[str, Any]] | None' = None) -> "'Recording'"`

A new Recording with this one's channels and then derived ones, computed as the app computes them.

`derive("speed_kmh", "speed * 3.6", unit="km/h")` makes one; `recipes=[{"name": ..., "expr": ..., "unit": ...}]`
makes several; with neither, the recipes of the nearest `.tlmbench/` that apply to this recording are used.
Raises `ValueError` when a recipe you gave doesn't apply (a channel it names is missing, a mistake in it); shared
recipes that don't apply are skipped with a warning. This recording is not changed.

### `Recording.events(self) -> 'list[dict[str, Any]]'`

The events of this recording in start order: the ones you added (in the app's cache) and the ones shared in the
nearest `.tlmbench/events/`. Each is `{id, name, start, duration, category, source, channels}` with `start` and
`duration` as nanosecond integers.

### `Recording.export_html(self, out: 'str', layout: 'dict | str | None' = None, channels: 'Sequence[str | int] | None' = None, raw: 'Sequence[str | int] | None' = None, mode: 'str' = 'auto', budget: 'int | None' = None) -> 'dict[str, Any]'`

Export to HTML (the CLI's `export html`): a static folder holding the viewer and the data. `layout` is a
shared view (a dict or its JSON); `raw` lists the channels that keep raw samples in Light mode.

### `Recording.ns(self, t: 'float | int | str | None', default: 'int | None' = None) -> 'int'`

A time as nanoseconds: seconds from the first sample (a number or "10 s", "250 ms"), or an ISO 8601 string.

### `Recording.read(self, channels: 'Sequence[str | int] | None' = None, start: 'Any' = None, until: 'Any' = None) -> 'dict[str, np.ndarray]'`

Samples from `start` up to `until` (default: all) as `{"time_ns": int64, name: float64, ...}`: shown values
(scale and offset applied), NaN where a sample is missing.

### `Recording.stats(self, channels: 'Sequence[str | int] | None' = None, start: 'Any' = None, until: 'Any' = None) -> 'dict[str, dict[str, Any]]'`

Exact count, min, max (with their times), mean and standard deviation over `[start, until)`.

### `Recording.to_arrow(self, channels: 'Sequence[str | int] | None' = None, start: 'Any' = None, until: 'Any' = None)`

`read` as a pyarrow Table (channels keep their unit in the field metadata). The columns cross through the Arrow C
stream interface, not through a Python copy.

### `Recording.values_at(self, t: 'Any', channels: 'Sequence[str | int] | None' = None) -> 'dict[str, float | None] | None'`

The sample nearest `t`: `{"time_ns": ..., name: value}` (None for a missing sample).

### `Recording.view(self, channel: 'str | int', t0: 'Any', t1: 'Any', width: 'int' = 1000) -> 'tuple[np.ndarray, np.ndarray]'`

What the app draws for `channel` over `[t0, t1)` in a chart `width` pixels wide: the extremes of each pixel
column, so spikes stay. Returns `(time_ns, values)`.


## `Channel`

Channel(index: 'int', name: 'str', unit: 'str | None', dtype: 'str', category: 'str | None')


## `Config`

What a `.tlmbench/` folder holds, as objects. `dir` is the folder (None when there is none); `problems` lists files
that did not read, each `{path, why}`.

### `Config.checks(self) -> 'list[ConfigObject]'`

### `Config.decoders(self) -> 'list[ConfigObject]'`

### `Config.events(self) -> 'list[ConfigObject]'`

### `Config.of(self, kind: 'str') -> 'list[ConfigObject]'`

### `Config.profiles(self) -> 'list[ConfigObject]'`

### `Config.recipes(self) -> 'list[ConfigObject]'`

### `Config.views(self) -> 'list[ConfigObject]'`

### `Config.widgets(self) -> 'list[ConfigObject]'`


## `ConfigObject`

One object in a `.tlmbench/` folder: a recipe, a check set, a shared view, an import profile, ... `files` maps each of
its files (relative to the folder) to its text; `text` is the first one, and `data` parses it (TOML or JSON).

### `ConfigObject.data(self) -> 'Any'`

### `ConfigObject.text(self) -> 'str'`
