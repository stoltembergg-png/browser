#!/usr/bin/env python3
"""Fail-closed, dependency-free baseline checks for repository security hygiene."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path


def main() -> int:
    root = Path.cwd()
    lockfile = root / "Cargo.lock"
    if not lockfile.is_file():
        print("missing Cargo.lock", file=sys.stderr)
        return 1

    lock_text = lockfile.read_text()
    violations: list[str] = []
    if re.search(r"(?m)^source\s*=\s*\"git\+", lock_text):
        violations.append("git source in Cargo.lock")

    secret_patterns = (
        r"gh[pousr]_[A-Za-z0-9_]{20,}",
        r"AKIA[0-9A-Z]{16}",
        r"-----BEGIN (?:RSA|OPENSSH|EC|DSA) PRIVATE KEY-----",
    )
    for path in root.rglob("*"):
        if not path.is_file() or ".git" in path.parts or "target" in path.parts or "__pycache__" in path.parts or path.suffix == ".pyc":
            continue
        try:
            text = path.read_text(errors="ignore")
        except OSError:
            continue
        if any(re.search(pattern, text) for pattern in secret_patterns):
            violations.append(f"secret-like text in {path.relative_to(root)}")

    if violations:
        print("security baseline violations:", file=sys.stderr)
        for violation in violations:
            print(f"- {violation}", file=sys.stderr)
        return 1

    print(json.dumps({"status": "pass", "lockfile": "present", "violations": []}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
