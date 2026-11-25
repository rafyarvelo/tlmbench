#!/usr/bin/env python3
"""The release test plan per OS: docs/release/test-plan.md is the one plan; this writes test-plan-macos.md,
test-plan-windows.md and test-plan-linux.md from it, each with only its own system's items and wording.

    python3 tools/testplan.py            write the three copies
    python3 tools/testplan.py --check    exit 1 when a copy is out of date with the plan

A copy that exists keeps what was written in it: each item's result (everything after `Result:`), found by the
item's number, and the header's OS line. So edit the plan, run this, and the copies follow without losing results.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PLAN = ROOT / "docs/release/test-plan.md"
OSES = {"mac": ("macOS", "test-plan-macos.md"), "win": ("Windows", "test-plan-windows.md"), "linux": ("Linux", "test-plan-linux.md")}

# Text of the plan that names more than one system, and what each copy says instead (an exact match is required:
# a phrase the plan no longer has fails loudly, so this list can't silently fall behind it)
SWAPS = [
    (
        "After\n`Result:` write **P** (pass) or **F** (fail); add a few words only for an F. Skip an item that doesn't apply to the OS\n(marked **mac**, **win** or **linux**) by writing **–**.",
        {k: "After\n`Result:` write **P** (pass) or **F** (fail); add a few words only for an F." for k in OSES},
    ),
    (
        "This is the plan; don't fill it in. Each OS has its copy, with only its own items and wording: `test-plan-macos.md`,\n`test-plan-windows.md`, `test-plan-linux.md`. Fill in yours and hand it back. After changing this plan, run\n`python3 tools/testplan.py`: it rewrites the copies and keeps what was written in them.",
        {k: f"This is the {OSES[k][0]} copy of `test-plan.md` (written by `tools/testplan.py`, which keeps the results): fill it\nin and hand it back." for k in OSES},
    ),
    (
        "It needs Python 3.9 or newer (`python3 --version`; macOS and\n   Linux have it, on Windows install it from python.org). In a terminal (macOS, Linux):\n",
        {
            "mac": "It needs Python 3.9 or newer (`python3 --version`; macOS has\n   it). In Terminal:\n",
            "linux": "It needs Python 3.9 or newer (`python3 --version`; Linux has\n   it). In a terminal:\n",
            "win": "It needs Python 3.9 or newer (install it from python.org; `py\n   --version` checks it). In PowerShell:\n",
        },
    ),
    (
        "   ```\n   cd ~ && curl -fsSLO https://raw.githubusercontent.com/rafyarvelo/tlmbench/main/tools/testdata/make.py\n   python3 make.py big       # (everything, with the 400 MB file for section 9; without \"big\", the rest only)\n   ```\n   On Windows, in PowerShell:\n   ```\n   cd ~; curl.exe -fsSLO https://raw.githubusercontent.com/rafyarvelo/tlmbench/main/tools/testdata/make.py\n   py make.py big\n   ```\n",
        {
            "mac": "   ```\n   cd ~ && curl -fsSLO https://raw.githubusercontent.com/rafyarvelo/tlmbench/main/tools/testdata/make.py\n   python3 make.py big       # (everything, with the 400 MB file for section 9; without \"big\", the rest only)\n   ```\n",
            "linux": "   ```\n   cd ~ && curl -fsSLO https://raw.githubusercontent.com/rafyarvelo/tlmbench/main/tools/testdata/make.py\n   python3 make.py big       # (everything, with the 400 MB file for section 9; without \"big\", the rest only)\n   ```\n",
            "win": "   ```\n   cd ~; curl.exe -fsSLO https://raw.githubusercontent.com/rafyarvelo/tlmbench/main/tools/testdata/make.py\n   py make.py big            # (everything, with the 400 MB file for section 9; without \"big\", the rest only)\n   ```\n",
        },
    ),
    (
        "run `python3 -m pip install pyarrow` (Windows:\n   `py -m pip install pyarrow`) and `make.py big` again.",
        {
            "mac": "run `python3 -m pip install pyarrow` and\n   `python3 make.py big` again.",
            "linux": "run `python3 -m pip install pyarrow` and\n   `python3 make.py big` again.",
            "win": "run `py -m pip install pyarrow` and\n   `py make.py big` again.",
        },
    ),
    (
        "\"Checking a download\" (macOS and Linux: `shasum -a 256 -c SHA256SUMS --ignore-missing`; Windows: its PowerShell\nlines). The installer's line says OK (Windows: `True`).",
        {
            "mac": "\"Checking a download\" (`shasum -a 256 -c SHA256SUMS --ignore-missing` in Terminal). The installer's line says OK.",
            "linux": "\"Checking a download\" (`shasum -a 256 -c SHA256SUMS --ignore-missing`). The installer's line says OK.",
            "win": "\"Checking a download\" (its PowerShell lines). It prints `True`.",
        },
    ),
    (
        "About TelemetryBench (macOS: the app menu; Windows and Linux: end of Help)",
        {"mac": "About TelemetryBench (the app menu)", "win": "About TelemetryBench (end of Help)", "linux": "About TelemetryBench (end of Help)"},
    ),
    ("Hold Option (Windows/Linux: Ctrl) while dropping", {"mac": "Hold Option while dropping", "win": "Hold Ctrl while dropping", "linux": "Hold Ctrl while dropping"}),
    ("⌘Z (Ctrl+Z)", {"mac": "⌘Z", "win": "Ctrl+Z", "linux": "Ctrl+Z"}),
    ("⇧⌘Z (Ctrl+Shift+Z)", {"mac": "⇧⌘Z", "win": "Ctrl+Shift+Z", "linux": "Ctrl+Shift+Z"}),
    (
        "make a copy of `flights` read-only (Windows: Properties › Read-only on the folder and\nits files; Linux: `chmod -R a-w`)",
        {
            "mac": "",  # the item is Windows and Linux only
            "win": "make a copy of `flights` read-only (Properties › Read-only on the folder and its\nfiles)",
            "linux": "make a copy of `flights` read-only (`chmod -R a-w` on it)",
        },
    ),
    (
        "Turn on Reduce motion (macOS: Accessibility › Display; Windows: Settings › Accessibility › Visual effects ›\nAnimation effects off)",
        {
            "mac": "Turn on Reduce motion (System Settings › Accessibility › Display)",
            "win": "Turn off animations (Settings › Accessibility › Visual effects › Animation effects off)",
            "linux": "Turn on your desktop's setting to reduce animation (GNOME: Settings › Accessibility › Reduce\nAnimation)",
        },
    ),
]
# after the swaps: shortcuts the plan writes for the Mac only
KEYS = {"win": [("⇧⌘Z", "Ctrl+Shift+Z"), ("⌘Z", "Ctrl+Z")], "linux": [("⇧⌘Z", "Ctrl+Shift+Z"), ("⌘Z", "Ctrl+Z")], "mac": []}

ITEM = re.compile(r"^\*\*(\d+\.\d+)((?: [a-z/]+)?)\*\*", re.M)


def blocks(text):
    """The plan's paragraphs (split at blank lines), each with the item number it starts with, if any."""
    out = []
    for b in text.split("\n\n"):
        m = ITEM.match(b)
        out.append((m.group(1) if m else None, m.group(2).strip() if m else "", b))
    return out


