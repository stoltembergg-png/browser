import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


class QualityGateManifestTests(unittest.TestCase):
    def run_checker(self, root: Path) -> subprocess.CompletedProcess[str]:
        return subprocess.run([sys.executable, "scripts/quality_gate_check.py"], cwd=root, text=True, capture_output=True)

    def workspace(self) -> tuple[tempfile.TemporaryDirectory, Path]:
        temporary = tempfile.TemporaryDirectory()
        root = Path(temporary.name)
        (root / "scripts").mkdir()
        (root / "docs" / "ci").mkdir(parents=True)
        source = Path(__file__).parents[1] / "scripts" / "quality_gate_check.py"
        (root / "scripts" / source.name).write_text(source.read_text())
        manifest = Path(__file__).parents[1] / "docs" / "ci" / "quality-gate-manifest.json"
        (root / "docs" / "ci" / manifest.name).write_text(manifest.read_text())
        return temporary, root

    def test_accepts_fail_closed_manifest(self) -> None:
        temporary, root = self.workspace()
        self.addCleanup(temporary.cleanup)
        result = self.run_checker(root)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)["status"], "pass")

    def test_rejects_missing_evidence_field(self) -> None:
        temporary, root = self.workspace()
        self.addCleanup(temporary.cleanup)
        path = root / "docs" / "ci" / "quality-gate-manifest.json"
        data = json.loads(path.read_text())
        data["evidence_fields"].remove("head_sha")
        path.write_text(json.dumps(data))
        result = self.run_checker(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("evidence_fields", result.stderr)

    def test_rejects_non_fail_closed_manifest(self) -> None:
        temporary, root = self.workspace()
        self.addCleanup(temporary.cleanup)
        path = root / "docs" / "ci" / "quality-gate-manifest.json"
        data = json.loads(path.read_text())
        data["fail_closed"] = False
        path.write_text(json.dumps(data))
        result = self.run_checker(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("fail_closed", result.stderr)


if __name__ == "__main__":
    unittest.main()
