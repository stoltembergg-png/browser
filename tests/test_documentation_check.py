import json
import subprocess
import sys
from pathlib import Path


def run_checker(workspace: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(workspace / "scripts" / "documentation_check.py")],
        cwd=workspace,
        text=True,
        capture_output=True,
        check=False,
    )


def test_documentation_checker_rejects_missing_required_document(tmp_path: Path) -> None:
    (tmp_path / "scripts").mkdir()
    source = Path(__file__).parents[1] / "scripts" / "documentation_check.py"
    (tmp_path / "scripts" / source.name).write_text(source.read_text())
    (tmp_path / "docs").mkdir()
    (tmp_path / "docs" / "document-authority.yaml").write_text(
        "present:\n  - README.md\n"
    )
    result = run_checker(tmp_path)
    assert result.returncode != 0
    assert "README.md" in result.stderr


def test_documentation_checker_accepts_present_required_documents(tmp_path: Path) -> None:
    (tmp_path / "scripts").mkdir()
    source = Path(__file__).parents[1] / "scripts" / "documentation_check.py"
    (tmp_path / "scripts" / source.name).write_text(source.read_text())
    (tmp_path / "docs").mkdir()
    (tmp_path / "docs" / "document-authority.yaml").write_text(
        "present:\n  - README.md\n  - docs/document-authority.yaml\n"
    )
    (tmp_path / "README.md").write_text("# test\n")
    result = run_checker(tmp_path)
    assert result.returncode == 0, result.stderr
    assert json.loads(result.stdout)["status"] == "pass"
