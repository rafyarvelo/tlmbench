#!/usr/bin/env python3
"""The Parquet fixtures for browser-only mode's tests: one small file per compression codec, all holding the same data.

    python3 tools/testdata/parquet_codecs.py        write crates/web-parquet/tests/fixtures/codecs-<codec>.parquet

Needs pyarrow (requirements-dev.txt pins it). The data, 2000 rows: `label` (text: left out), `ts` (a timestamp in microseconds,
null at row 7), `speed` (float32, unit m/s in the field's metadata, null at row 9), `Temp (°C)` (int16, null at row 11, so
it widens to f64), `armed` (boolean, no nulls). The reader drops row 7 (no time), so 1999 samples remain.
"""
from pathlib import Path

import pyarrow as pa
import pyarrow.parquet as pq

N = 2000
OUT = Path(__file__).resolve().parents[2] / "crates/web-parquet/tests/fixtures"
CODECS = ["none", "snappy", "gzip", "brotli", "lz4", "zstd"]


def table() -> pa.Table:
    speed = pa.field("speed", pa.float32(), metadata={"unit": "m/s"})
    schema = pa.schema(
        [
            pa.field("label", pa.string()),
            pa.field("ts", pa.timestamp("us")),
            speed,
            pa.field("Temp (°C)", pa.int16()),
            pa.field("armed", pa.bool_(), nullable=False),
        ]
    )
    return pa.table(
        {
            "label": [f"r{i}" for i in range(N)],
            "ts": pa.array([None if i == 7 else i * 1000 for i in range(N)], pa.timestamp("us")),
            "speed": pa.array([None if i == 9 else i * 0.5 for i in range(N)], pa.float32()),
            "Temp (°C)": pa.array([None if i == 11 else i % 300 for i in range(N)], pa.int16()),
            "armed": pa.array([i % 2 == 0 for i in range(N)], pa.bool_()),
        },
        schema=schema,
    )


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    t = table()
    for codec in CODECS:
        # two row groups, so a file read in pieces is covered too
        pq.write_table(t, OUT / f"codecs-{codec}.parquet", compression=codec, row_group_size=1200)
        print("wrote", OUT / f"codecs-{codec}.parquet")


if __name__ == "__main__":
    main()