def for_os(plan, os_key):
    text = plan
    for old, new in SWAPS:
        if old not in text:
            sys.exit(f"testplan.py: the plan no longer has this text (update SWAPS):\n{old}")
        text = text.replace(old, new[os_key])
    for a, b in KEYS[os_key]:
        text = text.replace(a, b)
    kept = []
    for num, tags, b in blocks(text):
        if tags and os_key not in tags.split("/"):
            continue
        if num and tags:
            b = b.replace(f"**{num} {tags}**", f"**{num}**", 1)
        kept.append(b)
    name = OSES[os_key][0]
    out = "\n\n".join(kept)
    out = out.replace("# Release test plan\n", f"# Release test plan: {name}\n", 1)
    if os_key != "mac":
        # the manual's screenshots taken on a Mac have no place on the others; nor has "Retina"
        out = "\n\n".join(b for n, _, b in blocks(out) if not (n and "*(for the manual, on a Mac)*" in b))
        out = out.replace("**Screen:** (Retina / normal / both)", "**Screen:** (HiDPI / normal / both)", 1)
    return out


def carry(new, old):
    """Each item's result and the OS line from an existing copy."""
    results = {}
    for num, _, b in blocks(old):
        if num and "Result:" in b:
            results[num] = b[b.rindex("Result:"):]
    os_line = re.search(r"^\*\*OS:\*\*.*$", old, re.M)
    out = []
    for num, _, b in blocks(new):
        if num in results and "Result:" in b:
            b = b[: b.rindex("Result:")] + results[num]
        out.append(b)
    text = "\n\n".join(out)
    if os_line:
        text = re.sub(r"^\*\*OS:\*\*.*$", lambda _: os_line.group(0), text, count=1, flags=re.M)
    return text


def main():
    check = "--check" in sys.argv[1:]
    plan = PLAN.read_text(encoding="utf-8")
    stale = []
    for key, (_, file) in OSES.items():
        path = PLAN.parent / file
        text = for_os(plan, key)
        if path.exists():
            text = carry(text, path.read_text(encoding="utf-8"))
        if path.exists() and path.read_text(encoding="utf-8") == text:
            continue
        if check:
            stale.append(file)
        else:
            path.write_text(text, encoding="utf-8", newline="\n")
            print(f"wrote {path.relative_to(ROOT)}")
    if stale:
        sys.exit(f"out of date with test-plan.md: {', '.join(stale)} (python3 tools/testplan.py)")


if __name__ == "__main__":
    main()
