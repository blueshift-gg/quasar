#!/usr/bin/env python3
"""Keep the root README's published-crate table aligned with Cargo metadata."""

from __future__ import annotations

import difflib
import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
README = ROOT / "README.md"
START = "<!-- published-crate-inventory:start -->"
END = "<!-- published-crate-inventory:end -->"


def published_packages() -> list[tuple[str, str, str]]:
    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    metadata = json.loads(result.stdout)
    workspace_root = Path(metadata["workspace_root"])

    packages = []
    for package in metadata["packages"]:
        if package["publish"] == []:
            continue

        package_dir = Path(package["manifest_path"]).parent.relative_to(workspace_root)
        description = package.get("description")
        if not description:
            raise SystemExit(f"{package['name']}: missing package description")
        if "|" in description:
            raise SystemExit(f"{package['name']}: description cannot contain a Markdown table pipe")

        packages.append((package["name"], f"{package_dir.as_posix()}/", description))

    return sorted(packages)


def expected_table() -> str:
    rows = [
        "| Package | Path | Purpose |",
        "| --- | --- | --- |",
    ]
    rows.extend(
        f"| `{name}` | `{path}` | {description} |"
        for name, path, description in published_packages()
    )
    return "\n".join(rows)


def read_table() -> str:
    readme = README.read_text()
    if readme.count(START) != 1 or readme.count(END) != 1:
        raise SystemExit("README must contain one published-crate inventory marker pair")

    before, remainder = readme.split(START, maxsplit=1)
    table, after = remainder.split(END, maxsplit=1)
    if not before or not after:
        raise SystemExit("published-crate inventory markers cannot wrap the whole README")
    return table.strip()


def main() -> None:
    expected = expected_table()
    actual = read_table()
    if actual == expected:
        return

    print("README published-crate inventory does not match Cargo metadata:")
    print(
        "\n".join(
            difflib.unified_diff(
                actual.splitlines(),
                expected.splitlines(),
                fromfile="README.md",
                tofile="Cargo metadata",
                lineterm="",
            )
        )
    )
    raise SystemExit(1)


if __name__ == "__main__":
    main()
