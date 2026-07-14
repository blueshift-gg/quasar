#!/usr/bin/env python3
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "check-release-dependencies.py"
INSTALLER = ROOT / "scripts" / "install-solana-tools.sh"


class ReleaseDependencyPolicyTests(unittest.TestCase):
    def fixture(self, directory: str) -> Path:
        root = Path(directory)
        shutil.copytree(ROOT / ".github", root / ".github")
        (root / "scripts").mkdir()
        shutil.copy2(ROOT / "scripts" / "install-solana-tools.sh", root / "scripts")
        return root

    def run_policy(self, root: Path) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["python3", str(SCRIPT), "--root", str(root)],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

    def test_current_release_dependency_closure_passes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            result = self.run_policy(self.fixture(directory))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("release dependency closure is immutable", result.stdout)

    def test_mutable_action_reports_exact_reference(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            workflow = root / ".github" / "workflows" / "release.yml"
            text = workflow.read_text()
            text = text.replace(
                "actions/checkout@df4cb1c069e1874edd31b4311f1884172cec0e10",
                "actions/checkout@v6",
                1,
            )
            workflow.write_text(text)
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("mutable action reference: actions/checkout@v6", result.stderr)
        self.assertIn(".github/workflows/release.yml:", result.stderr)

    def test_mutable_action_in_called_workflow_is_in_closure(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            workflow = root / ".github" / "workflows" / "ci.yml"
            text = workflow.read_text().replace(
                "taiki-e/install-action@43aecc8d72668fbcfe75c31400bc4f890f1c5853",
                "taiki-e/install-action@v2",
                1,
            )
            workflow.write_text(text)
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("mutable action reference: taiki-e/install-action@v2", result.stderr)
        self.assertIn(".github/workflows/ci.yml:", result.stderr)

    def test_mutable_container_reports_exact_image(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            dockerfile = root / ".github" / "docker" / "release-cli-smoke.Dockerfile"
            text = dockerfile.read_text()
            text = text.replace(
                "rust:1.92.0-slim-trixie@sha256:bf3368a992915f128293ac76917ab6e561e4dda883273c8f5c9f6f8ea37a378e",
                "rust:1.92.0-slim-trixie",
            )
            dockerfile.write_text(text)
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("mutable container image: rust:1.92.0-slim-trixie", result.stderr)

    def test_mutable_workflow_container_reports_exact_image(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            workflow = root / ".github" / "workflows" / "release.yml"
            with workflow.open("a") as file:
                file.write(
                    "\n  mutable_container:\n"
                    "    runs-on: ubuntu-24.04\n"
                    "    container: ubuntu:latest\n"
                    "    steps:\n"
                    "      - run: true\n"
                )
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "mutable workflow container image: ubuntu:latest", result.stderr
        )

    def test_mutable_debian_snapshot_reports_exact_value(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            dockerfile = root / ".github" / "docker" / "release-cli-smoke.Dockerfile"
            dockerfile.write_text(
                dockerfile.read_text().replace(
                    "ARG DEBIAN_SNAPSHOT=20260113T000000Z",
                    "ARG DEBIAN_SNAPSHOT=latest",
                )
            )
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("mutable Debian snapshot: latest", result.stderr)

    def test_solana_artifact_must_be_hash_verified(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            installer = root / "scripts" / "install-solana-tools.sh"
            installer.write_text(
                installer.read_text().replace("sha256sum -c", "sha256sum")
            )
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Solana artifact is not SHA-256 verified", result.stderr)

    def test_z3_package_must_use_exact_version(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            workflow = root / ".github" / "workflows" / "release.yml"
            workflow.write_text(
                workflow.read_text().replace(
                    '"z3=${{ env.Z3_VERSION }}"',
                    "z3",
                    1,
                )
            )
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("mutable z3 package reference", result.stderr)

    def test_external_checkout_must_use_commit_revision(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            workflow = root / ".github" / "workflows" / "ci.yml"
            workflow.write_text(
                workflow.read_text().replace("ref: ${{ env.CARAVEL_REV }}", "ref: master")
            )
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "mutable checkout reference for joeymeere/caravel: master", result.stderr
        )

    def test_container_cargo_install_must_use_workspace_lock(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            dockerfile = root / ".github" / "docker" / "release-cli-smoke.Dockerfile"
            dockerfile.write_text(dockerfile.read_text().replace(" --locked \\\n", " \\\n"))
            result = self.run_policy(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unlocked local cargo install", result.stderr)

    def test_cached_solana_archive_is_reverified(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            cache = home / ".cache" / "quasar" / "solana"
            cache.mkdir(parents=True)
            archive = cache / "v4.1.1-x86_64-unknown-linux-gnu.tar.bz2"
            archive.write_bytes(b"poisoned cache")
            result = subprocess.run(
                [
                    str(INSTALLER),
                    "v4.1.1",
                    "a5c8e74b8ffa9ce906872b812849057c7fb21cf036ba08f219eb335e20fa4fb3",
                    str(home / "install"),
                ],
                cwd=ROOT,
                env={**os.environ, "HOME": str(home)},
                check=False,
                capture_output=True,
                text=True,
            )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("FAILED", result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
