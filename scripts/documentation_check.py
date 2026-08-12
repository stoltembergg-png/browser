#!/usr/bin/env python3
"""Fail-closed check that every document marked present exists."""

from __future__ import annotations

import json
import sys
from pathlib import Path



def _present_paths(text: str) -> list[str]:
    paths: list[str] = []
    in_present = False
    for line in text.splitlines():
        if line == "present:":
            in_present = True
            continue
        if in_present and line and not line.startswith("  - "):
            break
        if in_present and line.startswith("  - "):
            paths.append(line[4:])
    return paths


def main() -> int:
    root = Path.cwd()
    authority_path = root / "docs" / "document-authority.yaml"
    if not authority_path.is_file():
        print(f"missing authority file: {authority_path}", file=sys.stderr)
        return 1

    required = _present_paths(authority_path.read_text())
    if not required:
        print("document authority 'present' must be a non-empty list of paths", file=sys.stderr)
        return 1

    missing = [path for path in required if not (root / path).is_file()]
    if missing:
        print("missing required documents:", file=sys.stderr)
        for path in missing:
            print(f"- {path}", file=sys.stderr)
        return 1

    print(json.dumps({"status": "pass", "required": len(required), "missing": []}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
