#!/usr/bin/env python3
"""Static contract tests for the bundled release-candidate workflow."""

from __future__ import annotations

from pathlib import Path
import unittest


WORKFLOW = Path(__file__).parents[1] / "assets" / "github-release-candidate.yml"


class ReleaseCandidateWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.text = WORKFLOW.read_text(encoding="utf-8")

    def test_dispatch_binds_release_context_digest(self) -> None:
        self.assertIn("release_context_sha256:", self.text)
        self.assertIn("RELEASE_CONTEXT_SHA256: ${{ inputs.release_context_sha256 }}", self.text)
        self.assertIn('"releaseContextSha256": snapshot["releaseContextSha256"]', self.text)
        self.assertIn('"releaseTag": snapshot["expectedTag"]', self.text)

    def test_context_is_captured_before_tests_and_reverified_before_manifest(self) -> None:
        capture = self.text.index("verify_release_context.py capture")
        tests = self.text.index("cargo test --workspace --all-targets --all-features --locked")
        verify = self.text.index("verify_release_context.py verify")
        manifest = self.text.index('manifest = {')
        self.assertLess(capture, tests)
        self.assertLess(tests, verify)
        self.assertLess(verify, manifest)

    def test_checkout_uses_default_branch_full_history_and_no_persisted_credentials(self) -> None:
        self.assertIn("ref: ${{ github.event.repository.default_branch }}", self.text)
        self.assertIn("fetch-depth: 0", self.text)
        self.assertIn("persist-credentials: false", self.text)

    def test_matrix_matches_configured_windows_and_macos_targets(self) -> None:
        self.assertIn("os: [macos-latest, windows-latest]", self.text)
        self.assertNotIn("ubuntu-latest", self.text)

    def test_project_and_cli_binary_names_are_distinct_and_exact(self) -> None:
        self.assertIn("PRODUCT_NAME: bifang_content_production_assistant", self.text)
        self.assertIn("BINARY_NAME: bifang_content_production_assistant_cli", self.text)
        self.assertIn('target["name"] == os.environ["BINARY_NAME"]', self.text)

    def test_obsolete_branch_envelope_contract_is_absent(self) -> None:
        for obsolete in (
            "branch_chain_state_sha256",
            "BRANCH_CHAIN_STATE_SHA256",
            "verify_release_envelope.py",
            "branchChainStateSha256",
            "GitHub 动态默认分支必须是 main 或 master",
        ):
            self.assertNotIn(obsolete, self.text)


if __name__ == "__main__":
    unittest.main()
