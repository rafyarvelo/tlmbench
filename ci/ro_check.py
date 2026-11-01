#!/usr/bin/env python3
"""Prove that commands leave a data folder untouched (hard requirement: nothing is written next to the data).

Usage: ro_check.py <folder> [--require-ro] [--cmd "<shell command>"]...

Records every path under <folder> with its type, size, mtime and SHA-256 (and each directory's mtime, so a file
created and then deleted still shows), runs each command, and fails if a command fails or anything changed.
With --require-ro it first checks that the folder really is read-only (creating a file must fail), so a command that
tries to write gets an error instead of quietly succeeding. In CI and in `just ro-test-mac` the folder is a
read-only mount; elsewhere the before/after comparison still catches writes.
"""
import argparse
import hashlib
import os
import subprocess
import sys
import uuid


def manifest(root):
    out = {}
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames.sort()
        st = os.lstat(dirpath)
        out[os.path.relpath(dirpath, root)] = ("dir", st.st_mtime_ns, sorted(dirnames + filenames))
        for name in sorted(filenames):
            path = os.path.join(dirpath, name)
            st = os.lstat(path)
            digest = None
            if os.path.isfile(path) and not os.path.islink(path):
                h = hashlib.sha256()
                with open(path, "rb") as f:
                    for block in iter(lambda: f.read(1 << 20), b""):
                        h.update(block)
                digest = h.hexdigest()
            out[os.path.relpath(path, root)] = ("file", st.st_size, st.st_mtime_ns, digest)
    return out


def is_writable(root):
    probe = os.path.join(root, f".ro-probe-{uuid.uuid4().hex}")
    try:
        with open(probe, "w"):
            pass
    except OSError:
        return False
    os.remove(probe)
    return True


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("folder")
    ap.add_argument("--require-ro", action="store_true", help="fail unless the folder is read-only")
    ap.add_argument("--cmd", action="append", default=[], help="shell command to run (repeatable)")
    a = ap.parse_args()

    if not os.path.isdir(a.folder):
        sys.exit(f"ro-check: no folder {a.folder}")
    if a.require_ro and is_writable(a.folder):
        sys.exit(f"ro-check: FAIL {a.folder} is writable; mount it read-only")

    before = manifest(a.folder)
    failed = []
    for cmd in a.cmd:
        print(f"ro-check: $ {cmd}", flush=True)
        if subprocess.run(cmd, shell=True).returncode != 0:
            failed.append(cmd)
    after = manifest(a.folder)

    changed = sorted(p for p in before.keys() | after.keys() if before.get(p) != after.get(p))
    for p in changed:
        what = "added" if p not in before else "removed" if p not in after else "changed"
        print(f"ro-check: FAIL {what}: {p}")
    for cmd in failed:
        print(f"ro-check: FAIL command exited non-zero: {cmd}")
    if changed or failed:
        sys.exit(1)
    mode = "read-only" if not is_writable(a.folder) else "writable"
    print(f"ro-check: PASS {len(before)} paths unchanged in {a.folder} ({mode}), {len(a.cmd)} commands")


if __name__ == "__main__":
    main()
