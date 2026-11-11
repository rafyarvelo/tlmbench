#!/usr/bin/env python3
"""Checks docs/release/ledger.md, the list of v1 requirements and the evidence for each.

Fails (exit 1) when:
  - a row has no ID, a repeated ID, or a verdict that isn't one of VERDICTS;
  - a Done row names no evidence (a test, a file or a command);
  - a file named as evidence doesn't exist;
  - a UI test named as evidence (`t_name`) is in neither app/ui/tests/ nor prototype/tests/;
  - a Rust or Python test named as evidence (`crate::path::test_fn`, `file.py::test_fn`) has no such function;
  - a row other than Done names no gap;
  - the counts at the top of the ledger differ from the table.

With --release it also fails on any row that isn't Done or a signed-off Owner pass ("Owner pass (signed off <date>)"):
that is the release gate.

How evidence is written in the ledger (all in backticks):
  `t_tables`                        a UI test; `proto:t_tables` names the prototype's test of that name
  `crates/core/src/table.rs:120`    a file, with or without a line
  `tlmbench-core::table::filter_syntax`   a Rust test, by its function name after the last `::`
  `python/tests/test_recording.py::test_view_keeps_the_spike`   a Python test
  `just gate5`, `cargo …`, `tlmbench …`, `node …`, `python3 …`   a command

Usage: python3 ci/ledger_check.py [--release] [ledger.md]
Plain Python 3, no packages.
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
VERDICTS = ("Done", "Partial", "Missing", "Diverged", "Unverified", "Owner pass")
COMMANDS = ("just ", "cargo ", "tlmbench ", "node ", "python3 ", "pytest ", "npm ", "npx ", "gh ", "grep ")
UI_TEST_DIRS = ("app/ui/tests", "prototype/tests")
RUST_DIRS = ("crates", "cli", "app/src-tauri", "tools", "python/src")
ROOTS = ("crates/", "cli/", "app/", "prototype/", "docs/", "ci/", "python/", "plugins/", "examples/", "tools/", ".github/")


def cells(line):
    """The cells of a Markdown table row; `\\|` is a literal bar."""
    parts = re.split(r"(?<!\\)\|", line.strip())
    return [p.strip().replace("\\|", "|") for p in parts[1:-1]]


def rust_test_names():
    names = set()
    for d in RUST_DIRS:
        for p in (ROOT / d).rglob("*.rs"):
            if "target" in p.parts:
                continue
            names.update(re.findall(r"\bfn ([a-z_0-9]+)\s*\(", p.read_text(errors="replace")))
    return names


def ui_test_exists(name, proto_only=False):
    dirs = UI_TEST_DIRS[1:] if proto_only else UI_TEST_DIRS
    return any((ROOT / d / (name + ext)).is_file() for d in dirs for ext in (".js", ".mjs"))


def main(argv):
    release = "--release" in argv
    args = [a for a in argv if not a.startswith("--")]
    path = pathlib.Path(args[0]) if args else ROOT / "docs/release/ledger.md"
    text = path.read_text()
    errors = []
    rows = []
    header = None
    for n, line in enumerate(text.splitlines(), 1):
        if not line.startswith("|"):
            continue
        c = cells(line)
        if header is None:
            if c and c[0] == "ID":
                header = c
            continue
        if all(set(x) <= set("-: ") for x in c):
            continue
        if len(c) != len(header):
            errors.append(f"line {n}: {len(c)} cells, the table has {len(header)}")
            continue
        rows.append((n, dict(zip(header, c))))
    if header is None:
        print(f"{path}: no table with an ID column")
        return 1
    need = ["ID", "Source", "Requirement (one sentence)", "Verdict", "Evidence", "Gap and the step that closes it"]
    if header != need:
        errors.append(f"columns are {header}, expected {need}")
        print("\n".join(errors))
        return 1

    fns = rust_test_names()
    seen = set()
    counts = dict.fromkeys(VERDICTS, 0)
    for n, r in rows:
        rid = r["ID"]
        where = f"line {n} ({rid or 'no ID'})"
        if not re.fullmatch(r"(S\d\.\d+-\d\d|D-[a-z0-9]+-\d\d|A-\d\d|X-[a-z0-9]+-\d\d)", rid):
            errors.append(f"{where}: the ID isn't S<step>-NN, D-<section>-NN, A-NN or X-<area>-NN")
        if rid in seen:
            errors.append(f"{where}: the ID is used twice")
        seen.add(rid)
        verdict = r["Verdict"]
        base = next((v for v in VERDICTS if verdict == v or verdict.startswith(v + " (")), None)
        if base is None:
            errors.append(f"{where}: verdict {verdict!r} isn't one of {', '.join(VERDICTS)}")
            continue
        counts[base] += 1
        ev = r["Evidence"]
        ticks = re.findall(r"`([^`]+)`", ev)
        named = 0
        for t in ticks:
            if t.startswith(COMMANDS):
                named += 1
                continue
            m = re.fullmatch(r"(proto:)?(t_[a-z0-9_]+)", t)
            if m:
                named += 1
                if not ui_test_exists(m.group(2), proto_only=bool(m.group(1))):
                    errors.append(f"{where}: no test {t} in {' or '.join(UI_TEST_DIRS)}")
                continue
            if "::" in t:
                named += 1
                head, fn = t.rsplit("::", 1)
                if head.endswith(".py"):
                    f = ROOT / head
                    if not f.is_file() or not re.search(rf"\bdef {re.escape(fn)}\s*\(", f.read_text()):
                        errors.append(f"{where}: no Python test {t}")
                elif fn not in fns:
                    errors.append(f"{where}: no Rust function named {fn} ({t})")
                continue
            p = re.sub(r":\d+(-\d+)?$", "", t)
            if p.startswith(ROOTS) or p in ("justfile", "Cargo.toml", "README.md", "CLAUDE.md"):
                named += 1
                if not (ROOT / p).exists():
                    errors.append(f"{where}: {p} doesn't exist")
        if base == "Done" and named == 0:
            errors.append(f"{where}: Done, but the evidence names no test, file or command")
        gap = r["Gap and the step that closes it"]
        if base != "Done" and len(gap) < 2:
            errors.append(f"{where}: {verdict}, but no gap or closing step is given")
        if release and not (base == "Done" or re.fullmatch(r"Owner pass \(signed off [^)]+\)", verdict)):
            errors.append(f"{where}: {verdict} (the release needs Done, or an Owner pass signed off)")

    # the counts at the top: "| Done | 123 |" lines before the table, or "Done 123" in a sentence
    top = text.split("| ID |", 1)[0]
    for v in VERDICTS:
        m = re.search(rf"\*\*{re.escape(v)}\*\*[ :]+(\d+)", top)
        if not m:
            errors.append(f"the counts at the top don't give **{v}** N")
        elif int(m.group(1)) != counts[v]:
            errors.append(f"the top says {v} {m.group(1)}, the table has {counts[v]}")

    total = sum(counts.values())
    print(f"{path.relative_to(ROOT) if path.is_absolute() and ROOT in path.parents else path}: {total} rows: "
          + ", ".join(f"{v} {counts[v]}" for v in VERDICTS))
    if errors:
        print(f"{len(errors)} problem(s):")
        for e in errors:
            print("  " + e)
        return 1
    print("ok" + (" for release" if release else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
