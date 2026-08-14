import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).parents[1]
SCRIPT = ROOT / "scripts" / "servo_evidence_check.py"


def valid_artifact() -> dict:
    return {
        "status": "pass",
        "repository": "stoltembergg-png/browser",
        "commit_sha": "a" * 40,
        "tree_sha": "b" * 40,
        "servo_revision": "859bd5edd60c0fb162a1f73c083a23e55474faf7",
        "engine_revision": "859bd5edd60c0fb162a1f73c083a23e55474faf7",
        "os_and_arch": "ubuntu-24.04-x86_64",
        "surface_strategy": "software-rendering-context",
        "thread_affinity": "engine-thread-only",
        "artifact_digest": "c" * 64,
        "frame_digest": "d" * 64,
        "load_complete": True,
        "screenshot_ready": True,
        "frame_count": 3,
        "cases": {
            "http_fixture_load": "pass",
            "frame_ready_paint_present": "pass",
            "text_input": "pass",
            "click_link": "pass",
            "resize": "pass",
            "thread_affinity": "pass",
            "shutdown_no_deadlock": "pass",
        },
    }


class ServoEvidenceCheckTests(unittest.TestCase):
    def run_check(self, artifact: dict) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "evidence.json"
            path.write_text(json.dumps(artifact))
            return subprocess.run(
                [sys.executable, str(SCRIPT), str(path)],
                cwd=ROOT,
                text=True,
                capture_output=True,
                check=False,
            )

    def test_accepts_complete_real_evidence(self) -> None:
        result = self.run_check(valid_artifact())
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_no_frame_or_skipped_case(self) -> None:
        artifact = valid_artifact()
        artifact["frame_count"] = 0
        artifact["cases"]["click_link"] = "skip"
        result = self.run_check(artifact)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("frame_count", result.stderr)
        self.assertIn("click_link", result.stderr)

    def test_rejects_wrong_servo_revision(self) -> None:
        artifact = valid_artifact()
        artifact["servo_revision"] = "d" * 40
        result = self.run_check(artifact)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("servo_revision", result.stderr)


if __name__ == "__main__":
    unittest.main()
