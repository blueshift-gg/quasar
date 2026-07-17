#!/usr/bin/env python3
"""Classify RustSec advisories by reachability from publishable crates."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from datetime import date
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_POLICY = ROOT / "security" / "audit-exceptions.json"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--audit-json", type=Path)
    parser.add_argument("--metadata-json", type=Path)
    parser.add_argument("--runtime-json", type=Path)
    parser.add_argument("--policy", type=Path, default=DEFAULT_POLICY)
    parser.add_argument("--today", type=date.fromisoformat, default=date.today())
    return parser.parse_args()


def read_json(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read {path}: {error}") from error


def command_json(command: list[str], allow_failure: bool = False) -> dict[str, Any]:
    result = subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode and not allow_failure:
        raise ValueError(
            f"{' '.join(command)} failed with exit {result.returncode}:\n{result.stderr}"
        )
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise ValueError(
            f"{' '.join(command)} did not produce valid JSON: {error}\n{result.stderr}"
        ) from error


def load_reports(args: argparse.Namespace) -> tuple[dict[str, Any], dict[str, Any]]:
    audit = (
        read_json(args.audit_json)
        if args.audit_json
        else command_json(["cargo", "audit", "--json"], allow_failure=True)
    )
    metadata = (
        read_json(args.metadata_json)
        if args.metadata_json
        else command_json(["cargo", "metadata", "--locked", "--format-version", "1"])
    )
    return audit, metadata


def package_key(package: dict[str, Any]) -> tuple[str, str, str | None]:
    return package["name"], package["version"], package.get("source")


def advisory_key(advisory: dict[str, Any]) -> tuple[str, str, str]:
    package = advisory["package"]
    return advisory["advisory"]["id"], package["name"], package["version"]


def collect_advisories(audit: dict[str, Any]) -> list[tuple[str, dict[str, Any]]]:
    findings: list[tuple[str, dict[str, Any]]] = []
    findings.extend(("vulnerability", item) for item in audit["vulnerabilities"]["list"])
    for kind, items in audit.get("warnings", {}).items():
        findings.extend((kind, item) for item in items)
    return sorted(findings, key=lambda finding: advisory_key(finding[1]))


def dependency_graph(metadata: dict[str, Any]) -> dict[str, set[str]]:
    graph: dict[str, set[str]] = {}
    for node in metadata["resolve"]["nodes"]:
        graph[node["id"]] = {dependency["pkg"] for dependency in node["deps"]}
    return graph


def publishable_packages(metadata: dict[str, Any]) -> list[dict[str, Any]]:
    workspace_members = set(metadata["workspace_members"])
    return sorted(
        (
            package
            for package in metadata["packages"]
            if package["id"] in workspace_members
            and (package.get("publish") is None or package["publish"])
        ),
        key=lambda package: package["name"],
    )


def runtime_inventory_from_json(report: dict[str, Any]) -> dict[tuple[str, str], set[str]]:
    inventory: dict[tuple[str, str], set[str]] = {}
    for package in report.get("packages", []):
        inventory[(package["name"], package["version"])] = set(package["roots"])
    return inventory


def runtime_inventory(metadata: dict[str, Any]) -> dict[tuple[str, str], set[str]]:
    inventory: dict[tuple[str, str], set[str]] = {}
    package_pattern = re.compile(r"^([^ ]+) v([^ ]+)(?: |$)")
    for root in publishable_packages(metadata):
        command = [
            "cargo",
            "tree",
            "--locked",
            "--package",
            root["name"],
            "--all-features",
            "--target",
            "all",
            "--edges",
            "normal",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ]
        result = subprocess.run(
            command,
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode:
            raise ValueError(
                f"{' '.join(command)} failed with exit {result.returncode}:\n{result.stderr}"
            )
        for line in result.stdout.splitlines():
            matched = package_pattern.match(line)
            if matched:
                inventory.setdefault((matched.group(1), matched.group(2)), set()).add(
                    root["name"]
                )
    return inventory


def advisory_runtime_roots(
    advisory: dict[str, Any], inventory: dict[tuple[str, str], set[str]]
) -> set[str]:
    package = advisory["package"]
    return inventory.get((package["name"], package["version"]), set())


def package_ids(metadata: dict[str, Any]) -> dict[tuple[str, str, str | None], set[str]]:
    packages: dict[tuple[str, str, str | None], set[str]] = {}
    for package in metadata["packages"]:
        packages.setdefault(package_key(package), set()).add(package["id"])
    return packages


def finding_package_ids(
    finding: dict[str, Any], packages: dict[tuple[str, str, str | None], set[str]]
) -> set[str]:
    package = finding["package"]
    exact = packages.get(package_key(package), set())
    if exact:
        return exact
    return {
        package_id
        for (name, version, _source), ids in packages.items()
        if name == package["name"] and version == package["version"]
        for package_id in ids
    }


def policy_entries(policy: dict[str, Any]) -> dict[tuple[str, str, str], dict[str, Any]]:
    if policy.get("schema") != 1:
        raise ValueError("audit exception policy must use schema 1")
    entries: dict[tuple[str, str, str], dict[str, Any]] = {}
    for entry in policy.get("exceptions", []):
        key = (entry.get("id", ""), entry.get("package", ""), entry.get("version", ""))
        if key in entries:
            raise ValueError(f"duplicate audit exception: {' '.join(key)}")
        entries[key] = entry
    return entries


def dev_test_roots(policy: dict[str, Any], metadata: dict[str, Any]) -> set[str]:
    """Return publishable roots whose public purpose is local test execution.

    These roots may carry normal dependencies that are runtime-reachable inside
    the test process. Findings still need a reviewed, expiring exception, and a
    finding reachable from any production root remains blocked.
    """
    publishable = {
        (package["name"], package["version"])
        for package in publishable_packages(metadata)
    }
    roots: set[str] = set()
    for entry in policy.get("dev_test_roots", []):
        if not isinstance(entry, dict):
            raise ValueError("dev_test_roots entries must be objects")
        name = entry.get("package", "")
        version = entry.get("version", "")
        if not name or not version or not entry.get("reason"):
            raise ValueError(
                "dev_test_roots entries require package, version, and reason"
            )
        if (name, version) not in publishable:
            raise ValueError(
                f"dev/test root is not a publishable workspace package: {name}@{version}"
            )
        if name in roots:
            raise ValueError(f"duplicate dev/test root: {name}@{version}")
        roots.add(name)
    return roots


def validate_path(
    entry: dict[str, Any],
    metadata: dict[str, Any],
    graph: dict[str, set[str]],
    finding_ids: set[str],
) -> str | None:
    raw_path = entry.get("dependency_path")
    if not isinstance(raw_path, list) or len(raw_path) < 2:
        return "dependency_path must contain at least two packages"

    ids_by_name_version: dict[tuple[str, str], set[str]] = {}
    for package in metadata["packages"]:
        ids_by_name_version.setdefault((package["name"], package["version"]), set()).add(
            package["id"]
        )

    path_ids: list[set[str]] = []
    labels: list[str] = []
    for component in raw_path:
        if not isinstance(component, dict):
            return "dependency_path entries must be objects"
        name = component.get("package", "")
        version = component.get("version", "")
        labels.append(f"{name}@{version}")
        candidates = ids_by_name_version.get((name, version), set())
        if not candidates:
            return f"dependency_path package is absent from metadata: {labels[-1]}"
        path_ids.append(candidates)

    for index in range(len(path_ids) - 1):
        if not any(
            child in graph.get(parent, set())
            for parent in path_ids[index]
            for child in path_ids[index + 1]
        ):
            return f"dependency_path edge does not exist: {labels[index]} -> {labels[index + 1]}"
    if not path_ids[-1].intersection(finding_ids):
        return "dependency_path does not end at the advisory package"
    return None


def validate_exception(
    entry: dict[str, Any],
    today: date,
    metadata: dict[str, Any],
    graph: dict[str, set[str]],
    finding_ids: set[str],
    runtime_roots: set[str],
) -> list[str]:
    errors: list[str] = []
    if entry.get("reachability") != "dev/test-only":
        errors.append("reachability must be dev/test-only")
    for field in ("owner", "reason", "reviewed_on", "review_by"):
        if not entry.get(field):
            errors.append(f"missing {field}")
    try:
        reviewed_on = date.fromisoformat(entry.get("reviewed_on", ""))
        review_by = date.fromisoformat(entry.get("review_by", ""))
        if reviewed_on > today:
            errors.append(f"reviewed_on is in the future: {reviewed_on}")
        if review_by < reviewed_on:
            errors.append(f"review_by precedes reviewed_on: {review_by}")
        if review_by < today:
            errors.append(f"exception expired on {review_by}")
    except ValueError:
        errors.append("reviewed_on and review_by must be ISO dates")

    path_error = validate_path(entry, metadata, graph, finding_ids)
    if path_error:
        errors.append(path_error)
    elif runtime_roots:
        path_root = entry["dependency_path"][0]["package"]
        if path_root not in runtime_roots:
            errors.append(
                "dependency_path must start at a reachable dev/test root: "
                f"expected one of {', '.join(sorted(runtime_roots))}, got {path_root}"
            )
    return errors


def main() -> int:
    args = parse_args()
    try:
        audit, metadata = load_reports(args)
        policy = read_json(args.policy)
        exceptions = policy_entries(policy)
        test_roots = dev_test_roots(policy, metadata)
        graph = dependency_graph(metadata)
        runtime_packages = (
            runtime_inventory_from_json(read_json(args.runtime_json))
            if args.runtime_json
            else runtime_inventory(metadata)
        )
        packages = package_ids(metadata)
        findings = collect_advisories(audit)
    except (KeyError, TypeError, ValueError) as error:
        print(f"audit reachability policy error: {error}", file=sys.stderr)
        return 1

    errors: list[str] = []
    seen_exceptions: set[tuple[str, str, str]] = set()
    runtime_count = 0
    accepted_count = 0

    for kind, finding in findings:
        key = advisory_key(finding)
        finding_ids = finding_package_ids(finding, packages)
        label = f"{key[0]} {key[1]}@{key[2]} ({kind})"
        entry = exceptions.get(key)
        if entry is not None:
            seen_exceptions.add(key)
        roots = advisory_runtime_roots(finding, runtime_packages)
        production_roots = roots - test_roots
        if production_roots:
            runtime_count += 1
            print(f"BLOCKED runtime-reachable: {label}")
            print(f"  production roots: {', '.join(sorted(production_roots))}")
            test_only = roots & test_roots
            if test_only:
                print(f"  dev/test roots: {', '.join(sorted(test_only))}")
            errors.append(f"runtime-reachable advisory: {label}")
            continue

        if entry is None:
            print(f"BLOCKED unreviewed dev/test-only: {label}")
            errors.append(f"missing dev/test-only exception: {label}")
            continue

        exception_errors = validate_exception(
            entry, args.today, metadata, graph, finding_ids, roots
        )
        if exception_errors:
            print(f"BLOCKED invalid dev/test-only exception: {label}")
            errors.extend(f"{label}: {error}" for error in exception_errors)
            continue

        path = " -> ".join(
            f"{component['package']}@{component['version']}"
            for component in entry["dependency_path"]
        )
        if roots:
            print(f"ACCEPTED dev/test-tool runtime: {label}")
            print(f"  dev/test roots: {', '.join(sorted(roots))}")
        else:
            print(f"ACCEPTED dev/test-only: {label}")
        print(f"  path: {path}")
        print(
            f"  owner: {entry['owner']}; reviewed: {entry['reviewed_on']}; "
            f"review by: {entry['review_by']}"
        )
        print(f"  reason: {entry['reason']}")
        accepted_count += 1

    for stale in sorted(set(exceptions) - seen_exceptions):
        errors.append(f"stale audit exception: {' '.join(stale)}")

    print()
    print(
        f"Audit reachability summary: {runtime_count} runtime-reachable, "
        f"{accepted_count} reviewed dev/test-only, {len(findings)} total"
    )
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
