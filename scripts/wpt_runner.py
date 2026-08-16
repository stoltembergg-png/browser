#!/usr/bin/env python3
"""Run a pinned, deterministic WPT subset through an explicit local adapter.

The runner never downloads tests or interprets network URLs. The adapter is a
separate executable boundary and receives WPT_OFFLINE=1; adapters must enforce
that policy for the browser process they launch. Missing, skipped, timed out,
or malformed adapter results are NO_GO.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

WPT_REVISION_RE = re.compile(r"^[0-9a-f]{40}$")
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
ALLOWED_ACTUAL = {"pass", "fail", "timeout", "notrun", "error"}
EXPECTED = {"pass", "fail"}
FAIL_OUTCOMES = {"fail", "unexpected-pass", "timeout", "notrun", "error"}
COUNT_KEYS = ("pass", "expected-fail", "fail", "unexpected-pass", "timeout", "notrun", "error")


class RunnerError(ValueError):
    """A manifest, identity, fixture, or adapter protocol violation."""


def canonical_json(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode(
        "utf-8"
    )


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def require_sha(name: str, value: Any) -> None:
    if not isinstance(value, str) or not WPT_REVISION_RE.fullmatch(value):
        raise RunnerError(f"{name} must be a 40-character lowercase SHA")


def require_non_empty(name: str, value: Any) -> None:
    if not isinstance(value, str) or not value.strip():
        raise RunnerError(f"{name} must be a non-empty string")


def validate_manifest(data: Any) -> list[dict[str, Any]]:
    if not isinstance(data, dict):
        raise RunnerError("manifest root must be an object")
    if data.get("schema_version") != 1:
        raise RunnerError("manifest schema_version must be 1")
    require_sha("wpt_revision", data.get("wpt_revision"))
    if data.get("adapter_protocol") != 1:
        raise RunnerError("adapter_protocol must be 1")
    tests = data.get("tests")
    if not isinstance(tests, list) or not tests:
        raise RunnerError("manifest tests must be a non-empty array")

    validated: list[dict[str, Any]] = []
    seen: set[str] = set()
    for index, case in enumerate(tests):
        if not isinstance(case, dict):
            raise RunnerError(f"test {index} must be an object")
        test_id = case.get("id")
        path = case.get("path")
        expected = case.get("expected")
        require_non_empty(f"test {index}.id", test_id)
        require_non_empty(f"test {index}.path", path)
        if test_id in seen:
            raise RunnerError(f"duplicate test id: {test_id}")
        seen.add(test_id)
        if expected not in EXPECTED:
            raise RunnerError(f"test {test_id}.expected must be pass or fail")
        fixture_sha256 = case.get("fixture_sha256")
        if not isinstance(fixture_sha256, str) or not SHA256_RE.fullmatch(fixture_sha256):
            raise RunnerError(f"test {test_id}.fixture_sha256 must be a 64-character lowercase SHA-256")
        path_value = Path(path)
        if path_value.is_absolute() or path.replace("\\", "/").startswith("/"):
            raise RunnerError(f"test {test_id}.path must be relative")
        if ".." in path.replace("\\", "/").split("/"):
            raise RunnerError(f"test {test_id}.path must stay inside the manifest root")
        if expected == "fail":
            for field in ("owner", "reason", "recheck_after"):
                require_non_empty(f"test {test_id}.{field}", case.get(field))
            try:
                dt.date.fromisoformat(case["recheck_after"])
            except (TypeError, ValueError) as error:
                raise RunnerError(f"test {test_id}.recheck_after must be YYYY-MM-DD") from error
        validated.append({"id": test_id, "path": path, "expected": expected, **case})
    return validated


def fixture_path(root: Path, relative_path: str, test_id: str, expected_sha256: str) -> Path:
    candidate = (root / relative_path).resolve()
    try:
        candidate.relative_to(root)
    except ValueError as error:
        raise RunnerError(f"test {test_id}.path escapes the manifest root") from error
    if not candidate.is_file():
        raise RunnerError(f"fixture missing for {test_id}: {relative_path}")
    actual_sha256 = sha256_bytes(candidate.read_bytes())
    if actual_sha256 != expected_sha256:
        raise RunnerError(
            f"fixture identity mismatch for {test_id}: expected {expected_sha256}, "
            f"got {actual_sha256}"
        )
    return candidate


def invoke_adapter(
    adapter_command: list[str],
    test: dict[str, Any],
    fixture: Path,
    root: Path,
    timeout_seconds: float,
    environment: dict[str, str],
) -> tuple[str, str | None]:
    command = [*adapter_command, "--test-id", test["id"], "--fixture", str(fixture)]
    try:
        completed = subprocess.run(
            command,
            cwd=root,
            env=environment,
            capture_output=True,
            text=True,
            timeout=timeout_seconds,
            check=False,
        )
    except FileNotFoundError as error:
        return "error", f"adapter executable not found: {error.filename}"
    except subprocess.TimeoutExpired:
        return "timeout", None
    if completed.returncode != 0:
        return "error", f"adapter exited with code {completed.returncode}"
    try:
        response = json.loads(completed.stdout)
    except json.JSONDecodeError:
        return "error", "adapter stdout is not one JSON object"
    if not isinstance(response, dict):
        return "error", "adapter response must be an object"
    uncontracted = set(response) - {"test_id", "status"}
    if uncontracted:
        return "error", "adapter response has uncontracted fields: " + ", ".join(sorted(uncontracted))
    if response.get("test_id") != test["id"]:
        return "error", "adapter test_id does not match the manifest"
    status = response.get("status")
    if status not in ALLOWED_ACTUAL:
        return "error", "adapter status is not pass/fail/timeout/notrun/error"
    return status, None


def classify(expected: str, actual: str) -> str:
    if actual in {"timeout", "notrun", "error"}:
        return actual
    if actual == expected:
        return "pass" if expected == "pass" else "expected-fail"
    if actual == "pass":
        return "unexpected-pass"
    return "fail"


def run_manifest(
    manifest_path: Path,
    output_path: Path,
    adapter_command: list[str],
    repository: str,
    commit_sha: str,
    tree_sha: str,
    engine_revision: str,
    os_and_arch: str,
    timeout_seconds: float,
) -> dict[str, Any]:
    require_non_empty("repository", repository)
    require_sha("commit_sha", commit_sha)
    require_sha("tree_sha", tree_sha)
    require_non_empty("engine_revision", engine_revision)
    require_non_empty("os_and_arch", os_and_arch)
    if not adapter_command:
        raise RunnerError("adapter command must not be empty")
    try:
        manifest_bytes = manifest_path.read_bytes()
        manifest = json.loads(manifest_bytes)
    except (OSError, json.JSONDecodeError) as error:
        raise RunnerError(f"manifest cannot be read as JSON: {error}") from error
    tests = validate_manifest(manifest)
    root = manifest_path.parent.resolve()
    environment = os.environ.copy()
    environment.update(
        {
            "WPT_OFFLINE": "1",
            "WPT_NETWORK": "disabled",
            "WPT_MANIFEST_REVISION": manifest["wpt_revision"],
            "WPT_TEST_ROOT": str(root),
        }
    )

    records: list[dict[str, Any]] = []
    counts = {key: 0 for key in COUNT_KEYS}
    for test in tests:
        fixture = fixture_path(root, test["path"], test["id"], test["fixture_sha256"])
        actual, error = invoke_adapter(
            adapter_command, test, fixture, root, timeout_seconds, environment
        )
        outcome = classify(test["expected"], actual)
        counts[outcome] += 1
        record: dict[str, Any] = {
            "id": test["id"],
            "path": test["path"],
            "fixture_sha256": test["fixture_sha256"],
            "expected": test["expected"],
            "actual": actual,
            "outcome": outcome,
        }
        if error is not None:
            record["error"] = error
        records.append(record)

    evidence: dict[str, Any] = {
        "schema_version": 1,
        "status": "pass" if not any(counts[key] for key in FAIL_OUTCOMES) else "NO_GO",
        "repository": repository,
        "commit_sha": commit_sha,
        "tree_sha": tree_sha,
        "engine_revision": engine_revision,
        "os_and_arch": os_and_arch,
        "wpt_revision": manifest["wpt_revision"],
        "manifest_sha256": sha256_bytes(manifest_bytes),
        "counts": counts,
        "tests": records,
    }
    evidence["result_digest"] = sha256_bytes(canonical_json(evidence))
    try:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_bytes(canonical_json(evidence) + b"\n")
    except OSError as error:
        raise RunnerError(f"result artifact cannot be written: {error}") from error
    return evidence


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--adapter-command", required=True)
    parser.add_argument("--adapter-arg", action="append", default=[])
    parser.add_argument("--repository", required=True)
    parser.add_argument("--commit-sha", required=True)
    parser.add_argument("--tree-sha", required=True)
    parser.add_argument("--engine-revision", required=True)
    parser.add_argument("--os-and-arch", required=True)
    parser.add_argument("--timeout-seconds", type=float, default=30.0)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    try:
        evidence = run_manifest(
            manifest_path=args.manifest,
            output_path=args.output,
            adapter_command=[args.adapter_command, *args.adapter_arg],
            repository=args.repository,
            commit_sha=args.commit_sha,
            tree_sha=args.tree_sha,
            engine_revision=args.engine_revision,
            os_and_arch=args.os_and_arch,
            timeout_seconds=args.timeout_seconds,
        )
    except (OSError, RunnerError) as error:
        print(f"WPT runner rejected input: {error}", file=sys.stderr)
        return 1
    print(
        json.dumps(
            {
                "status": evidence["status"],
                "result_digest": evidence["result_digest"],
                "counts": evidence["counts"],
            },
            sort_keys=True,
        )
    )
    return 0 if evidence["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
