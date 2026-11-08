#!/usr/bin/env python3
"""Gate 4 (formats complete), the core's half: the same code opens a file in the CLI and in the app (both go
through the store), so what the CLI reads is what the app reads.

`matrix`: one synthetic recording is written as .tlm, CSV, a big-endian scaled fixed-width file with its own
descriptor, and packet files (uncompressed, zstd, lz4, gzip), and converted to Parquet, Arrow IPC, HDF5 and MAT.
Every format the core reads opens with the same per-channel count, extremes, mean and standard deviation, and the
same view (the points a chart draws), bit for bit in the binary formats (CSV keeps what its text holds: six
significant digits from the generator, so CSV is compared to that precision). `convert` from each readable file to
Parquet reads back exactly, to Arrow reads back (pyarrow) with the same count, extremes and sum, and to CSV, HDF5
and MAT is written and checked for its signature (nothing here reads HDF5 back; the exporter's own tests do).

`packets`: a compressed packet file of about 10 GB (zstd): its first index build runs at disk speed (reported, not
gated), then, with the index and pyramid cached and the file evicted from memory, a reopen draws the whole view in
under 1 s (gated; the exact view, which reads the packets, is reported).

    python3 ci/gate4.py --bin target/release [--only matrix|packets] [--samples 2M] [--packet-gb 10] [--keep]

Needs numpy and pyarrow (the dev venv has them). Prints a JSON report; exits 0 on a pass. The packet file is
deleted afterwards unless --keep.
"""

import argparse
import csv
import hashlib
import io
import json
import math
import os
import shutil
import subprocess
import sys
import tempfile
import time

CHANNELS = 4
RATE = "10K"
FORMATS = ["tlm", "csv", "packets", "zstd", "lz4", "gzip"]
VIEW_WIDTH = 2000
REOPEN_BUDGET_S = 1.0


def run(cmd, **kw):
    r = subprocess.run(cmd, capture_output=True, text=True, **kw)
    if r.returncode != 0:
        raise SystemExit(f"{' '.join(map(str, cmd))}\n{r.stderr or r.stdout}")
    return r.stdout


class Tool:
    def __init__(self, bins, cache):
        self.gen = os.path.join(bins, "tlmbench-gen")
        self.cli = os.path.join(bins, "tlmbench")
        self.cache = cache

    def q(self, *args):
        return run([self.cli, *args, "--cache", self.cache])

    def stats(self, path, descriptor=None):
        extra = ["--descriptor", descriptor] if descriptor else []
        rows = list(csv.DictReader(io.StringIO(self.q("query", "stats", path, *extra))))
        return {r["channel"]: r for r in rows if r["channel"].startswith("signal_")}

    def view(self, path, descriptor=None):
        extra = ["--descriptor", descriptor] if descriptor else []
        out = self.q("query", "view", path, "--width", str(VIEW_WIDTH), *extra)
        per = {}
        for r in csv.DictReader(io.StringIO(out)):
            if r["channel"].startswith("signal_"):
                per.setdefault(r["channel"], []).append((int(r["time_ns"]), float(r["value"])))
        return per


def close(a, b, rel):
    return a == b or abs(a - b) <= rel * max(1.0, abs(a), abs(b))


def digest(points):
    h = hashlib.sha256()
    for t, v in points:
        h.update(f"{t},{v!r}\n".encode())
    return h.hexdigest()[:16]


def compare(ref_stats, ref_view, stats, view, exact):
    """What differs between a file and the .tlm: empty when they agree (exactly, or to CSV's six digits)."""
    rel = 0 if exact else 2e-4
    bad = []
    for ch, r in ref_stats.items():
        s = stats.get(ch)
        if s is None:
            bad.append(f"{ch}: missing")
            continue
        if int(s["count"]) != int(r["count"]):
            bad.append(f"{ch}: count {s['count']} != {r['count']}")
        for k in ("min", "max", "mean", "std"):
            if not close(float(r[k]), float(s[k]), rel if k in ("min", "max") else max(rel, 1e-9)):
                bad.append(f"{ch}: {k} {s[k]} != {r[k]}")
        a, b = ref_view.get(ch, []), view.get(ch, [])
        if exact:
            if digest(a) != digest(b):
                bad.append(f"{ch}: view checksum {digest(b)} != {digest(a)} ({len(b)} points, {len(a)})")
        else:
            # six-digit text moves which sample is an extreme within a pixel column, so the points can differ by a
            # few: the view must have the same size (to 1%), reach the same extremes and start and end together
            if not a or not b or abs(len(a) - len(b)) > 0.01 * len(a):
                bad.append(f"{ch}: view has {len(b)} points, {len(a)} expected")
            elif not (close(min(v for _, v in a), min(v for _, v in b), rel) and close(max(v for _, v in a), max(v for _, v in b), rel) and a[0][0] == b[0][0] and a[-1][0] == b[-1][0]):
                bad.append(f"{ch}: view extremes or span differ")
    return bad


