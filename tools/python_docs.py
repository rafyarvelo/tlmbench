#!/usr/bin/env python3
"""Writes the Python API reference (docs/python-api.md) from the `tlmbench` module's own docstrings, so it can't drift.

    just python-docs            # needs the extension built into the venv (`just setup`, or `maturin develop`)

With `--check` it compares instead of writing, and exits 1 when the file is out of date (CI runs it).
"""

from __future__ import annotations

import inspect
import sys
from pathlib import Path

import tlmbench

OUT = Path(__file__).resolve().parent.parent / "docs" / "python-api.md"


def sig(fn) -> str:
    try:
        return str(inspect.signature(fn)).replace("tlmbench.", "")
    except (TypeError, ValueError):
        return "(...)"


def block(name: str, obj, level: str, prop: bool = False) -> list[str]:
    doc = inspect.getdoc(obj) or ""
    if prop:
        # a property reads as an attribute: its type, not a call
        ret = sig(obj).rpartition(" -> ")[2] if " -> " in sig(obj) else ""
        head = f"{level} `{name}: {ret}` (property)" if ret else f"{level} `{name}` (property)"
    else:
        head = f"{level} `{name}{sig(obj)}`" if callable(obj) else f"{level} `{name}`"
    return [head, "", doc, ""] if doc else [head, ""]


def render() -> str:
    lines = [
        "# Python API",
        "",
        "Generated from the docstrings of the `tlmbench` module by `tools/python_docs.py` (`just python-docs`). Do not edit.",
        "",
        "```python",
        "import tlmbench",
        "```",
        "",
        inspect.getdoc(tlmbench) or "",
        "",
        "## Functions",
        "",
    ]
    for name in ("open", "config"):
        lines += block(f"tlmbench.{name}", getattr(tlmbench, name), "###")
    for cls_name in ("Recording", "Channel", "Config", "ConfigObject"):
        cls = getattr(tlmbench, cls_name)
        lines += ["", f"## `{cls_name}`", "", inspect.getdoc(cls) or "", ""]
        for name, member in inspect.getmembers(cls):
            if name.startswith("_"):
                continue
            if isinstance(member, property):
                lines += block(f"{cls_name}.{name}", member.fget, "###", prop=True)
            elif inspect.isfunction(member):
                lines += block(f"{cls_name}.{name}", member, "###")
    return "\n".join(lines).rstrip() + "\n"


def main() -> int:
    text = render()
    if "--check" in sys.argv:
        if not OUT.exists() or OUT.read_text() != text:
            print(f"{OUT.name} is out of date: run `just python-docs`", file=sys.stderr)
            return 1
        return 0
    OUT.write_text(text)
    print(f"wrote {OUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
