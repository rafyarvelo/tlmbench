#!/usr/bin/env python3
"""The release's version in one place (R4): the workspace version in Cargo.toml is the source; tauri.conf.json, the
Python package and the crates outside the workspace (the bindings, the viewer's WASM core) must say the same. With
--tag v1.2.3 (or v1.2.3-rc.1, a dry run of 1.2.3) the tag must name it too. Exits 1 and says which file differs.

    python3 tools/check_versions.py [--tag v1.0.0-rc.1]
"""

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def toml_version(path: Path, section: str) -> str | None:
    """`version = "…"` under `[section]` of a TOML file (a plain read: these files keep it on one line)."""
    inside = False
    for line in path.read_text().splitlines():
        s = line.strip()
        if s.startswith("["):
            inside = s == f"[{section}]"
        elif inside and (m := re.match(r'version\s*=\s*"([^"]+)"', s)):
            return m.group(1)
    return None


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--tag", help="the release tag, v<version>")
    a = ap.parse_args()
    want = toml_version(ROOT / "Cargo.toml", "workspace.package")
    if not want:
        print("Cargo.toml: no [workspace.package] version")
        return 1
    found = {
        "app/src-tauri/tauri.conf.json": json.loads((ROOT / "app/src-tauri/tauri.conf.json").read_text()).get("version"),
        "python/pyproject.toml": toml_version(ROOT / "python/pyproject.toml", "project"),
        "python/Cargo.toml": toml_version(ROOT / "python/Cargo.toml", "package"),
        "crates/web/Cargo.toml": toml_version(ROOT / "crates/web/Cargo.toml", "package"),
        "crates/web-parquet/Cargo.toml": toml_version(ROOT / "crates/web-parquet/Cargo.toml", "package"),
    }
    bad = [f"{f}: {v!r}, Cargo.toml says {want!r}" for f, v in found.items() if v != want]
    # a release candidate (v1.0.0-rc.1) is a dry run of the version it names
    if a.tag is not None and not re.fullmatch(rf"v{re.escape(want)}(-rc\.[0-9]+)?", a.tag):
        bad.append(f"the tag {a.tag!r} isn't v{want} or v{want}-rc.N")
    for b in bad:
        print(b)
    if not bad:
        print(f"version {want} everywhere" + (f", tag {a.tag}" if a.tag else ""))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