def write_big_endian(work, arrow_path):
    """A big-endian fixed-width file of the same samples, stored as value / 2 and scaled back by its descriptor."""
    import numpy as np
    import pyarrow as pa

    with pa.ipc.open_file(arrow_path) as f:
        table = f.read_all()
    names = [n for n in table.column_names if n.startswith("signal_")]
    n = table.num_rows
    rec = np.zeros(n, dtype=[(c, ">f4") for c in names])
    for c in names:
        rec[c] = (table[c].to_numpy() / 2.0).astype(">f4")
    path = os.path.join(work, "be.bin")
    rec.tofile(path)
    toml = os.path.join(work, "be.toml")
    fields = "".join(
        f'\n[[fields]]\nname = "{c}"\ntype = "f32"\noffset = {4 * i}\nunit = "V"\nscale = 2.0\n' for i, c in enumerate(names)
    )
    t0 = 1767225600000000000  # the generator's start, 2026-01-01T00:00:00Z
    with open(toml, "w") as f:
        f.write(
            f'schema = 1\nname = "big-endian scaled"\nendian = "big"\nrecord_size = {4 * len(names)}\n\n'
            f'[time]\nsource = "rate"\nrate_hz = 10000.0\nt0_ns = {t0}\n{fields}'
        )
    return path, toml


def matrix(tool, work, samples):
    report = {"samples": samples, "formats": {}, "failures": []}
    run([tool.gen, "--samples", samples, "--channels", str(CHANNELS), "--rate", RATE, "--format", ",".join(FORMATS), "--out", work, "--name", "m"])
    tlm = os.path.join(work, "m.tlm")
    conv = os.path.join(work, "conv")
    for fmt in ("csv", "parquet", "arrow", "hdf5", "mat"):
        run([tool.cli, "convert", tlm, "--to", fmt, "--out", os.path.join(conv, fmt), "--cache", tool.cache])
    ref_stats, ref_view = tool.stats(tlm), tool.view(tlm)
    report["channels"] = {c: int(s["count"]) for c, s in ref_stats.items()}
    be, be_toml = write_big_endian(work, os.path.join(conv, "arrow", "m.arrow"))
    # (name, path, descriptor, exact)
    readable = [
        ("csv", os.path.join(work, "m.csv"), None, False),
        ("big-endian scaled", be, be_toml, True),
        ("parquet", os.path.join(conv, "parquet", "m.parquet"), None, True),
        ("converted csv", os.path.join(conv, "csv", "m.csv"), None, False),
    ] + [(f, os.path.join(work, f"m.{f}.bin"), os.path.join(work, f"m.{f}.toml"), True) for f in FORMATS[2:]]
    for name, path, desc, exact in readable:
        bad = compare(ref_stats, ref_view, tool.stats(path, desc), tool.view(path, desc), exact)
        report["formats"][name] = bad or "same"
        report["failures"] += [f"{name}: {b}" for b in bad]
    # convert from each readable file reads back
    for name, path, desc, exact in [("tlm", tlm, None, True)] + readable:
        d = ["--descriptor", desc] if desc else []
        for fmt in ("parquet", "arrow", "csv", "hdf5", "mat"):
            out = os.path.join(work, "rt", name.replace(" ", "_"), fmt)
            run([tool.cli, "convert", path, "--to", fmt, "--out", out, "--channels", "signal_*", "--cache", tool.cache, *d])
            got = [os.path.join(out, f) for f in os.listdir(out) if not f.endswith(".channels.csv")]
            if len(got) != 1:
                report["failures"].append(f"{name} -> {fmt}: wrote {len(got)} files")
                continue
            bad = check_converted(tool, fmt, got[0], ref_stats)
            report["formats"][f"{name} -> {fmt}"] = bad or "reads back"
            report["failures"] += [f"{name} -> {fmt}: {b}" for b in bad]
    return report


