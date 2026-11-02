#!/usr/bin/env python3
"""Gate 3 (4M samples/s with no drops), the core alone: `tlmbench-gen --send` streams to `tlmbench record` on
localhost, and the recording is then checked against the sender record by record (`tlmbench-gen --verify`).

Two shapes of 4M f32 samples/s each, 60 s apiece (docs/design.md, Roadmap): 4 channels at 1 MHz and 32 at 125 kHz,
over TCP (gated) and UDP (reported: localhost UDP can lose datagrams in the kernel before the app sees them, and
those show as missing sequence numbers, apart from the ring buffer's own drops).

A tailed CSV is reported, not gated (parsing text costs more than binary records): `tlmbench-gen --append` writes a
growing log at 1M and 4M samples/s (4 channels at 250k and 1M rows/s), truncated once half-way (log rotation), and
`tlmbench record file://…` follows it. Each part is checked row by row; the report says whether the tail kept up.

A TCP run passes with every record recorded and identical, nothing dropped by the ring buffer, no sequence number
missing or repeated, and the ring never more than half full. A run where the sender fell behind its schedule by
over 0.1 s is invalid, not failed. Each recording is deleted once checked. Prints a JSON report; exits 0 on a pass.

    python3 ci/gate3.py --bin target/release [--seconds 60] [--only tcp]
"""

import argparse
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time

SHAPES = [(4, 1_000_000), (32, 125_000)]


