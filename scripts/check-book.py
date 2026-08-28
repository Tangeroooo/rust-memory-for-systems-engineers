#!/usr/bin/env python3
"""Check local Markdown links and SUMMARY coverage without network access."""

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "src"
LINK = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")


def markdown_files() -> list[Path]:
    return sorted(SRC.rglob("*.md"))


def check_local_links(files: list[Path]) -> list[str]:
    errors: list[str] = []
    for path in files:
        text = path.read_text(encoding="utf-8")
        for target in LINK.findall(text):
            target = target.strip().split()[0].strip("<>")
            if target.startswith(("http://", "https://", "mailto:", "#")):
                continue
            relative = target.split("#", 1)[0]
            if not relative:
                continue
            resolved = (path.parent / relative).resolve()
            if not resolved.exists():
                errors.append(f"{path.relative_to(ROOT)}: missing link target {target}")
    return errors


def check_summary(files: list[Path]) -> list[str]:
    summary = (SRC / "SUMMARY.md").read_text(encoding="utf-8")
    listed = {
        (SRC / target.split("#", 1)[0]).resolve()
        for target in LINK.findall(summary)
        if target.endswith(".md")
    }
    expected = {path.resolve() for path in files if path.name != "SUMMARY.md"}
    return [
        f"src/SUMMARY.md: chapter not listed: {path.relative_to(SRC)}"
        for path in sorted(expected - listed)
    ]


def main() -> int:
    files = markdown_files()
    errors = check_local_links(files) + check_summary(files)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"checked {len(files)} Markdown files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
