"""The Python bindings over the same core: what they read, query, check, convert and export is what the app and the
command line do, so the tests compare with known values and with the command line's own export."""

import json
import math
import os
import subprocess
from pathlib import Path

import numpy as np
import pytest

import tlmbench

ROOT = Path(__file__).resolve().parents[2]


@pytest.fixture()
def csv(tmp_path: Path) -> Path:
    p = tmp_path / "bench.csv"
    rows = ["time,rpm [rpm],temp [C]"]
    rows += [f"{i / 100},{math.sin(i / 50)},{20 + i / 300}" for i in range(20000)]
    p.write_text("\n".join(rows) + "\n")
    return p


@pytest.fixture()
def rec(csv: Path, tmp_path: Path) -> tlmbench.Recording:
    return tlmbench.open(csv, cache=str(tmp_path / "cache"), config=[])


def test_info_and_channels(rec: tlmbench.Recording):
    assert rec.samples == 20000
    assert [c.name for c in rec.channels] == ["rpm", "temp"]
    assert rec.channels[0].unit == "rpm"
    assert rec.t_first == 0
    assert rec.t_last == 199_990_000_000


def test_read_returns_numpy_with_exact_times(rec: tlmbench.Recording):
    d = rec.read(["rpm"], start="1 s", until="2 s")
    assert d["time_ns"].dtype == np.int64
    assert d["time_ns"][0] == 1_000_000_000
    assert len(d["time_ns"]) == 100
    assert d["rpm"][0] == pytest.approx(math.sin(100 / 50))
    # by index, and everything when no channel is named
    assert list(rec.read([1], start=0, until=0.05)) == ["time_ns", "temp"]
    assert set(rec.read()) == {"time_ns", "rpm", "temp"}


def test_arrow_table_keeps_units(rec: tlmbench.Recording):
    pa = pytest.importorskip("pyarrow")
    t = rec.to_arrow(["rpm"], until=1)
    assert t.num_rows == 100
    assert t.schema.field("rpm").metadata[b"unit"] == b"rpm"
    assert t.schema.field("time_ns").type == pa.int64()


def test_view_keeps_the_spike(tmp_path: Path):
    p = tmp_path / "spike.csv"
    p.write_text("time,v\n" + "\n".join(f"{i},{25 if i == 5000 else 0}" for i in range(10000)) + "\n")
    r = tlmbench.open(p, cache=str(tmp_path / "c"), config=[])
    t, y = r.view("v", 0, 9999, width=100)
    assert len(t) <= 4 * 100 + 8
    assert y.max() == 25


def test_values_and_stats_are_exact(rec: tlmbench.Recording):
    v = rec.values_at(1.0)
    assert v["time_ns"] == 1_000_000_000
    assert v["rpm"] == pytest.approx(math.sin(100 / 50))
    s = rec.stats(["rpm", "temp"])
    assert s["rpm"]["count"] == 20000
    assert s["rpm"]["max"] == pytest.approx(max(math.sin(i / 50) for i in range(20000)))
    assert s["temp"]["min"] == pytest.approx(20)
    assert s["temp"]["mean"] == pytest.approx(np.mean([20 + i / 300 for i in range(20000)]))


def test_check_runs_a_set(rec: tlmbench.Recording):
    toml = (
        'schema = 1\nname = "T"\nruns_on_any_file = false\n\n[[check]]\nname = "rpm under 0.9"\nkind = "check"\n'
        'group = "Limits"\nexpect = "rpm < 0.9"\nseverity = "fail"\ngrace = 0\nsettle = 0\nenabled = true\n'
    )
    r = rec.check(toml)
    assert r["set"] == "T"
    assert r["rollup"]["verdict"] == "fail"
    with pytest.raises(ValueError):
        rec.check("No such set")


def test_convert_writes_the_file(rec: tlmbench.Recording, tmp_path: Path):
    out, rows = rec.convert(str(tmp_path / "o.csv"), "csv", ["rpm"], until=1)
    assert rows == 100
    assert Path(out).read_text().splitlines()[0].startswith("time")
    with pytest.raises(ValueError):
        rec.convert(str(tmp_path / "x"), "xlsx")


def test_errors_name_what_is_wrong(rec: tlmbench.Recording):
    with pytest.raises(KeyError):
        rec.read(["nope"])
    with pytest.raises(IndexError):
        rec.read([7])
    with pytest.raises(RuntimeError):
        tlmbench.open("/no/such/file.csv")


def test_export_html_matches_the_command_line(csv: Path, rec: tlmbench.Recording, tmp_path: Path):
    mine = tmp_path / "py"
    rep = rec.export_html(str(mine), layout={"schema": 1, "name": "L", "panes": [{"channels": ["rpm"]}]}, channels=["rpm"])
    assert rep["mode"] == "full"
    m = json.loads((mine / "export.json").read_text())
    assert m["layout"]["name"] == "L" and m["files"][0]["samples"] == 20000
    cli = next((p for p in (ROOT / "target/release/tlmbench", ROOT / "target/debug/tlmbench") if p.exists()), None)
    if cli is None:
        pytest.skip("no command line built to compare with")
    theirs = tmp_path / "cli"
    layout = tmp_path / "l.json"
    layout.write_text(json.dumps({"schema": 1, "name": "L", "panes": [{"channels": ["rpm"]}]}))
    env = {**os.environ, "TLMBENCH_CACHE": str(tmp_path / "cache2")}
    subprocess.run([cli, "export", "html", str(csv), "--layout", str(layout), "--no-viewer", "--out", str(theirs)], check=True, env=env, capture_output=True)
    # the same data, byte for byte (the checksum manifest)
    assert json.loads((theirs / "export.json").read_text())["checksums"] == m["checksums"]
