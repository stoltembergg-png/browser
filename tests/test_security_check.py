import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


class SecurityCheckTests(unittest.TestCase):
    def _workspace(self, lock_text: str, extra: str = "") -> tuple[tempfile.TemporaryDirectory, Path]:
        temporary = tempfile.TemporaryDirectory()
        root = Path(temporary.name)
        (root / "scripts").mkdir()
        source = Path(__file__).parents[1] / "scripts" / "security_check.py"
        (root / "scripts" / source.name).write_text(source.read_text())
        (root / "Cargo.lock").write_text(lock_text)
        (root / "README.md").write_text(extra)
        return temporary, root

    def test_rejects_git_source(self) -> None:
        temporary, root = self._workspace("[[package]]\nname = \"demo\"\nsource = \"git+https://example.invalid/demo\"\n")
        self.addCleanup(temporary.cleanup)
        result = subprocess.run([sys.executable, "scripts/security_check.py"], cwd=root, text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("git source", result.stderr)

    def test_rejects_secret_like_text(self) -> None:
        temporary, root = self._workspace("[[package]]\nname = \"demo\"\n", "token = " + "ghp_" + "abcdefghijklmnopqrstuvwxyz1234567890\n")
        self.addCleanup(temporary.cleanup)
        result = subprocess.run([sys.executable, "scripts/security_check.py"], cwd=root, text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("secret-like", result.stderr)

    def test_accepts_minimal_locked_workspace(self) -> None:
        temporary, root = self._workspace("[[package]]\nname = \"demo\"\nversion = \"0.1.0\"\n")
        self.addCleanup(temporary.cleanup)
        result = subprocess.run([sys.executable, "scripts/security_check.py"], cwd=root, text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)["status"], "pass")


if __name__ == "__main__":
    unittest.main()
