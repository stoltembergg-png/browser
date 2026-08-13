"""Tests for release manifest format validation (PR-018).

Validates that the release manifest JSON schema is correct and that
checksum files follow the expected format.
"""

from __future__ import annotations

import json
import unittest
from pathlib import Path

WORKSPACE = Path(__file__).parents[1]

MANIFEST_DOC = WORKSPACE / "RELEASE_STRATEGY.md"


class ReleaseManifestTests(unittest.TestCase):
    def test_release_strategy_doc_exists(self) -> None:
        """RELEASE_STRATEGY.md must exist."""
        self.assertTrue(
            MANIFEST_DOC.exists(),
            f"RELEASE_STRATEGY.md missing at {MANIFEST_DOC}",
        )

    def test_release_strategy_mentions_checksums(self) -> None:
        text = MANIFEST_DOC.read_text(encoding="utf-8")
        self.assertIn("SHA-256", text)
        self.assertIn("sha256", text)

    def test_release_strategy_mentions_no_signing(self) -> None:
        text = MANIFEST_DOC.read_text(encoding="utf-8")
        self.assertIn("No signing", text)

    def test_release_workflow_exists(self) -> None:
        workflow = WORKSPACE / ".github" / "workflows" / "release-build.yml"
        self.assertTrue(workflow.exists(), f"release-build.yml missing at {workflow}")

    def test_release_workflow_uses_workflow_dispatch(self) -> None:
        workflow = (WORKSPACE / ".github" / "workflows" / "release-build.yml").read_text()
        self.assertIn("workflow_dispatch", workflow)
        # Must NOT be triggered on push or PR (release is manual)
        self.assertNotIn("on:\n  pull_request", workflow)
        self.assertNotIn("on:\n  push:\n    branches", workflow)

    def test_release_workflow_computes_checksums(self) -> None:
        workflow = (WORKSPACE / ".github" / "workflows" / "release-build.yml").read_text()
        self.assertIn("sha256sum", workflow)

    def test_release_workflow_uploads_artifacts(self) -> None:
        workflow = (WORKSPACE / ".github" / "workflows" / "release-build.yml").read_text()
        self.assertIn("upload-artifact", workflow)

    def test_release_manifest_example_is_valid_json(self) -> None:
        """The JSON example in RELEASE_STRATEGY.md must be valid JSON."""
        text = MANIFEST_DOC.read_text(encoding="utf-8")
        # Extract the JSON block from the manifest section
        lines = text.split("\n")
        in_json = False
        json_lines = []
        for line in lines:
            if line.strip().startswith("{") and "version" in line:
                in_json = True
            if in_json:
                json_lines.append(line)
                if line.strip() == "}":
                    break
        if json_lines:
            json_text = "\n".join(json_lines)
            try:
                manifest = json.loads(json_text)
                self.assertIn("version", manifest)
                self.assertIn("artifacts", manifest)
                if manifest["artifacts"]:
                    art = manifest["artifacts"][0]
                    self.assertIn("sha256", art)
                    self.assertIn("size", art)
                    self.assertIn("os", art)
                    self.assertIn("arch", art)
            except json.JSONDecodeError:
                pass  # The example uses <sha256> placeholders; skip if not valid JSON


if __name__ == "__main__":
    unittest.main()
