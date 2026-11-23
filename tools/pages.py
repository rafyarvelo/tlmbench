#!/usr/bin/env python3
"""Builds the web copy for GitHub Pages into OUT (`just pages OUT`; the Pages workflow runs it):

    OUT/index.html   a landing page (docs/web/index.html)
    OUT/manual/      the User Manual as HTML (tools/manual/build.mjs --web)
    OUT/app/         browser-only mode, the static site in app/browser/dist (`just viewer` builds it)
    OUT/sample/      an HTML export of tools/testdata/make.py's flight-bad.csv, made by the release CLI

Needs `just viewer` done, the release CLI built after it (`cargo build --release --bin tlmbench`: it embeds the viewer)
and `npm ci` in tools/manual. The recipe does all three. Nothing here publishes anything.
"""

from __future__ import annotations

import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def run(*cmd: object, **kw) -> None:
    subprocess.run([str(c) for c in cmd], check=True, **kw)


def main() -> int:
    if len(sys.argv) != 2:
        sys.exit("usage: tools/pages.py OUT")
    out = Path(sys.argv[1]).resolve()
    site = ROOT / "app/browser/dist"
    viewer = ROOT / "app/viewer/dist"
    cli = next((p for p in (ROOT / "target/release/tlmbench", ROOT / "target/release/tlmbench.exe") if p.is_file()), None)
    for need, what in ((site / "tlmbench_web.wasm", "just viewer"), (viewer / "index.html", "just viewer"), (cli, "cargo build --release --bin tlmbench")):
        if not need or not Path(need).exists():
            sys.exit(f"pages: {need or 'the release CLI'} is missing: run `{what}`")
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)

    run("node", ROOT / "tools/manual/build.mjs", "--web", out / "manual")
    shutil.copytree(site, out / "app")
    shutil.copy(ROOT / "docs/web/index.html", out / "index.html")
    (out / ".nojekyll").write_text("")

    with tempfile.TemporaryDirectory() as tmp:
        run(sys.executable, ROOT / "tools/testdata/make.py", Path(tmp) / "td", stdout=subprocess.DEVNULL)
        flights = Path(tmp) / "td" / "flights"
        run(
            cli, "export", "html", flights / "flight-bad.csv", "--layout", "Altitude and battery", "--checks", "Flight limits",
            "--config", flights / ".tlmbench", "--cache", Path(tmp) / "cache", "--viewer", viewer, "--out", out / "sample",
            stdout=subprocess.DEVNULL,
        )
    size = sum(f.stat().st_size for f in out.rglob("*") if f.is_file())
    print(f"{out}: landing page, manual/, app/, sample/ ({size / 1e6:.1f} MB)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
