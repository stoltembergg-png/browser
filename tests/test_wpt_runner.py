from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).parents[1]
RUNNER = ROOT / "scripts" / "wpt_runner.py"
WPT_REVISION = "40f78009c81558c6ff89915cb2546c2fe3ef3b97"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class WptRunnerTests(unittest.TestCase):
    def test_checked_in_manifest_pins_local_fixture_identity(self) -> None:
        manifest_path = ROOT / "tests" / "fixtures" / "wpt" / "manifest.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

        self.assertEqual(manifest["wpt_revision"], WPT_REVISION)
        for test in manifest["tests"]:
            fixture = manifest_path.parent / test["path"]
            self.assertEqual(test["fixture_sha256"], sha256(fixture))

    def test_runs_offline_manifest_and_binds_identity(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "navigation").mkdir()
            (root / "navigation" / "local-url.html").write_text(
                "<!doctype html><title>local URL</title>", encoding="utf-8"
            )
            (root / "csp").mkdir()
            (root / "csp" / "inline-script.html").write_text(
                "<!doctype html><title>inline script</title>", encoding="utf-8"
            )
            manifest = {
                "schema_version": 1,
                "wpt_revision": WPT_REVISION,
                "adapter_protocol": 1,
                "tests": [
                    {
                        "id": "navigation/local-url",
                        "path": "navigation/local-url.html",
                        "fixture_sha256": sha256(root / "navigation" / "local-url.html"),
                        "expected": "pass",
                    },
                    {
                        "id": "csp/inline-script",
                        "path": "csp/inline-script.html",
                        "fixture_sha256": sha256(root / "csp" / "inline-script.html"),
                        "expected": "fail",
                        "owner": "browser-team",
                        "reason": "The current engine does not execute this CSP fixture.",
                        "recheck_after": "2027-01-01",
                    },
                ],
            }
            manifest_path = root / "manifest.json"
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            adapter = root / "adapter.py"
            adapter.write_text(
                "import argparse, json\n"
                "parser = argparse.ArgumentParser()\n"
                "parser.add_argument('--test-id', required=True)\n"
                "parser.add_argument('--fixture', required=True)\n"
                "args = parser.parse_args()\n"
                "status = 'fail' if args.test_id == 'csp/inline-script' else 'pass'\n"
                "print(json.dumps({'test_id': args.test_id, 'status': status}))\n",
                encoding="utf-8",
            )
            output_path = root / "result.json"
            command = [
                sys.executable,
                str(RUNNER),
                "--manifest",
                str(manifest_path),
                "--output",
                str(output_path),
                "--adapter-command",
                sys.executable,
                "--adapter-arg",
                str(adapter),
                "--repository",
                "stoltembergg-png/browser",
                "--commit-sha",
                "a" * 40,
                "--tree-sha",
                "b" * 40,
                "--engine-revision",
                "servo-859bd5e",
                "--os-and-arch",
                "ubuntu-24.04-x86_64",
            ]
            result = subprocess.run(
                command,
                cwd=ROOT,
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            evidence = json.loads(output_path.read_text(encoding="utf-8"))
            self.assertEqual(evidence["status"], "pass")
            self.assertEqual(evidence["wpt_revision"], WPT_REVISION)
            self.assertEqual(evidence["commit_sha"], "a" * 40)
            self.assertEqual(evidence["tree_sha"], "b" * 40)
            self.assertEqual(evidence["engine_revision"], "servo-859bd5e")
            self.assertEqual(evidence["counts"]["pass"], 1)
            self.assertEqual(evidence["counts"]["expected-fail"], 1)
            self.assertEqual(evidence["counts"]["fail"], 0)
            self.assertEqual(len(evidence["result_digest"]), 64)

    def test_rejects_adapter_response_with_uncontracted_fields(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "fixture.html").write_text("<!doctype html>", encoding="utf-8")
            (root / "manifest.json").write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "wpt_revision": WPT_REVISION,
                        "adapter_protocol": 1,
                        "tests": [
                            {
                                "id": "contract/extra-field",
                                "path": "fixture.html",
                                "fixture_sha256": sha256(root / "fixture.html"),
                                "expected": "pass",
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )
            adapter = root / "adapter.py"
            adapter.write_text(
                "import argparse, json\n"
                "parser = argparse.ArgumentParser()\n"
                "parser.add_argument('--test-id', required=True)\n"
                "parser.add_argument('--fixture', required=True)\n"
                "args = parser.parse_args()\n"
                "print(json.dumps({'test_id': args.test_id, 'status': 'pass', 'extra': 'reject'}))\n",
                encoding="utf-8",
            )
            output_path = root / "result.json"
            result = subprocess.run(
                [
                    sys.executable,
                    str(RUNNER),
                    "--manifest",
                    str(root / "manifest.json"),
                    "--output",
                    str(output_path),
                    "--adapter-command",
                    sys.executable,
                    "--adapter-arg",
                    str(adapter),
                    "--repository",
                    "stoltembergg-png/browser",
                    "--commit-sha",
                    "a" * 40,
                    "--tree-sha",
                    "b" * 40,
                    "--engine-revision",
                    "servo-859bd5e",
                    "--os-and-arch",
                    "ubuntu-24.04-x86_64",
                ],
                cwd=ROOT,
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("NO_GO", result.stdout)
            evidence = json.loads(output_path.read_text(encoding="utf-8"))
            self.assertEqual(evidence["tests"][0]["outcome"], "error")
            self.assertIn("uncontracted", evidence["tests"][0]["error"])

    def test_rejects_fixture_identity_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "fixture.html").write_text("<!doctype html>", encoding="utf-8")
            (root / "manifest.json").write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "wpt_revision": WPT_REVISION,
                        "adapter_protocol": 1,
                        "tests": [
                            {
                                "id": "contract/mismatch",
                                "path": "fixture.html",
                                "fixture_sha256": "0" * 64,
                                "expected": "pass",
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )
            adapter = root / "adapter.py"
            adapter.write_text(
                "import argparse, json\n"
                "parser = argparse.ArgumentParser()\n"
                "parser.add_argument('--test-id', required=True)\n"
                "parser.add_argument('--fixture', required=True)\n"
                "args = parser.parse_args()\n"
                "print(json.dumps({'test_id': args.test_id, 'status': 'pass'}))\n",
                encoding="utf-8",
            )
            result = subprocess.run(
                [
                    sys.executable,
                    str(RUNNER),
                    "--manifest",
                    str(root / "manifest.json"),
                    "--output",
                    str(root / "result.json"),
                    "--adapter-command",
                    sys.executable,
                    "--adapter-arg",
                    str(adapter),
                    "--repository",
                    "stoltembergg-png/browser",
                    "--commit-sha",
                    "a" * 40,
                    "--tree-sha",
                    "b" * 40,
                    "--engine-revision",
                    "servo-859bd5e",
                    "--os-and-arch",
                    "ubuntu-24.04-x86_64",
                ],
                cwd=ROOT,
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("fixture identity", result.stderr)


if __name__ == "__main__":
    unittest.main()
