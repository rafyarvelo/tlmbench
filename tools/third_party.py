#!/usr/bin/env python3
"""Writes THIRD_PARTY_LICENSES.md (UTF-8, the same bytes on every OS): every crate and npm package the shipped code is built from, with its licence and the
licence texts it ships. Plain Python 3; needs `cargo` and the UI's node_modules (npm ci in app/ui).

    python3 tools/third_party.py           write it
    python3 tools/third_party.py --check   fail if the file is out of date (a dependency changed)

Crates: what the workspace's members (the app, the CLI, the core crates) and the WASM crates (crates/web,
crates/web-parquet: the viewer and browser-only mode) and the Python extension reach through normal dependencies, on any
platform; build scripts' and tests' dependencies are not shipped. npm: the UI's `dependencies` (what Vite bundles into
the app, the viewer and the browser page), with what they depend on.
"""
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "THIRD_PARTY_LICENSES.md"
NAMES = ("LICENSE", "LICENCE", "COPYING", "NOTICE", "UNLICENSE")


def texts(folder: Path) -> list[tuple[str, str]]:
    found = []
    for p in sorted(folder.iterdir()) if folder.is_dir() else []:
        if p.is_file() and p.name.upper().startswith(NAMES):
            try:
                found.append((p.name, p.read_text(encoding="utf-8", errors="replace").strip()))
            except OSError:
                pass
    return found


def crates(manifest: Path) -> dict:
    meta = json.loads(subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--manifest-path", str(manifest)],
        capture_output=True, text=True, encoding="utf-8", check=True, cwd=ROOT).stdout)
    pkgs = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    ours = set(meta["workspace_members"])
    seen, todo = set(), list(ours)
    while todo:
        i = todo.pop()
        if i in seen:
            continue
        seen.add(i)
        for d in nodes[i]["deps"]:
            if any(k["kind"] is None for k in d["dep_kinds"]):
                todo.append(d["pkg"])
    out = {}
    for i in seen - ours:
        p = pkgs[i]
        if p["source"] is None:  # a path crate of this repository (tools, plugins, shims)
            continue
        out[(p["name"], p["version"])] = (p.get("license") or p.get("license_file") or "see its texts",
                                          p.get("repository") or "", Path(p["manifest_path"]).parent)
    return out


def npm() -> dict:
    ui = ROOT / "app/ui"
    want = list(json.loads((ui / "package.json").read_text(encoding="utf-8")).get("dependencies", {}))
    out, todo = {}, list(want)
    while todo:
        name = todo.pop()
        d = ui / "node_modules" / name
        pj = d / "package.json"
        if not pj.exists():
            sys.exit(f"{name}: not in app/ui/node_modules (npm ci in app/ui first)")
        m = json.loads(pj.read_text(encoding="utf-8"))
        key = (m["name"], m["version"])
        if key in out:
            continue
        lic = m.get("license") or (m.get("licenses") or [{}])[0].get("type") or "see its texts"
        repo = m.get("repository")
        out[key] = (lic, repo.get("url", "") if isinstance(repo, dict) else (repo or ""), d)
        todo += list(m.get("dependencies", {}))
    return out


def render() -> str:
    rust = {}
    for manifest in ("Cargo.toml", "crates/web/Cargo.toml", "crates/web-parquet/Cargo.toml", "python/Cargo.toml"):
        rust.update(crates(ROOT / manifest))
    js = npm()
    lines = [
        "# Third-party licences",
        "",
        "TelemetryBench is licensed under the MIT License (`LICENSE-MIT`) or the Apache License 2.0 (`LICENSE-APACHE`),",
        "at your choice. It is built with the open-source software below; each is listed with its licence and the licence",
        "texts it ships. Written by `tools/third_party.py`.",
        "",
        "| Package | Version | Licence |",
        "| --- | --- | --- |",
    ]
    for (n, v), (lic, _, _) in sorted(rust.items()):
        lines.append(f"| {n} (Rust) | {v} | {lic} |")
    for (n, v), (lic, _, _) in sorted(js.items()):
        lines.append(f"| {n} (npm) | {v} | {lic} |")
    # each distinct text once, with the packages that ship it
    by_text: dict[str, list[str]] = {}
    for kind, table in (("Rust", rust), ("npm", js)):
        for (n, v), (_, _, folder) in sorted(table.items()):
            for _, t in texts(folder):
                by_text.setdefault(t, []).append(f"{n} {v}")
    lines += ["", "## Licence texts", ""]
    for t, who in sorted(by_text.items(), key=lambda x: x[1][0].lower()):
        lines += [f"### {', '.join(who)}", "", "```", t, "```", ""]
    return "\n".join(lines).rstrip() + "\n"


def main() -> int:
    text = render()
    if "--check" in sys.argv:
        if not OUT.exists() or OUT.read_bytes().decode("utf-8") != text:
            print("THIRD_PARTY_LICENSES.md is out of date: python3 tools/third_party.py")
            return 1
        return 0
    OUT.write_bytes(text.encode("utf-8"))  # UTF-8 and \n on every OS
    print(f"wrote {OUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
