#!/usr/bin/env python3
"""Inventory required Cargo tests and reject tests hidden from every runner."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any


TEST_ATTRIBUTE = re.compile(r"(?m)^\s*#\s*\[\s*test\s*\]")
GENERATED_CLIENT_TARGET = ("quasar-cli", "generated_clients_smoke")
MIRI_TARGETS = {
    ("quasar-lang", "miri"),
    ("quasar-spl", "miri"),
    ("quasar-metadata", "miri"),
}
SBF_HOST_PACKAGES = {
    "quasar-vault",
    "quasar-escrow",
    "quasar-multisig",
    "upstream-vault",
    "quasar-test-suite",
}
EXCLUDED_TEST_ROOTS = {
    "compatibility-baselines": (
        "versioned generated artifacts; executable coverage is enforced by dedicated "
        "compatibility gates"
    ),
    "lang/fuzz": "cargo-fuzz crate; fuzz target compilation is tracked by issue #271",
}


def cargo_metadata(root: Path) -> dict[str, Any]:
    completed = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(completed.stdout)


def tracked_rust_files(root: Path) -> list[Path]:
    completed = subprocess.run(
        ["git", "ls-files", "*.rs"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return [root / line for line in completed.stdout.splitlines() if line]


def excluded_test_root(relative: Path) -> str | None:
    return next(
        (
            excluded_root
            for excluded_root in EXCLUDED_TEST_ROOTS
            if relative.is_relative_to(excluded_root)
        ),
        None,
    )


def runners_for(package: str, target: str) -> list[str]:
    key = (package, target)
    if key == GENERATED_CLIENT_TARGET:
        return ["make generated-client-smoke"]
    if key in MIRI_TARGETS:
        return ["make test-host", "make test-miri"]
    if package in SBF_HOST_PACKAGES:
        return ["make test-sbf-host"]
    return ["make test-host"]


def without_raw_strings(text: str) -> str:
    """Blank Rust raw strings while retaining newlines for stable scanning."""

    output: list[str] = []
    index = 0
    while index < len(text):
        prefix_length = 0
        if text.startswith("br", index):
            prefix_length = 2
        elif text.startswith("r", index):
            prefix_length = 1

        starts_token = index == 0 or not (
            text[index - 1].isalnum() or text[index - 1] == "_"
        )
        if prefix_length and starts_token:
            cursor = index + prefix_length
            while cursor < len(text) and text[cursor] == "#":
                cursor += 1
            if cursor < len(text) and text[cursor] == '"':
                hashes = text[index + prefix_length : cursor]
                terminator = '"' + hashes
                end = text.find(terminator, cursor + 1)
                if end == -1:
                    end = len(text) - len(terminator)
                segment_end = min(len(text), end + len(terminator))
                output.extend("\n" if char == "\n" else " " for char in text[index:segment_end])
                index = segment_end
                continue

        output.append(text[index])
        index += 1
    return "".join(output)


def owning_package(file: Path, packages: list[dict[str, Any]]) -> dict[str, Any] | None:
    owners = []
    for package in packages:
        root = Path(package["manifest_path"]).parent
        if file.is_relative_to(root):
            owners.append((len(root.parts), package))
    return max(owners, default=(0, None), key=lambda item: item[0])[1]


def nearest_manifest_root(file: Path, workspace_root: Path) -> Path | None:
    for parent in (file.parent, *file.parents):
        if (parent / "Cargo.toml").is_file():
            return parent
        if parent == workspace_root:
            break
    return None


def source_target(
    file: Path,
    package: dict[str, Any],
    targets: dict[tuple[str, str], dict[str, Any]],
) -> tuple[str, str] | None:
    package_name = package["name"]
    package_root = Path(package["manifest_path"]).parent
    relative = file.relative_to(package_root)

    exact = [
        (package_name, target["name"])
        for target in package["targets"]
        if Path(target["src_path"]) == file
    ]
    if exact:
        return exact[0]

    if relative.parts[0] == "src":
        library = [
            (package_name, target["name"])
            for target in package["targets"]
            if {"lib", "proc-macro"}.intersection(target["kind"])
        ]
        return library[0] if library else None

    if relative.parts[0] == "tests" and len(relative.parts) >= 2:
        root_name = Path(relative.parts[1]).stem
        key = (package_name, root_name)
        return key if key in targets else None

    return None


def build_inventory(
    metadata: dict[str, Any],
    root: Path,
    tested_packages: set[str],
    sbf_packages: set[str],
) -> dict[str, Any]:
    packages = metadata["packages"]
    publishable = {
        package["name"] for package in packages if package.get("publish") != []
    }
    if tested_packages != publishable:
        missing = sorted(publishable - tested_packages)
        unknown = sorted(tested_packages - publishable)
        raise RuntimeError(
            "publishable host package list drifted: "
            f"missing={missing or '[]'}, unknown={unknown or '[]'}"
        )
    if sbf_packages != SBF_HOST_PACKAGES:
        missing = sorted(SBF_HOST_PACKAGES - sbf_packages)
        unknown = sorted(sbf_packages - SBF_HOST_PACKAGES)
        raise RuntimeError(
            "SBF host package list drifted: "
            f"missing={missing or '[]'}, unknown={unknown or '[]'}"
        )

    targets: dict[tuple[str, str], dict[str, Any]] = {}
    for package in packages:
        for target in package["targets"]:
            key = (package["name"], target["name"])
            targets[key] = target

    test_counts: dict[tuple[str, str], int] = {}
    errors: list[str] = []
    workspace_package_roots = {
        Path(package["manifest_path"]).parent for package in packages
    }
    for file in tracked_rust_files(root):
        text = file.read_text(encoding="utf-8")
        count = len(TEST_ATTRIBUTE.findall(without_raw_strings(text)))
        if count == 0:
            continue

        nearest_manifest = nearest_manifest_root(file, root)
        if nearest_manifest not in workspace_package_roots:
            relative = file.relative_to(root)
            excluded = excluded_test_root(relative)
            if excluded is not None:
                continue
            errors.append(
                f"{relative}: nested non-workspace test is not explicitly excluded"
            )
            continue

        package = owning_package(file, packages)
        if package is None:
            errors.append(f"{file.relative_to(root)}: no workspace package owns test file")
            continue

        key = source_target(file, package, targets)
        if key is None:
            errors.append(f"{file.relative_to(root)}: no Cargo test target found")
            continue

        target = targets[key]
        if not target["test"]:
            errors.append(
                f"{file.relative_to(root)}: Cargo target {key[0]}/{key[1]} has test=false"
            )
            continue
        test_counts[key] = test_counts.get(key, 0) + count

    if errors:
        raise RuntimeError("unassigned test inventory:\n  " + "\n  ".join(errors))

    unassigned_packages = sorted(
        {
            package["name"]
            for package in packages
            if any(target["test"] for target in package["targets"])
            and package["name"] not in publishable
            and package["name"] not in sbf_packages
        }
    )
    if unassigned_packages:
        raise RuntimeError(
            f"non-publishable test packages need a required runner: {unassigned_packages}"
        )

    inventory_targets = []
    for package in packages:
        for target in package["targets"]:
            if not target["test"]:
                continue
            key = (package["name"], target["name"])
            inventory_targets.append(
                {
                    "package": key[0],
                    "target": key[1],
                    "kind": target["kind"],
                    "runners": runners_for(*key),
                    "test_count": test_counts.get(key, 0),
                    "source": str(Path(target["src_path"]).relative_to(root)),
                }
            )

    inventory_targets.sort(key=lambda item: (item["package"], item["target"]))
    return {
        "schema_version": 1,
        "publishable_host_packages": sorted(publishable),
        "excluded_roots": [
            {"path": path, "reason": reason}
            for path, reason in sorted(EXCLUDED_TEST_ROOTS.items())
        ],
        "targets": inventory_targets,
    }


def cli_host_args(metadata: dict[str, Any]) -> list[str]:
    package = next(package for package in metadata["packages"] if package["name"] == "quasar-cli")
    args: list[str] = []
    for target in package["targets"]:
        if not target["test"] or "make test-host" not in runners_for(
            package["name"], target["name"]
        ):
            continue
        kinds = set(target["kind"])
        if "lib" in kinds:
            args.append("--lib")
        elif "bin" in kinds:
            args.extend(("--bin", target["name"]))
        elif "test" in kinds:
            args.extend(("--test", target["name"]))
    return args


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tested-package", action="append", default=[])
    parser.add_argument("--sbf-package", action="append", default=[])
    parser.add_argument("--cli-host-args", action="store_true")
    args = parser.parse_args()

    root = Path(__file__).resolve().parent.parent
    metadata = cargo_metadata(root)
    if args.cli_host_args:
        print(" ".join(cli_host_args(metadata)))
        return 0

    inventory = build_inventory(
        metadata,
        root,
        set(args.tested_package),
        set(args.sbf_package),
    )
    json.dump(inventory, sys.stdout, indent=2)
    print()
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"host-test inventory failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
