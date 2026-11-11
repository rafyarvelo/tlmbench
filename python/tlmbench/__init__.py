"""TelemetryBench for Python: the same core as the app and the command line.

    import tlmbench
    rec = tlmbench.open("flight.bin")            # decodes once into the app's cache; reads the nearest .tlmbench/
    rec.channels                                  # names, units, types
    rec.read(["motor_rpm"], start="10 s", until="20 s")   # {"time_ns": ..., "motor_rpm": ...} as NumPy arrays
    rec.to_arrow(["motor_rpm"])                   # a pyarrow Table (needs pyarrow)
    rec.view("motor_rpm", 0, 60, width=1200)      # what the app draws for a 1200 px wide chart
    rec.stats(["motor_rpm"], 0, 60)               # exact count, min, max, mean, std
    rec.check("Flight acceptance")                # a check set's results, as the CLI's JSON report holds them
    rec.convert("out/flight.parquet", "parquet")
    rec.export_html("site/", layout=...)          # the same folder as `tlmbench export html`

Times are nanoseconds as integers (`time_ns`) or seconds from the first sample (`t=` / `start=` / `until=` take seconds
as numbers or "10 s" text, or ISO 8601 text). Nothing is written beside the recording: the cache is the app's.
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from typing import Any, Iterable, Sequence

import numpy as np

from . import _core

__all__ = ["open", "Recording", "Channel", "__version__"]
__version__ = _core.__version__


@dataclass(frozen=True)
class Channel:
    index: int
    name: str
    unit: str | None
    dtype: str
    category: str | None


_SECONDS = re.compile(r"^\s*(-?[0-9]*\.?[0-9]+(?:[eE][-+]?[0-9]+)?)\s*(ns|us|ms|s)?\s*$")
_UNIT = {None: 1e9, "s": 1e9, "ms": 1e6, "us": 1e3, "ns": 1.0}


class Recording:
    """An opened recording. Channels are given by name or index."""

    def __init__(self, core: _core.Rec) -> None:
        self._rec = core
        self._info = json.loads(core.info())
        self.path: str = self._info["path"]
        self.format: str = self._info["format"]
        self.samples: int = self._info["samples"]
        self.t_first: int | None = self._info["t_first"]
        self.t_last: int | None = self._info["t_last"]
        self.channels: list[Channel] = [
            Channel(i, c["name"], c.get("unit"), c["dtype"], c.get("category")) for i, c in enumerate(self._info["channels"])
        ]
        self._by_name = {c.name: c.index for c in self.channels}

    def __repr__(self) -> str:
        return f"<tlmbench.Recording {self.path!r} {self.samples:,} samples, {len(self.channels)} channels>"

    # -- helpers --------------------------------------------------------------------------------------------------

    def _chan(self, c: str | int) -> int:
        if isinstance(c, str):
            if c not in self._by_name:
                raise KeyError(f"no channel {c!r} (have: {', '.join(self._by_name)})")
            return self._by_name[c]
        if not 0 <= c < len(self.channels):
            raise IndexError(f"no channel {c}")
        return int(c)

    def _chans(self, channels: Iterable[str | int] | None) -> list[int]:
        return list(range(len(self.channels))) if channels is None else [self._chan(c) for c in channels]

    def ns(self, t: float | int | str | None, default: int | None = None) -> int:
        """A time as nanoseconds: seconds from the first sample (a number or "10 s", "250 ms"), or an ISO 8601 string."""
        if t is None:
            if default is None:
                raise ValueError("a time is needed")
            return default
        first = self.t_first or 0
        if isinstance(t, str):
            m = _SECONDS.match(t)
            if m:
                return first + round(float(m.group(1)) * _UNIT[m.group(2)])
            from datetime import datetime, timezone

            d = datetime.fromisoformat(t.replace("Z", "+00:00"))
            if d.tzinfo is None:
                d = d.replace(tzinfo=timezone.utc)
            return round(d.timestamp() * 1e9)
        return first + round(float(t) * 1e9)

    # -- reading --------------------------------------------------------------------------------------------------

    def read(self, channels: Sequence[str | int] | None = None, start: Any = None, until: Any = None) -> dict[str, np.ndarray]:
        """Samples from `start` up to `until` (default: all) as `{"time_ns": int64, name: float64, ...}`: shown values
        (scale and offset applied), NaN where a sample is missing."""
        idx = self._chans(channels)
        a = 0 if start is None else self._rec.lower_bound(self.ns(start))
        b = self.samples if until is None else self._rec.lower_bound(self.ns(until))
        times, cols = self._rec.read(idx, a, max(a, b))
        out = {"time_ns": np.frombuffer(times, dtype="<i8")}
        for i, raw in zip(idx, cols):
            out[self.channels[i].name] = np.frombuffer(raw, dtype="<f8")
        return out

    def to_arrow(self, channels: Sequence[str | int] | None = None, start: Any = None, until: Any = None):
        """`read` as a pyarrow Table (channels keep their unit in the field metadata)."""
        import pyarrow as pa

        data = self.read(channels, start, until)
        fields = [pa.field("time_ns", pa.int64())]
        for name in list(data)[1:]:
            unit = self.channels[self._by_name[name]].unit
            fields.append(pa.field(name, pa.float64(), metadata={"unit": unit} if unit else None))
        return pa.table(list(data.values()), schema=pa.schema(fields))

    def view(self, channel: str | int, t0: Any, t1: Any, width: int = 1000) -> tuple[np.ndarray, np.ndarray]:
        """What the app draws for `channel` over `[t0, t1)` in a chart `width` pixels wide: the extremes of each pixel
        column, so spikes stay. Returns `(time_ns, values)`."""
        t, y = self._rec.view(self._chan(channel), self.ns(t0), self.ns(t1), int(width))
        return np.frombuffer(t, dtype="<i8"), np.frombuffer(y, dtype="<f4")

    def values_at(self, t: Any, channels: Sequence[str | int] | None = None) -> dict[str, float | None] | None:
        """The sample nearest `t`: `{"time_ns": ..., name: value}` (None for a missing sample)."""
        idx = self._chans(channels)
        r = json.loads(self._rec.values_at(idx, self.ns(t)))
        if r is None:
            return None
        return {"time_ns": r["t"], **{self.channels[i].name: v for i, v in zip(idx, r["values"])}}

    def stats(self, channels: Sequence[str | int] | None = None, start: Any = None, until: Any = None) -> dict[str, dict[str, Any]]:
        """Exact count, min, max (with their times), mean and standard deviation over `[start, until)`."""
        self._rec.build()
        idx = self._chans(channels)
        t0 = self.t_first or 0 if start is None else self.ns(start)
        t1 = (self.t_last or 0) + 1 if until is None else self.ns(until)
        r = json.loads(self._rec.stats(idx, t0, t1))
        return {self.channels[s["channel"]].name: {k: v for k, v in s.items() if k != "channel"} for s in r}

    # -- checks, conversion, export -------------------------------------------------------------------------------

    def check(self, set: str) -> dict[str, Any]:
        """Runs a check set (its name in the nearest `.tlmbench/checks`, or TOML text); its results as a dict with
        `rollup` and `outcomes`, as `tlmbench check --report json` holds them."""
        return json.loads(self._rec.check(set))

    def convert(self, out: str, fmt: str, channels: Sequence[str | int] | None = None, start: Any = None, until: Any = None) -> tuple[str, int]:
        """Writes the recording as csv, parquet, arrow, hdf5 or mat (the CLI's `convert`): `(path, rows)`."""
        a = 0 if start is None else self._rec.lower_bound(self.ns(start))
        b = None if until is None else self._rec.lower_bound(self.ns(until))
        return self._rec.convert(out, fmt, None if channels is None else self._chans(channels), a, b)

    def export_html(self, out: str, layout: dict | str | None = None, channels: Sequence[str | int] | None = None, raw: Sequence[str | int] | None = None, mode: str = "auto", budget: int | None = None) -> dict[str, Any]:
        """Export to HTML (the CLI's `export html`): a static folder holding the viewer and the data. `layout` is a
        shared view (a dict or its JSON); `raw` lists the channels that keep raw samples in Light mode."""
        text = layout if isinstance(layout, str) or layout is None else json.dumps(layout)
        r = self._rec.export_html(out, text, None if channels is None else self._chans(channels), None if raw is None else self._chans(raw), mode, budget)
        return json.loads(r)

    def config_dirs(self) -> list[str]:
        """The `.tlmbench/` folders configuration is read from."""
        return self._rec.config_dirs()


def open(path: str, descriptor: str | None = None, cache: str | None = None, config: Sequence[str] | None = None) -> Recording:
    """Opens a recording (CSV, Parquet, .tlm, packets, ...). `descriptor` names a format descriptor for a binary file;
    `config` lists `.tlmbench/` folders to read instead of the nearest one; `cache` overrides the cache folder."""
    return Recording(_core.open(str(path), descriptor, cache, None if config is None else [str(c) for c in config]))
