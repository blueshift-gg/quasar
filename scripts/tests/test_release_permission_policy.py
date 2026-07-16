#!/usr/bin/env python3
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "check-release-permissions.py"


class ReleasePermissionPolicyTests(unittest.TestCase):
    def fixture(self, directory: str) -> Path:
        root = Path(directory)
        workflow_directory = root / ".github" / "workflows"
        workflow_directory.mkdir(parents=True)
        shutil.copy2(ROOT / ".github" / "workflows" / "release.yml", workflow_directory)
        return root

    def run_policy(self, root: Path) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["python3", str(SCRIPT), "--root", str(root)],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

    def mutate(self, root: Path, old: str, new: str, count: int = 1) -> None:
        workflow = root / ".github" / "workflows" / "release.yml"
        text = workflow.read_text()
        self.assertGreaterEqual(text.count(old), count)
        workflow.write_text(text.replace(old, new, count))

    def test_current_release_permissions_pass(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            result = self.run_policy(self.fixture(directory))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("default and verification jobs: contents: read", result.stdout)
        self.assertIn("github-release: contents: write", result.stdout)

    def test_workflow_default_must_be_read_only(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            self.mutate(root, "permissions:\n  contents: read", "permissions:\n  contents: write")
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("release permissions must default to contents: read", result.stderr)

    def test_unused_global_write_permission_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            self.mutate(
                root,
                "permissions:\n  contents: read",
                "permissions:\n  contents: read\n  issues: write",
            )
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("'issues': 'write'", result.stderr)

    def test_verification_job_cannot_gain_write_permission(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            self.mutate(
                root,
                "  kani:\n    name: Kani",
                "  kani:\n    permissions:\n      checks: write\n    name: Kani",
            )
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("job kani has unauthorized write permissions", result.stderr)

    def test_verification_job_cannot_gain_write_all(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            self.mutate(
                root,
                "  kani:\n    name: Kani",
                "  kani:\n    permissions: write-all\n    name: Kani",
            )
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("job kani has unauthorized write permissions", result.stderr)
        self.assertIn("'<all>': 'write-all'", result.stderr)

    def test_github_release_has_only_contents_write(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            self.mutate(
                root,
                "      contents: write\n    steps:",
                "      contents: write\n      issues: write\n    steps:",
            )
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("github-release must grant only contents: write", result.stderr)

    def test_publisher_environment_is_confined_to_publish_job(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            self.mutate(
                root,
                "  kani:\n    name: Kani",
                "  kani:\n    environment: crates-io\n    name: Kani",
            )
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("publisher environment is attached to non-publish job kani", result.stderr)

    def test_publisher_environment_mapping_is_confined_to_publish_job(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            self.mutate(
                root,
                "  kani:\n    name: Kani",
                "  kani:\n    environment:\n      name: crates-io\n    name: Kani",
            )
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("publisher environment is attached to non-publish job kani", result.stderr)

    def test_crates_token_is_confined_to_publish_job(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            self.mutate(
                root,
                "  kani:\n    name: Kani",
                "  kani:\n    env:\n"
                "      LEAKED_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}\n"
                "    name: Kani",
            )
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("crates.io token is available to non-publish job kani", result.stderr)

    def test_npm_token_is_confined_to_typescript_publish_job(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            self.mutate(
                root,
                "  kani:\n    name: Kani",
                "  kani:\n    env:\n"
                "      LEAKED_TOKEN: ${{ secrets.NPM_TOKEN }}\n"
                "    name: Kani",
            )
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("npm token is available to non-TypeScript-publish job kani", result.stderr)

    def test_github_token_is_confined_to_release_job(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            self.mutate(
                root,
                "  kani:\n    name: Kani",
                "  kani:\n    env:\n"
                "      LEAKED_TOKEN: ${{ secrets.GITHUB_TOKEN }}\n"
                "    name: Kani",
            )
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("GITHUB_TOKEN is explicitly exposed to non-release job kani", result.stderr)

    def test_bracket_secret_reference_is_audited(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            self.mutate(
                root,
                "  kani:\n    name: Kani",
                "  kani:\n    env:\n"
                "      LEAKED_TOKEN: ${{ secrets['RELEASE_PAT'] }}\n"
                "    name: Kani",
            )
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unauthorized secret RELEASE_PAT in job kani", result.stderr)

    def test_reusable_workflow_cannot_inherit_all_secrets(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            self.mutate(
                root,
                "    uses: ./.github/workflows/ci.yml",
                "    uses: ./.github/workflows/ci.yml\n    secrets: inherit",
            )
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("job ci inherits all secrets", result.stderr)


if __name__ == "__main__":
    unittest.main()
