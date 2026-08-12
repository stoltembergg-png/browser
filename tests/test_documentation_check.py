import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


def run_checker(workspace: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(workspace / "scripts" / "documentation_check.py")],
        cwd=workspace,
        text=True,
        capture_output=True,
        check=False,
    )


class DocumentationCheckerTests(unittest.TestCase):
  def _workspace(self) -> tuple[tempfile.TemporaryDirectory[str], Path]:
    temporary = tempfile.TemporaryDirectory()
    root = Path(temporary.name)
    (root / "scripts").mkdir()
    source = Path(__file__).parents[1] / "scripts" / "documentation_check.py"
    (root / "scripts" / source.name).write_text(source.read_text())
    (root / "docs").mkdir()
    return temporary, root

  def test_rejects_missing_required_document(self) -> None:
    temporary, root = self._workspace()
    self.addCleanup(temporary.cleanup)
    (root / "docs" / "document-authority.yaml").write_text(
        "present:\n  - README.md\n"
    )
    result = run_checker(root)
    self.assertNotEqual(result.returncode, 0)
    self.assertIn("README.md", result.stderr)


  def test_accepts_present_required_documents(self) -> None:
    temporary, root = self._workspace()
    self.addCleanup(temporary.cleanup)
    (root / "docs" / "document-authority.yaml").write_text(
        "present:\n  - README.md\n  - docs/document-authority.yaml\n"
    )
    (root / "README.md").write_text("# test\n")
    result = run_checker(root)
    self.assertEqual(result.returncode, 0, result.stderr)
    self.assertEqual(json.loads(result.stdout)["status"], "pass")


if __name__ == "__main__":
  unittest.main()
