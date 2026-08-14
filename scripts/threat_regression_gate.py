#!/usr/bin/env python3
"""Validate and evaluate the machine-readable threat regression gate."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

EXPECTED_IDS = [f"TM-{index:03d}" for index in range(1, 19)]
VALID_STATUSES = {"implemented", "partial", "planned", "deferred", "blocked"}
RELEASE_BLOCKING_STATUSES = {"partial", "planned", "deferred", "blocked"}
REQUIRED_SCENARIO_FIELDS = {"id", "status", "control", "test", "evidence", "release_blocker"}


def load_manifest(path: Path) -> dict:
    return json.loads(path.read_text())


def validate_manifest(data: dict, root: Path) -> list[str]:
    errors: list[str] = []
    if data.get("schema_version") != 1:
        errors.append("schema_version must be 1")
    if data.get("source_of_truth") != "THREAT_MODEL.md":
        errors.append("source_of_truth must be THREAT_MODEL.md")

    scenarios = data.get("scenarios")
    if not isinstance(scenarios, list):
        return ["scenarios must be a list"]
    ids = [scenario.get("id") for scenario in scenarios if isinstance(scenario, dict)]
    if ids != EXPECTED_IDS:
        errors.append(f"scenario ids must be exactly {EXPECTED_IDS}")

    for index, scenario in enumerate(scenarios):
        if not isinstance(scenario, dict):
            errors.append(f"scenario {index} must be an object")
            continue
        missing = REQUIRED_SCENARIO_FIELDS - scenario.keys()
        if missing:
            errors.append(f"{scenario.get('id', index)} missing fields: {sorted(missing)}")
            continue
        scenario_id = scenario["id"]
        status = scenario["status"]
        if status not in VALID_STATUSES:
            errors.append(f"{scenario_id} has invalid status {status!r}")
        if not isinstance(scenario["control"], str) or not scenario["control"].strip():
            errors.append(f"{scenario_id} requires a non-empty control")
        if not isinstance(scenario["evidence"], str) or not scenario["evidence"].strip():
            errors.append(f"{scenario_id} requires a non-empty evidence note")
        if not isinstance(scenario["release_blocker"], bool):
            errors.append(f"{scenario_id} release_blocker must be boolean")
        elif (status in RELEASE_BLOCKING_STATUSES) != scenario["release_blocker"]:
            errors.append(f"{scenario_id} release_blocker does not match status {status}")

        test = scenario["test"]
        if not isinstance(test, dict) or set(test) != {"available", "path", "selector"}:
            errors.append(f"{scenario_id} test must contain available, path and selector")
            continue
        available = test["available"]
        path_text = test["path"]
        selector = test["selector"]
        if not isinstance(available, bool):
            errors.append(f"{scenario_id} test.available must be boolean")
        if available:
            if not isinstance(path_text, str) or not path_text:
                errors.append(f"{scenario_id} available test requires a path")
            elif not (root / path_text).is_file():
                errors.append(f"{scenario_id} test path does not exist: {path_text}")
            if not isinstance(selector, str) or not selector.strip():
                errors.append(f"{scenario_id} available test requires a selector")
        elif path_text is not None or selector is not None:
            errors.append(f"{scenario_id} unavailable test must use null path and selector")
        if status == "implemented" and not available:
            errors.append(f"{scenario_id} implemented status requires an available test")
        if status in RELEASE_BLOCKING_STATUSES and not scenario["release_blocker"]:
            errors.append(f"{scenario_id} non-implemented status must block release")

    return errors


def evaluate_release(data: dict, channel: str) -> dict:
    blockers = [
        scenario["id"]
        for scenario in data["scenarios"]
        if scenario["status"] != "implemented" or scenario["release_blocker"]
    ]
    return {
        "status": "GO" if not blockers else "NO_GO",
        "channel": channel,
        "blockers": blockers,
        "scenario_count": len(data["scenarios"]),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--manifest",
        type=Path,
        default=Path("docs/security/threat-regression-manifest.json"),
    )
    parser.add_argument("--channel", choices=("mvp", "alpha", "beta", "stable"), default="alpha")
    parser.add_argument("--validate-only", action="store_true")
    args = parser.parse_args()

    try:
        data = load_manifest(args.manifest)
    except (OSError, json.JSONDecodeError) as error:
        print(f"threat regression manifest invalid: {error}", file=sys.stderr)
        return 1

    errors = validate_manifest(data, Path.cwd())
    if errors:
        print("threat regression manifest violations:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    if args.validate_only:
        report = {
            "status": "pass",
            "scenario_count": len(data["scenarios"]),
            "scenario_ids": [scenario["id"] for scenario in data["scenarios"]],
            "blocking_scenarios": [
                scenario["id"]
                for scenario in data["scenarios"]
                if scenario["status"] != "implemented" or scenario["release_blocker"]
            ],
        }
        print(json.dumps(report, sort_keys=True))
        return 0

    report = evaluate_release(data, args.channel)
    print(json.dumps(report, sort_keys=True))
    return 0 if report["status"] == "GO" else 1


if __name__ == "__main__":
    raise SystemExit(main())
