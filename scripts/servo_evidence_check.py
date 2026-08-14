#!/usr/bin/env python3
"""Fail-closed validator for PR-026 real Servo evidence artifacts."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

SERVO_REVISION = "859bd5edd60c0fb162a1f73c083a23e55474faf7"
REQUIRED_CASES = {
    "http_fixture_load",
    "frame_ready_paint_present",
    "text_input",
    "click_link",
    "resize",
    "thread_affinity",
    "shutdown_no_deadlock",
}
SHA40 = re.compile(r"^[0-9a-f]{40}$")
SHA64 = re.compile(r"^[0-9a-f]{64}$")


def validate(artifact: dict) -> list[str]:
    errors: list[str] = []
    required = {
        "status",
        "repository",
        "commit_sha",
        "tree_sha",
        "servo_revision",
        "engine_revision",
        "os_and_arch",
        "surface_strategy",
        "thread_affinity",
        "artifact_digest",
        "frame_digest",
        "load_complete",
        "screenshot_ready",
        "frame_count",
        "cases",
    }
    errors.extend(f"missing field: {field}" for field in sorted(required - artifact.keys()))
    if errors:
        return errors
    if artifact["status"] != "pass":
        errors.append("status must be pass")
    if not artifact["repository"]:
        errors.append("repository identity is empty")
    if not SHA40.fullmatch(artifact["commit_sha"]):
        errors.append("commit_sha must be a 40-character lowercase SHA")
    if not SHA40.fullmatch(artifact["tree_sha"]):
        errors.append("tree_sha must be a 40-character lowercase SHA")
    if artifact["servo_revision"] != SERVO_REVISION:
        errors.append("servo_revision does not match the pinned Servo revision")
    if artifact["engine_revision"] != artifact["servo_revision"]:
        errors.append("engine_revision must match servo_revision for this adapter")
    if not artifact["os_and_arch"]:
        errors.append("os_and_arch is empty")
    if artifact["surface_strategy"] != "software-rendering-context":
        errors.append("unsupported surface strategy")
    if artifact["thread_affinity"] != "engine-thread-only":
        errors.append("thread affinity is not engine-thread-only")
    if not SHA64.fullmatch(artifact["artifact_digest"]):
        errors.append("artifact_digest must be a 64-character lowercase SHA-256")
    if not SHA64.fullmatch(artifact["frame_digest"]):
        errors.append("frame_digest must be a 64-character lowercase SHA-256")
    if artifact["load_complete"] is not True:
        errors.append("load_complete must be true")
    if artifact["screenshot_ready"] is not True:
        errors.append("screenshot_ready must be true")
    if not isinstance(artifact["frame_count"], int) or artifact["frame_count"] < 1:
        errors.append("frame_count must prove at least one frame")
    cases = artifact["cases"]
    if not isinstance(cases, dict):
        errors.append("cases must be an object")
    else:
        missing = REQUIRED_CASES - cases.keys()
        errors.extend(f"missing case: {case}" for case in sorted(missing))
        errors.extend(
            f"case {case} is not pass"
            for case, status in cases.items()
            if status != "pass"
        )
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("artifact", type=Path)
    args = parser.parse_args()
    try:
        artifact = json.loads(args.artifact.read_text())
    except (OSError, json.JSONDecodeError) as error:
        print(f"invalid evidence artifact: {error}", file=sys.stderr)
        return 1
    errors = validate(artifact)
    if errors:
        print("PR-026 evidence rejected:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1
    print(json.dumps({"status": "pass", "artifact": str(args.artifact)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
