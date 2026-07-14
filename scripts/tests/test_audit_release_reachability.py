#!/usr/bin/env python3
import json
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "audit-release-reachability.py"


def package(package_id: str, name: str, version: str, publish=None) -> dict:
    return {
        "id": package_id,
        "name": name,
        "version": version,
        "source": None if package_id.startswith("path+") else "registry",
        "publish": publish,
    }


def metadata(runtime_advisory: bool = False) -> dict:
    packages = [
        package("path+runtime", "runtime", "0.1.0"),
        package("path+tests", "tests", "0.1.0", []),
        package("registry+bridge", "bridge", "1.0.0"),
        package("registry+advised", "advised", "2.0.0"),
    ]
    runtime_deps = [
        {
            "pkg": "registry+advised" if runtime_advisory else "registry+bridge",
            "dep_kinds": [{"kind": None, "target": None}],
        }
    ]
    return {
        "packages": packages,
        "workspace_members": ["path+runtime", "path+tests"],
        "resolve": {
            "nodes": [
                {"id": "path+runtime", "deps": runtime_deps},
                {
                    "id": "path+tests",
                    "deps": [
                        {
                            "pkg": "registry+advised",
                            "dep_kinds": [{"kind": "dev", "target": None}],
                        }
                    ],
                },
                {"id": "registry+bridge", "deps": []},
                {"id": "registry+advised", "deps": []},
            ]
        },
    }


def audit() -> dict:
    finding = {
        "package": {
            "name": "advised",
            "version": "2.0.0",
            "source": "registry",
        },
        "advisory": {"id": "RUSTSEC-2099-0001"},
    }
    return {
        "vulnerabilities": {"list": [], "count": 0, "found": False},
        "warnings": {"unsound": [finding]},
    }


def runtime_inventory(runtime_advisory: bool = False) -> dict:
    packages = [
        {"name": "runtime", "version": "0.1.0", "roots": ["runtime"]},
        {"name": "bridge", "version": "1.0.0", "roots": ["runtime"]},
    ]
    if runtime_advisory:
        packages.append({"name": "advised", "version": "2.0.0", "roots": ["runtime"]})
    return {"packages": packages}


def policy(review_by: str = "2026-10-13") -> dict:
    return {
        "schema": 1,
        "exceptions": [
            {
                "id": "RUSTSEC-2099-0001",
                "package": "advised",
                "version": "2.0.0",
                "reachability": "dev/test-only",
                "dependency_path": [
                    {"package": "tests", "version": "0.1.0"},
                    {"package": "advised", "version": "2.0.0"},
                ],
                "owner": "release maintainers",
                "reason": "fixture advisory is confined to tests",
                "reviewed_on": "2026-07-13",
                "review_by": review_by,
            }
        ],
    }


class AuditReachabilityTests(unittest.TestCase):
    def run_policy(
        self,
        metadata_report: dict,
        audit_report: dict,
        policy_report: dict,
        runtime_report: dict | None = None,
    ) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as directory:
            directory_path = Path(directory)
            paths = {
                "metadata": directory_path / "metadata.json",
                "audit": directory_path / "audit.json",
                "policy": directory_path / "policy.json",
                "runtime": directory_path / "runtime.json",
            }
            for name, report in (
                ("metadata", metadata_report),
                ("audit", audit_report),
                ("policy", policy_report),
                ("runtime", runtime_report or runtime_inventory()),
            ):
                paths[name].write_text(json.dumps(report))
            return subprocess.run(
                [
                    "python3",
                    str(SCRIPT),
                    "--metadata-json",
                    str(paths["metadata"]),
                    "--audit-json",
                    str(paths["audit"]),
                    "--policy",
                    str(paths["policy"]),
                    "--runtime-json",
                    str(paths["runtime"]),
                    "--today",
                    "2026-07-13",
                ],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )

    def test_reviewed_dev_only_advisory_passes(self) -> None:
        result = self.run_policy(metadata(), audit(), policy())
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("ACCEPTED dev/test-only", result.stdout)
        self.assertIn("tests@0.1.0 -> advised@2.0.0", result.stdout)

    def test_runtime_reachable_advisory_fails_even_with_exception(self) -> None:
        result = self.run_policy(
            metadata(runtime_advisory=True),
            audit(),
            policy(),
            runtime_inventory(runtime_advisory=True),
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("BLOCKED runtime-reachable", result.stdout)
        self.assertIn("runtime-reachable advisory", result.stderr)

    def test_unreviewed_dev_only_advisory_fails(self) -> None:
        result = self.run_policy(metadata(), audit(), {"schema": 1, "exceptions": []})
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing dev/test-only exception", result.stderr)

    def test_expired_exception_fails(self) -> None:
        result = self.run_policy(metadata(), audit(), policy(review_by="2026-07-12"))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("exception expired on 2026-07-12", result.stderr)

    def test_stale_exception_fails(self) -> None:
        empty_audit = {
            "vulnerabilities": {"list": [], "count": 0, "found": False},
            "warnings": {},
        }
        result = self.run_policy(metadata(), empty_audit, policy())
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("stale audit exception", result.stderr)

    def test_invalid_dependency_path_fails(self) -> None:
        invalid_policy = policy()
        invalid_policy["exceptions"][0]["dependency_path"].insert(
            1, {"package": "bridge", "version": "1.0.0"}
        )
        result = self.run_policy(metadata(), audit(), invalid_policy)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("dependency_path edge does not exist", result.stderr)


if __name__ == "__main__":
    unittest.main()
