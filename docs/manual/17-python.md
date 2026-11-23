# Python

The `tlmbench` Python package is the same core as the app and the command line, as a library. Use it to read a recording into NumPy or Arrow, ask for what the app draws, compute exact statistics, run check sets, convert, and export HTML, all from a script or a notebook. It reads the same recordings and the same `.tlmbench/` folder, and it never writes beside your data. The app itself never runs Python: scripts run outside it and hand results back as files the app already reads.

The full list of functions and their signatures is in [`docs/python-api.md`](../python-api.md), generated from the package's docstrings. This chapter shows how to use them.

## Install

```sh
pip install tlmbench
```

It needs Python 3.10 or later and NumPy 2. Install `pyarrow` as well (`pip install pyarrow`) if you want `to_arrow()`.

## Open a recording

```python
import tlmbench

rec = tlmbench.open("flight.bin", descriptor="formats/flight.toml")
rec.channels       # a list of Channel(index, name, unit, dtype, category)
rec.samples        # the number of samples
rec.t_first, rec.t_last   # nanoseconds
```

`open(path, descriptor=None, cache=None, config=None)` opens CSV, Parquet, `.tlm` and descriptor-described binary files. The first open decodes into the app's cache, shared with the app and the command line; later opens are fast. `descriptor` names a format descriptor for a binary file. `config` is a list of `.tlmbench/` folders to read instead of the nearest one above the recording. `cache` overrides the cache folder.

**Times.** Methods take times as seconds from the first sample (a number, or text such as `"10 s"` or `"250 ms"`), or as ISO 8601 text. Results carry times as integer nanoseconds (`time_ns`). Channels are given by name or by index.

## Read samples

```python
d = rec.read(["motor_rpm", "battery_v"], start="10 s", until="20 s")
d["time_ns"], d["motor_rpm"]      # NumPy arrays
```

`read` returns a dictionary of arrays: `time_ns` as `int64`, and each channel as `float64` with the app's scale and offset applied, `NaN` where a sample is missing. With no channels it reads all; with no `start` or `until` it reads everything.

For Arrow:

```python
table = rec.to_arrow(["motor_rpm"], start=0, until=60)   # a pyarrow Table; units in the field metadata

import polars
df = polars.DataFrame(rec.arrow_stream(["motor_rpm"]))   # any reader of the Arrow PyCapsule interface
```

The columns cross by the Arrow C stream, not through a Python copy. An `arrow_stream()` can be read once.

## What the app draws

```python
t, y = rec.view("motor_rpm", 0, 60, width=1200)
```

Returns `(time_ns, values)`: the extremes of each pixel column, as the app draws a chart `width` pixels wide, so spikes stay at any zoom. Use it to plot a long recording in matplotlib without reading every sample.

```python
rec.values_at("12.5 s")             # the sample nearest a time, for every channel
rec.values_at(12.5, ["motor_rpm"])  # {"time_ns": ..., "motor_rpm": ...}
```

## Exact statistics

```python
s = rec.stats(["motor_rpm"], 0, 60)
s["motor_rpm"]      # a dict: the count, min and max (with their times), mean and standard deviation
```

Count, min, max (with the times they happen), mean and standard deviation over `[start, until)`, exact whatever the length. The first call builds the pyramid.

## Check sets

```python
result = rec.check("Flight acceptance")
result["rollup"], result["outcomes"]
```

`check` takes a set's name (looked up in the nearest `.tlmbench/checks`) or the set's TOML text. The result has the same `rollup` and `outcomes` as the JSON report of `tlmbench check`.

## Derived channels and events

```python
fast = rec.derive("speed_kmh", "speed * 3.6", unit="km/h")   # a new Recording
fast.read(["speed_kmh"])

rec.derive(recipes=[{"name": "power", "expr": "battery_v * current_a", "unit": "W"}])
rec.derive()                      # the recipes of the nearest .tlmbench/ that apply

rec.add_event("Spike", "12.5 s", duration="2 s", category="Alarm")
rec.events()                      # yours, plus the ones shared in .tlmbench/events/
rec.delete_event(id)              # one of the events you added, by its id
```

`derive` returns a new `Recording` with this one's channels and the derived ones; the original isn't changed. It raises `ValueError` when a recipe you gave doesn't apply (a channel it names is missing, or a mistake in it). Shared recipes that don't apply are skipped with a warning. Events you add are kept in the app's cache, so the app shows them too; nothing is written beside the data. Each event is a dict `{id, name, start, duration, category, source, channels}` with `start` and `duration` in nanoseconds.

## Convert and export HTML

```python
path, rows = rec.convert("out/flight.parquet", "parquet", channels=["motor_rpm"], start=0, until=60)
rec.export_html("site/", layout=view_dict, mode="auto", budget=250 * 1024 * 1024)
```

`convert` writes `csv`, `parquet`, `arrow`, `hdf5` or `mat` to the file path you give and returns `(path, rows)`. `export_html` writes the same folder as `tlmbench export html` and returns its result as a dict. `layout` is a shared view (a dict or its JSON text), `channels` limits the data, `raw` lists channels that keep raw samples in Light mode, `mode` is `"auto"`, `"full"` or `"light"`, and `budget` is in bytes.

## Read a `.tlmbench/` folder

```python
cfg = tlmbench.config("runs/")     # the nearest .tlmbench/ above the path
cfg.dir, cfg.problems              # the folder, and files that didn't read ({path, why})
for r in cfg.recipes:
    print(r.name, r.data)          # parsed TOML (or JSON, for views)
cfg.checks, cfg.views, cfg.events, cfg.profiles, cfg.decoders, cfg.widgets
cfg.of("timemaps")                 # any kind by name
```

Each object has `kind`, `name`, `files` (path to text), `text` (the first file) and `data` (parsed). `rec.config_dirs()` lists the folders a recording's configuration is read from.

## Python and the app

Python is how you do what the app deliberately doesn't. A script can detect something the check language can't express, then hand it back as a file: events in `.tlmbench/events/<recording>.toml`, extra channels as a Parquet file opened beside the recording, or pass and fail as the JSON or JUnit report `tlmbench check` writes.
