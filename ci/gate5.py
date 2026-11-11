#!/usr/bin/env python3
"""Gate 5 (the v1 release): a folder check, and an HTML export of a 2 h flight within the size budget that opens in
under 2 s from a static host.

1. A 2 h flight (20 channels at 100 Hz, 720k samples each) and two shorter ones are written; `tlmbench check` runs a
   check set over the folder (exit 0 or 1, a report for each of the three files, with the failing cells listed).
2. `tlmbench export html` writes the flight, every channel of it, with a layout and the check set's summary: the folder must be within the
   250 MB budget, in Full mode (every pane can switch to a table), with the viewer in it.
3. The folder is served by a plain static file server (no server code) and opened in a headless browser: the first
   chart view is drawn in under 2 s (the raw samples go on arriving; when they have, that is reported too), with no
   page errors.

    python3 ci/gate5.py --bin target/release [--keep]

Needs the viewer built into the CLI (`just viewer`, then a build) and Playwright's Chromium (`just setup`). Prints a
JSON report; exits 0 on a pass.
"""

import argparse
import functools
import http.server
import json
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time

BUDGET = 250 * 1024 * 1024
OPEN_BUDGET_MS = 2000
RATE = "100"
CHANNELS = 20

CHECK_SET = """schema = 1
name = "Gate flight"
runs_on_any_file = true

[[check]]
name = "Signals stay under 11"
kind = "check"
group = "Limits"
expect = "max(signal_0, signal_1) < 11"
severity = "fail"
grace = 0
settle = 0
enabled = true
"""

LAYOUT = {
    "schema": 1,
    "name": "Gate flight",
    "panes": [
        {"channels": ["signal_0"], "beside": False, "view": "chart"},
        {"channels": ["signal_1", "signal_2"], "beside": False, "view": "chart"},
        {"channels": ["signal_3"], "beside": False, "view": "table"},
    ],
}


def run(cmd, ok=(0,), **kw):
    r = subprocess.run(cmd, capture_output=True, text=True, **kw)
    if r.returncode not in ok:
        raise SystemExit(f"{' '.join(map(str, cmd))}\n{r.stderr or r.stdout}")
    return r


def folder_size(path):
    return sum(os.path.getsize(os.path.join(d, f)) for d, _, fs in os.walk(path) for f in fs)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default="target/release")
    ap.add_argument("--keep", action="store_true")
    a = ap.parse_args()
    gen, cli = os.path.join(a.bin, "tlmbench-gen"), os.path.join(a.bin, "tlmbench")
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    tmp = tempfile.mkdtemp(prefix="tlm-gate5-")
    report = {}
    try:
        data, cfg, cache = (os.path.join(tmp, n) for n in ("flights", "cfg", "cache"))
        os.makedirs(os.path.join(cfg, "checks"))
        with open(os.path.join(cfg, "checks", "gate.toml"), "w") as f:
            f.write(CHECK_SET)
        t = time.time()
        for name, samples, seed in (("flight-2h", "720K", 1), ("flight-b", "60K", 2), ("flight-c", "60K", 3)):
            run([gen, "--samples", samples, "--channels", str(CHANNELS), "--rate", RATE, "--format", "tlm", "--out", data, "--name", name, "--seed", str(seed)])
        report["generated_s"] = round(time.time() - t, 1)

        # 1. the folder check
        t = time.time()
        out = os.path.join(tmp, "report")
        r = run([cli, "check", data, "--set", "Gate flight", "--config", cfg, "--cache", cache, "--report", "json,html", "--out", out], ok=(0, 1))
        res = json.load(open(os.path.join(out, "results.json"))) if os.path.exists(os.path.join(out, "results.json")) else None
        files = len(res) if isinstance(res, list) else None
        report["folder_check"] = {"exit": r.returncode, "files": files, "seconds": round(time.time() - t, 1)}
        if files != 3:
            raise SystemExit(f"the folder check covered {files} files, not 3\n{r.stdout}\n{r.stderr}")

        # 2. the export
        layout = os.path.join(tmp, "layout.json")
        json.dump(LAYOUT, open(layout, "w"))
        site = os.path.join(tmp, "site")
        t = time.time()
        r = run([cli, "export", "html", os.path.join(data, "flight-2h.tlm"), "--layout", layout, "--all-channels", "--checks", "Gate flight", "--config", cfg, "--cache", cache, "--out", site, "--json"])
        export = json.loads(r.stdout)["report"]
        size = folder_size(site)
        report["export"] = {"seconds": round(time.time() - t, 1), "mode": export["mode"], "bytes": size, "mb": round(size / 1048576, 1), "trimmed": export["trimmed"]}
        if size > BUDGET:
            raise SystemExit(f"the export is {size / 1048576:.0f} MB, over the 250 MB budget")
        if export["mode"] != "full" or export["trimmed"]:
            raise SystemExit("the 2 h flight should fit the budget in Full mode")
        if not os.path.exists(os.path.join(site, "index.html")):
            raise SystemExit("the export has no viewer (run `just viewer`, then build the CLI again)")

        # 3. open it from a static host
        class Quiet(http.server.SimpleHTTPRequestHandler):
            def log_message(self, *args):
                pass

        handler = functools.partial(Quiet, directory=site)
        srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        url = f"http://127.0.0.1:{srv.server_address[1]}/index.html"
        r = run(["node", os.path.join(root, "ci", "gate5_open.mjs"), url])
        opened = json.loads(r.stdout.strip().splitlines()[-1])
        srv.shutdown()
        report["open"] = opened
        if opened["errors"]:
            raise SystemExit(f"the page raised errors: {opened['errors']}")
        if opened["firstViewMs"] > OPEN_BUDGET_MS:
            raise SystemExit(f"the first view took {opened['firstViewMs']} ms, over {OPEN_BUDGET_MS}")
        report["pass"] = True
        print(json.dumps(report, indent=2))
    finally:
        if a.keep:
            print("kept", tmp, file=sys.stderr)
        else:
            shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    main()