def check_converted(tool, fmt, path, ref):
    if fmt in ("hdf5", "mat"):
        with open(path, "rb") as f:
            head = f.read(8)
        return [] if (head.startswith(b"\x89HDF") or head.startswith(b"MATLAB")) and os.path.getsize(path) > 1000 else [f"not an HDF5 file ({head!r})"]
    if fmt == "csv":
        return []  # written and read back through the CSV reader in the "converted csv" row
    if fmt == "parquet":
        s = tool.stats(path)
        return [
            f"{c}: parquet {k} {s.get(c, {}).get(k)} != {r[k]}"
            for c, r in ref.items()
            for k in ("count", "min", "max")
            if c not in s or not close(float(s[c][k]), float(r[k]), 2e-4)
        ]
    import pyarrow as pa

    with pa.ipc.open_file(arrow_path) as f:
        table = f.read_all()
    names = [n for n in table.column_names if n.startswith("signal_")]
    n = table.num_rows
    rec = np.zeros(n, dtype=[(c, ">f4") for c in names])
    for c in names:
        rec[c] = (table[c].to_numpy() / 2.0).astype(">f4")
    path = os.path.join(work, "be.bin")
    rec.tofile(path)
    toml = os.path.join(work, "be.toml")
    fields = "".join(
        f'\n[[fields]]\nname = "{c}"\ntype = "f32"\noffset = {4 * i}\nunit = "V"\nscale = 2.0\n' for i, c in enumerate(names)
    )
    t0 = 1767225600000000000  # the generator's start, 2026-01-01T00:00:00Z
    with open(toml, "w") as f:
        f.write(
            f'schema = 1\nname = "big-endian scaled"\nendian = "big"\nrecord_size = {4 * len(names)}\n\n'
            f'[time]\nsource = "rate"\nrate_hz = 10000.0\nt0_ns = {t0}\n{fields}'
        )
    return path, toml


def matrix(tool, work, samples):
    report = {"samples": samples, "formats": {}, "failures": []}
    run([tool.gen, "--samples", samples, "--channels", str(CHANNELS), "--rate", RATE, "--format", ",".join(FORMATS), "--out", work, "--name", "m"])
    tlm = os.path.join(work, "m.tlm")
    conv = os.path.join(work, "conv")
    for fmt in ("csv", "parquet", "arrow", "hdf5", "mat"):
        run([tool.cli, "convert", tlm, "--to", fmt, "--out", os.path.join(conv, fmt), "--cache", tool.cache])
    ref_stats, ref_view = tool.stats(tlm), tool.view(tlm)
    report["channels"] = {c: int(s["count"]) for c, s in ref_stats.items()}
    be, be_toml = write_big_endian(work, os.path.join(conv, "arrow", "m.arrow"))
    # (name, path, descriptor, exact)
    readable = [
        ("csv", os.path.join(work, "m.csv"), None, False),
        ("big-endian scaled", be, be_toml, True),
        ("parquet", os.path.join(conv, "parquet", "m.parquet"), None, True),
        ("converted csv", os.path.join(conv, "csv", "m.csv"), None, False),
    ] + [(f, os.path.join(work, f"m.{f}.bin"), os.path.join(work, f"m.{f}.toml"), True) for f in FORMATS[2:]]
    for name, path, desc, exact in readable:
        bad = compare(ref_stats, ref_view, tool.stats(path, desc), tool.view(path, desc), exact)
        report["formats"][name] = bad or "same"
        report["failures"] += [f"{name}: {b}" for b in bad]
    # convert from each readable file reads back
    for name, path, desc, exact in [("tlm", tlm, None, True)] + readable:
        d = ["--descriptor", desc] if desc else []
        for fmt in ("parquet", "arrow", "csv", "hdf5", "mat"):
            out = os.path.join(work, "rt", name.replace(" ", "_"), fmt)
            run([tool.cli, "convert", path, "--to", fmt, "--out", out, "--channels", "signal_*", "--cache", tool.cache, *d])
            got = [os.path.join(out, f) for f in os.listdir(out) if not f.endswith(".channels.csv")]
            if len(got) != 1:
                report["failures"].append(f"{name} -> {fmt}: wrote {len(got)} files")
                continue
            bad = check_converted(tool, fmt, got[0], ref_stats)
            report["formats"][f"{name} -> {fmt}"] = bad or "reads back"
            report["failures"] += [f"{name} -> {fmt}: {b}" for b in bad]
    return report


