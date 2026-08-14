import json
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[1]
SCRIPT = ROOT / "scripts" / "threat_regression_gate.py"
MANIFEST = ROOT / "docs" / "security" / "threat-regression-manifest.json"


class ThreatRegressionGateTests(unittest.TestCase):
    def run_gate(self, *arguments: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--manifest", str(MANIFEST), *arguments],
            cwd=ROOT,
            text=True,
            capture_output=True,
        )

    def test_validation_covers_all_threat_model_scenarios(self) -> None:
        result = self.run_gate("--validate-only")
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(report["status"], "pass")
        self.assertEqual(report["scenario_count"], 18)
        self.assertEqual(report["scenario_ids"], [f"TM-{index:03d}" for index in range(1, 19)])

    def test_rejects_missing_threat_scenario(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "manifest.json"
            data = json.loads(MANIFEST.read_text())
            data["scenarios"] = data["scenarios"][:-1]
            path.write_text(json.dumps(data))
            result = self.run_gate("--manifest", str(path), "--validate-only")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("scenario ids", result.stderr)

    def test_alpha_gate_is_no_go_until_unproven_scenarios_are_implemented(self) -> None:
        result = self.run_gate("--channel", "alpha")
        self.assertNotEqual(result.returncode, 0)
        report = json.loads(result.stdout)
        self.assertEqual(report["status"], "NO_GO")
        self.assertIn("TM-001", report["blockers"])
        self.assertIn("TM-008", report["blockers"])


if __name__ == "__main__":
    unittest.main()
