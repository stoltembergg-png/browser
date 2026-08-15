"""Workflow contract tests for the PR-029 reference-platform evidence path."""

from __future__ import annotations

import unittest
from pathlib import Path


WORKSPACE = Path(__file__).parents[1]
WORKFLOW = WORKSPACE / ".github" / "workflows" / "ci-quality-gate.yml"
SERVO_CONTRACT = WORKSPACE / "crates" / "servo-engine" / "tests" / "real_servo_contract.rs"


class ReferencePlatformSmokeWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.workflow = WORKFLOW.read_text(encoding="utf-8")
        cls.servo_contract = SERVO_CONTRACT.read_text(encoding="utf-8")

    def test_pr029_branch_runs_real_reference_contract(self) -> None:
        """PR-029 must select a real Servo evidence job, not the fake smoke."""
        self.assertIn("name: CI / Quality Gate", self.workflow)
        self.assertIn(
            "startsWith(github.head_ref, 'feat/pr-029-reference-platform-smoke')",
            self.workflow,
        )
        self.assertIn("name: PR-029 reference-platform smoke", self.workflow)
        self.assertIn("--test real_servo_contract", self.workflow)

    def test_reference_job_is_required_by_aggregate(self) -> None:
        """The aggregate must fail when the selected reference job is skipped."""
        self.assertIn("needs: [quality, os-matrix, servo-real, reference-smoke]", self.workflow)
        self.assertIn("needs.reference-smoke.result", self.workflow)
        self.assertIn("PR-029 reference-platform smoke failed or was skipped", self.workflow)

    def test_reference_artifact_uses_neutral_identity_names(self) -> None:
        """The artifact contract must not mislabel PR-029 evidence as PR-026."""
        for name in (
            "SERVO_EVIDENCE_ARTIFACT_PATH",
            "SERVO_EVIDENCE_REPOSITORY",
            "SERVO_EVIDENCE_COMMIT_SHA",
            "SERVO_EVIDENCE_TREE_SHA",
            "SERVO_EVIDENCE_OS_ARCH",
        ):
            self.assertIn(name, self.workflow)
            self.assertIn(name, self.servo_contract)
        self.assertNotIn("PR026_ARTIFACT_PATH", self.servo_contract)


if __name__ == "__main__":
    unittest.main()