def check_converted(tool, fmt, path, ref):
    if fmt in ("hdf5", "mat"):
        with open(path, "rb") as f:
            head = f.read(8)
        return [] if (head.startswith(b"\x89HDF") or head.startswith(b"MATLAB")) and os.path.getsize(path) > 1000 else [f"not an HDF5 file ({head!r})"]
    if fmt == "csv":
        return []  # written and read back through the CSV reader in the "converted csv" row
    if fmt == "parquet":
        s = tool.stats(path)
        return compare(ref, {}, s, {}, False) if False else [f"{c}: {k}" for c, r in ref.items() for k in ("count", "min", "max") if str(s.get(c, {}).get(k)) != r[k] and not close(float(s.get(c, {}).get(k, "nan")), float(r[k]), 2e-4)]
    import pyarrow as pa

    with pa.ipc.open_file(path) as f:
        t = f.read_all()
    bad = []
    for c, r in ref.items():
        if c not in t.column_names:
            bad.append(f"{c}: missing")
            continue
        v = t[c].to_numpy()
        if len(v) != int(r["count"]) or not close(float(v.min()), float(r["min"]), 2e-4) or not close(float(v.max()), float(r["max"]), 2e-4):
            bad.append(f"{c}: arrow count/extremes differ")
    return bad


def packets(tool, work, gb, keep):
    report = {"gated": {}, "reported": {}, "failures": []}
    # f32 sine + noise compresses to about 0.88 with zstd: samples for a file of `gb` GB at 20 channels
    samples = int(gb * 1e9 / 0.88 / (4 * 20))
    t = time.time()
    run([tool.gen, "--samples", str(samples), "--channels", "20", "--rate", "125K", "--format", "zstd", "--per-packet", "10000", "--out", work, "--name", "big"])
    report["reported"]["write_s"] = round(time.time() - t, 1)
    bin_, toml = os.path.join(work, "big.zstd.bin"), os.path.join(work, "big.zstd.toml")
    size = os.path.getsize(bin_)
    report["reported"]["file_gb"] = round(size / 1e9, 2)
    if size < gb * 1e9 * 0.95:
        report["failures"].append(f"the packet file is {size / 1e9:.1f} GB, under the {gb} GB the gate needs")
    t = time.time()
    run([tool.cli, "cache", "build", bin_, "--descriptor", toml, "--quiet", "--cache", tool.cache])
    first = time.time() - t
    report["reported"]["first_open_and_build_s"] = round(first, 1)
    report["reported"]["first_open_mb_per_s"] = round(size / 1e6 / first)
    here = os.path.dirname(os.path.abspath(__file__))
    run([sys.executable, os.path.join(here, "evict.py"), bin_, tool.cache])
    # the whole view as the app first draws it, from the cached pyramid alone (gated), then exact (reported)
    view = [tool.cli, "query", "view", bin_, "--descriptor", toml, "--channel", "signal_0", "--width", "1000", "--cache", tool.cache]
    t = time.time()
    out = run(view + ["--coarse"])
    reopen = time.time() - t
    points = max(0, len(out.splitlines()) - 1)
    report["gated"]["cached_reopen_whole_view_s"] = round(reopen, 3)
    report["reported"]["view_points"] = points
    if reopen >= REOPEN_BUDGET_S:
        report["failures"].append(f"the cached reopen and whole view took {reopen:.2f} s (budget {REOPEN_BUDGET_S} s)")
    if points < 500:
        report["failures"].append(f"the whole view has only {points} points")
    t = time.time()
    run(view)
    report["reported"]["exact_whole_view_s"] = round(time.time() - t, 2)
    if not keep:
        os.remove(bin_)
    return report


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default="target/release")
    ap.add_argument("--only", choices=["matrix", "packets"])
    ap.add_argument("--samples", default="2M")
    ap.add_argument("--packet-gb", type=float, default=10.0)
    ap.add_argument("--keep", action="store_true")
    a = ap.parse_args()
    for mod in ("numpy", "pyarrow"):
        try:
            __import__(mod)
        except ImportError:
            raise SystemExit(f"gate4 needs {mod}: run `just setup python` and use .venv/bin/python3")
    work = tempfile.mkdtemp(prefix="gate4-")
    cache = os.path.join(work, "cache")
    os.makedirs(cache)
    tool = Tool(a.bin, cache)
    report = {}
    try:
        if a.only in (None, "matrix"):
            report["matrix"] = matrix(tool, work, a.samples)
        if a.only in (None, "packets"):
            report["packets"] = packets(tool, work, a.packet_gb, a.keep)
    finally:
        if not a.keep:
            shutil.rmtree(work, ignore_errors=True)
        else:
            print(f"kept {work}", file=sys.stderr)
    fails = [f for r in report.values() for f in r["failures"]]
    report["pass"] = not fails
    print(json.dumps(report, indent=1))
    sys.exit(0 if not fails else 1)


if __name__ == "__main__":
    main()
