#!/usr/bin/env python3
"""Step 5.1's budget: derived channels over a 100M-sample recording, against the pyramid build.

Writes a 2-channel, 100M-sample .tlm in a temporary folder, builds its pyramid, then computes each recipe into a fresh
cache (`tlmbench info --expr`, which evaluates the recipe into a derived entry) and prints the seconds. The recording
is deleted afterwards.
"""
import argparse, os, shutil, subprocess, sys, tempfile, time

RECIPES = [
    ("element by element", "S=signal_0+signal_1"),
    ("element by element, if", "S=if(signal_0>0,signal_0,0)"),
    ("difference", "S=delta(signal_0)"),
    ("running total", "S=cumsum(signal_0)"),
    ("rolling mean (50)", "S=sma(signal_0,50)"),
    ("rolling std (100)", "S=rolling_std(signal_0,100)"),
    ("low-pass, zero phase", "S=lowpass(signal_0,100)"),
    ("z-score (two passes)", "S=(signal_0-mean(signal_0))/std(signal_0)"),
]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default="target/release")
    ap.add_argument("--samples", default="100M")
    a = ap.parse_args()
    tlm, gen = os.path.join(a.bin, "tlmbench"), os.path.join(a.bin, "tlmbench-gen")
    tmp = tempfile.mkdtemp(prefix="derived-bench-")
    try:
        subprocess.run([gen, "--samples", a.samples, "--channels", "2", "--format", "tlm", "--out", tmp, "--name", "big"],
                       check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        rec = os.path.join(tmp, "big.tlm")
        cache = os.path.join(tmp, "cache")
        t = time.time()
        subprocess.run([tlm, "cache", "build", rec, "--cache", cache], check=True, stdout=subprocess.DEVNULL)
        pyr = time.time() - t
        print(f"pyramid build, both channels (hot file): {pyr:.2f} s")
        for label, expr in RECIPES:
            c = os.path.join(tmp, "c")
            shutil.rmtree(c, ignore_errors=True)
            t = time.time()
            subprocess.run([tlm, "info", rec, "--cache", c, "--expr", expr], check=True, stdout=subprocess.DEVNULL)
            print(f"derived, {label}: {time.time() - t:.2f} s  ({a.samples} samples, written as f64)")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
