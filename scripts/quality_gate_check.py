#!/usr/bin/env python3
"""Validate the checked-in quality-gate manifest without claiming GitHub enforcement."""

from __future__ import annotations

import json
import sys
from pathlib import Path


REQUIRED_EVENTS = {"pull_request", "push", "merge_group"}
REQUIRED_FIELDS = {"repository", "event", "base_sha", "head_sha", "tree_sha", "run_id", "attempt", "workflow_revision", "status", "steps"}


def validate_manifest(data: dict) -> list[str]:
    errors: list[str] = []
    if data.get("schema_version") != 1:
        errors.append("schema_version must be 1")
    if data.get("check_name") != "CI / Quality Gate":
        errors.append("check_name must be CI / Quality Gate")
    if data.get("fail_closed") is not True:
        errors.append("fail_closed must be true")
    if set(data.get("events", [])) != REQUIRED_EVENTS:
        errors.append("events must be exactly pull_request, push and merge_group")
    if set(data.get("evidence_fields", [])) != REQUIRED_FIELDS:
        errors.append("evidence_fields do not match the evidence identity contract")
    if data.get("status_model") != ["UNVERIFIED", "OFF", "SHADOW", "ENFORCED"]:
        errors.append("status_model must preserve the documented transition states")
    if not data.get("required_steps"):
        errors.append("required_steps must not be empty")
    return errors


def main() -> int:
    path = Path("docs/ci/quality-gate-manifest.json")
    try:
        data = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        print(f"quality-gate manifest invalid: {error}", file=sys.stderr)
        return 1
    errors = validate_manifest(data)
    if errors:
        print("quality-gate manifest violations:", file=sys.stderr)
        print("\n".join(f"- {error}" for error in errors), file=sys.stderr)
        return 1
    print(json.dumps({"status": "pass", "check_name": data["check_name"], "fail_closed": True}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