def free_port(kind):
    with socket.socket(socket.AF_INET, kind) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def run_one(bins, transport, channels, rate, seconds, work):
    gen, rec = os.path.join(bins, "tlmbench-gen"), os.path.join(bins, "tlmbench")
    port = free_port(socket.SOCK_STREAM if transport == "tcp" else socket.SOCK_DGRAM)
    endpoint = f"{transport}://127.0.0.1:{port}"
    desc = os.path.join(work, "sender.toml")
    subprocess.run([gen, "--channels", str(channels), "--rate", str(rate), "--descriptor-out", desc], check=True)
    send_cmd = [gen, "--send", endpoint, "--channels", str(channels), "--rate", str(rate), "--seconds", str(seconds)]
    rec_cmd = [rec, "record", endpoint, "--descriptor", desc, "--out", work, "--json"]
    if transport == "udp":
        rec_cmd += ["--idle-end", "2"]
    # TCP: the sender listens and the recorder connects (it retries); UDP: the recorder binds before anything is sent
    first, second = (send_cmd, rec_cmd) if transport == "tcp" else (rec_cmd, send_cmd)
    p1 = subprocess.Popen(first, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    time.sleep(0.5)
    p2 = subprocess.Popen(second, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    sender, recorder = (p1, p2) if transport == "tcp" else (p2, p1)
    out = {}
    for name, p in (("sender", sender), ("recorder", recorder)):
        text = p.stdout.read()
        _, status, usage = os.wait4(p.pid, 0)
        p.returncode = os.waitstatus_to_exitcode(status)
        out[name] = json.loads(text) if text.strip() else {}
        out[name + "_cpu_s"] = usage.ru_utime + usage.ru_stime
        out[name + "_exit"] = p.returncode
    sent, h = out["sender"], out["recorder"].get("health", {})
    file = out["recorder"].get("file")
    verified = {}
    if file:
        v = subprocess.run([gen, "--verify", file, "--expect", str(sent.get("sent", -1))], capture_output=True, text=True)
        verified = json.loads(v.stdout) if v.stdout.strip() else {"ok": False}
        size = os.path.getsize(file)
        shutil.rmtree(out["recorder"]["session"], ignore_errors=True)
    else:
        size = 0
    ring = h.get("ring_bytes") or 1
    report = {
        "transport": transport,
        "channels": channels,
        "rate_hz": rate,
        "samples_per_s": sent.get("samples_per_s"),
        "sender_kept_pace": sent.get("kept_pace", False),
        "sender_behind_max_s": sent.get("behind_max_s"),
        "sent": sent.get("sent"),
        "received": h.get("records_in"),
        "recorded": h.get("records_written"),
        "dropped_by_ring": h.get("records_dropped"),
        "sequence_missing": h.get("sequence_missing"),
        "sequence_repeated": h.get("sequence_repeated"),
        "verified": verified,
        "ring_mb": round(ring / 2**20),
        "ring_high_water_pct": round(100 * h.get("ring_high_water_bytes", 0) / ring, 2),
        "write_mb_per_s": round(size / 2**20 / max(h.get("seconds", 1), 1e-9), 1),
        "recorder_cpu_pct": round(100 * out["recorder_cpu_s"] / max(h.get("seconds", 1), 1e-9), 1),
        "error": h.get("error") or (None if out["recorder_exit"] == 0 else f"recorder exit {out['recorder_exit']}"),
    }
    report["valid"] = report["sender_kept_pace"]
    report["pass"] = (
        report["valid"]
        and report["error"] is None
        and report["dropped_by_ring"] == 0
        and report["sequence_missing"] == 0
        and report["sequence_repeated"] == 0
        and verified.get("ok") is True
        and report["ring_high_water_pct"] <= 50
    )
    return report


def run_csv(bins, channels, rate, seconds, work):
    gen, rec = os.path.join(bins, "tlmbench-gen"), os.path.join(bins, "tlmbench")
    log = os.path.join(work, "log.csv")
    app = subprocess.Popen([gen, "--append", log, "--channels", str(channels), "--rate", str(rate),
                            "--seconds", str(seconds), "--rotate-at", str(seconds / 2)], stdout=subprocess.PIPE, text=True)
    time.sleep(0.3)
    tail = subprocess.Popen([rec, "record", "file://" + log, "--out", work, "--idle-end", "2", "--json"],
                            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    appended = json.loads(app.stdout.read() or "{}")
    app.wait()
    text = tail.stdout.read()
    _, status, usage = os.wait4(tail.pid, 0)
    out = json.loads(text) if text.strip() else {}
    h = out.get("health", {})
    checks = []
    for f in h.get("files", []):
        v = subprocess.run([gen, "--verify", f, "--rate", str(rate)], capture_output=True, text=True)
        checks.append(json.loads(v.stdout) if v.stdout.strip() else {"ok": False})
    if out.get("session"):
        shutil.rmtree(out["session"], ignore_errors=True)
    os.remove(log)
    recorded = [c.get("records") for c in checks]
    # a truncation loses only what the tail hadn't read yet (a few ms), as with any copy-and-truncate rotation
    lost = sum(p - (r or 0) for p, r in zip(appended.get("parts", []), recorded))
    secs = max(h.get("seconds", 1) - 2, 1e-9)  # less the idle wait at the end
    return {
        "transport": "csv tail",
        "channels": channels,
        "rate_hz": rate,
        "samples_per_s": channels * appended.get("rows_per_s", 0),
        "appender_kept_pace": appended.get("rows_per_s", 0) >= 0.99 * rate,
        "appended_parts": appended.get("parts"),
        "recorded_parts": recorded,
        "rows_lost_at_rotation": lost,
        "all_rows_identical": bool(checks) and all(c.get("first_mismatch") is None for c in checks),
        "kept_up": bool(checks) and lost <= rate // 10 and recorded[-1:] == appended.get("parts", [])[-1:],
        "ring_high_water_pct": round(100 * h.get("ring_high_water_bytes", 0) / (h.get("ring_bytes") or 1), 2),
        "tail_cpu_pct": round(100 * (usage.ru_utime + usage.ru_stime) / secs, 1),
        "error": h.get("error"),
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bin", default="target/release", help="folder with tlmbench and tlmbench-gen")
    ap.add_argument("--seconds", type=float, default=60.0)
    ap.add_argument("--only", choices=["tcp", "udp", "csv"])
    a = ap.parse_args()
    runs = []
    with tempfile.TemporaryDirectory(prefix="tlmbench-gate3-") as work:
        for transport in [t for t in ("tcp", "udp") if a.only in (None, t)]:
            for channels, rate in SHAPES:
                print(f"gate3: {transport} {channels} channels at {rate:,} Hz for {a.seconds:g} s…", file=sys.stderr)
                r = run_one(a.bin, transport, channels, rate, a.seconds, work)
                verdict = ("pass" if r["pass"] else "FAIL" if r["valid"] else "invalid") if transport == "tcp" else "reported"
                print(f"  {verdict}: {r['recorded']} of {r['sent']} records, "
                      f"ring {r['ring_high_water_pct']}%, missing {r['sequence_missing']}", file=sys.stderr)
                runs.append(r)
        if a.only in (None, "csv"):
            for channels, rate in [(4, 250_000), (4, 1_000_000)]:
                print(f"gate3: csv tail {channels} channels at {rate:,} rows/s for {a.seconds:g} s…", file=sys.stderr)
                r = run_csv(a.bin, channels, rate, a.seconds, work)
                print(f"  reported: {r['recorded_parts']} of {r['appended_parts']} rows, kept up: {r['kept_up']}, "
                      f"tail {r['tail_cpu_pct']}% of a core", file=sys.stderr)
                runs.append(r)
    gated = [r for r in runs if r["transport"] == "tcp"]
    valid = all(r["valid"] for r in gated)
    report = {
        "gate": "Phase 3: 4M samples/s with no drops (core)",
        "seconds": a.seconds,
        "valid": valid,
        "pass": valid and (bool(gated) or a.only is not None) and all(r["pass"] for r in gated),
        "runs": runs,
    }
    print(json.dumps(report, indent=2))
    sys.exit(0 if report["pass"] else 1)


if __name__ == "__main__":
    main()
