#!/usr/bin/env python3
"""Benchmark tracking per commit, kept locally (CI runners are too noisy for timing).

  bench.py record    save the medians criterion just measured (target/criterion) with the commit, in bench/history.json
  bench.py compare   compare them with the last recorded run; exit 1 if any is more than 10% slower

Run `just bench` first, on the same Mac."""
import json, subprocess, sys, datetime, pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
HISTORY = ROOT / "bench" / "history.json"
LIMIT = 0.10


def medians():
    out = {}
    for f in (ROOT / "target" / "criterion").glob("**/new/estimates.json"):
        name = "/".join(f.relative_to(ROOT / "target" / "criterion").parts[:-2])
        out[name] = json.loads(f.read_text())["median"]["point_estimate"]  # ns
    if not out:
        sys.exit("no criterion results in target/criterion: run `just bench` first")
    return dict(sorted(out.items()))


def history():
    return json.loads(HISTORY.read_text()) if HISTORY.exists() else []


def git(*a):
    return subprocess.run(["git", *a], cwd=ROOT, capture_output=True, text=True).stdout.strip()


def record():
    runs = history()
    commit = git("rev-parse", "--short", "HEAD") + ("+dirty" if git("status", "--porcelain", "--untracked-files=no") else "")
    runs = [r for r in runs if r["commit"] != commit] + [
        {"commit": commit, "date": datetime.date.today().isoformat(), "medians_ns": medians()}]
    HISTORY.parent.mkdir(exist_ok=True)
    HISTORY.write_text(json.dumps(runs[-50:], indent=1) + "\n")
    print(f"recorded {len(medians())} medians for {commit} in {HISTORY.relative_to(ROOT)}")


def compare():
    runs = history()
    if not runs:
        sys.exit("nothing recorded yet: run `just bench-record`")
    last, now, bad = runs[-1], medians(), 0
    print(f"against {last['commit']} ({last['date']}); regressions are over {LIMIT:.0%}")
    for name, ns in now.items():
        was = last["medians_ns"].get(name)
        if was is None:
            print(f"  new       {name}")
            continue
        d = ns / was - 1
        flag = "REGRESSED" if d > LIMIT else "ok"
        bad += d > LIMIT
        print(f"  {flag:9} {name}: {was / 1e6:.3f} -> {ns / 1e6:.3f} ms ({d:+.1%})")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    {"record": record, "compare": compare}.get(sys.argv[1] if len(sys.argv) > 1 else "", lambda: sys.exit(__doc__))()
