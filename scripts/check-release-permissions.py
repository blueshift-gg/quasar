#!/usr/bin/env python3
"""Audit the tag-triggered release workflow's authority boundaries."""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Block:
    name: str
    start: int
    end: int


def value_without_comment(value: str) -> str:
    return value.split(" #", 1)[0].strip()


def job_blocks(lines: list[str]) -> dict[str, Block]:
    try:
        jobs_line = lines.index("jobs:")
    except ValueError:
        return {}

    starts: list[tuple[str, int]] = []
    jobs_end = len(lines)
    for index in range(jobs_line + 1, len(lines)):
        line = lines[index]
        if line and not line.startswith(" "):
            jobs_end = index
            break
        match = re.fullmatch(r"  ([A-Za-z0-9_-]+):\s*", line)
        if match:
            starts.append((match.group(1), index))

    blocks: dict[str, Block] = {}
    for position, (name, start) in enumerate(starts):
        end = starts[position + 1][1] if position + 1 < len(starts) else jobs_end
        blocks[name] = Block(name, start, end)
    return blocks


def permissions_at(lines: list[str], index: int, indentation: int) -> dict[str, str]:
    header = lines[index][indentation + len("permissions:") :].strip()
    if header:
        return {"<all>": value_without_comment(header)}

    permissions: dict[str, str] = {}
    child_prefix = " " * (indentation + 2)
    for line in lines[index + 1 :]:
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        current_indentation = len(line) - len(line.lstrip())
        if current_indentation <= indentation:
            break
        if current_indentation != indentation + 2:
            continue
        match = re.fullmatch(rf"{re.escape(child_prefix)}([A-Za-z-]+):\s*(.+)", line)
        if match:
            permissions[match.group(1)] = value_without_comment(match.group(2))
    return permissions


def top_level_permissions(lines: list[str]) -> tuple[dict[str, str], int] | None:
    for index, line in enumerate(lines):
        if line.startswith("jobs:"):
            break
        if line.startswith("permissions:"):
            return permissions_at(lines, index, 0), index
    return None


def job_permissions(lines: list[str], block: Block) -> tuple[dict[str, str], int] | None:
    for index in range(block.start + 1, block.end):
        if re.fullmatch(r"    permissions:.*", lines[index]):
            return permissions_at(lines, index, 4), index
    return None


def job_environment(lines: list[str], block: Block) -> tuple[str, int] | None:
    pattern = re.compile(r"    environment:\s*(.*)")
    for index in range(block.start + 1, block.end):
        match = pattern.fullmatch(lines[index])
        if match:
            value = value_without_comment(match.group(1))
            if value:
                return value, index
            for child_index in range(index + 1, block.end):
                child = lines[child_index]
                if not child.strip() or child.lstrip().startswith("#"):
                    continue
                indentation = len(child) - len(child.lstrip())
                if indentation <= 4:
                    break
                name = re.fullmatch(r"      name:\s*(.+)", child)
                if name:
                    return value_without_comment(name.group(1)), index
            return "<missing name>", index
    return None


def secret_references(
    lines: list[str], blocks: dict[str, Block]
) -> list[tuple[str, str, int]]:
    pattern = re.compile(
        r"secrets(?:\.([A-Za-z_][A-Za-z0-9_]*)|\[['\"]([^'\"]+)['\"]\])"
    )
    matches: list[tuple[str, str, int]] = []
    for block in blocks.values():
        for index in range(block.start, block.end):
            if lines[index].lstrip().startswith("#"):
                continue
            for match in pattern.finditer(lines[index]):
                matches.append((block.name, match.group(1) or match.group(2), index))
    return matches


def inherited_secret_jobs(lines: list[str], blocks: dict[str, Block]) -> list[tuple[str, int]]:
    matches: list[tuple[str, int]] = []
    for block in blocks.values():
        for index in range(block.start + 1, block.end):
            if re.fullmatch(r"    secrets:\s*inherit\s*", lines[index]):
                matches.append((block.name, index))
    return matches


def check(root: Path) -> list[str]:
    workflow = root.resolve() / ".github" / "workflows" / "release.yml"
    try:
        lines = workflow.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        return [f".github/workflows/release.yml: cannot read workflow: {error}"]

    errors: list[str] = []
    default = top_level_permissions(lines)
    if default is None:
        errors.append(".github/workflows/release.yml: missing workflow permissions default")
    elif default[0] != {"contents": "read"}:
        errors.append(
            f".github/workflows/release.yml:{default[1] + 1}: "
            f"release permissions must default to contents: read; found {default[0]}"
        )

    blocks = job_blocks(lines)
    if not blocks:
        errors.append(".github/workflows/release.yml: missing jobs mapping")
        return errors

    for block in blocks.values():
        declared = job_permissions(lines, block)
        permissions = declared[0] if declared else {}
        writes = {
            name: access
            for name, access in permissions.items()
            if access in {"write", "write-all"}
        }
        if block.name == "github-release":
            if permissions != {"contents": "write"}:
                line = declared[1] + 1 if declared else block.start + 1
                errors.append(
                    f".github/workflows/release.yml:{line}: github-release must grant "
                    f"only contents: write; found {permissions or '<inherited read>'}"
                )
        elif writes:
            line = declared[1] + 1 if declared else block.start + 1
            errors.append(
                f".github/workflows/release.yml:{line}: job {block.name} has "
                f"unauthorized write permissions: {writes}"
            )

    publisher_environments = {
        "publish": "crates-io",
        "publish-typescript": "npmjs",
    }
    for job, expected_environment in publisher_environments.items():
        publisher = blocks.get(job)
        if publisher is None:
            errors.append(f".github/workflows/release.yml: missing {job} job")
            continue
        environment = job_environment(lines, publisher)
        if environment is None or environment[0] != expected_environment:
            line = environment[1] + 1 if environment else publisher.start + 1
            value = environment[0] if environment else "<missing>"
            errors.append(
                f".github/workflows/release.yml:{line}: {job} job must use the "
                f"{expected_environment} environment; found {value}"
            )

    for block in blocks.values():
        environment = job_environment(lines, block)
        if environment and block.name not in publisher_environments:
            errors.append(
                f".github/workflows/release.yml:{environment[1] + 1}: "
                f"publisher environment is attached to non-publish job {block.name}"
            )

    secrets = secret_references(lines, blocks)
    cargo_tokens = [
        reference for reference in secrets if reference[1] == "CARGO_REGISTRY_TOKEN"
    ]
    if not cargo_tokens:
        errors.append(".github/workflows/release.yml: publish job lacks bootstrap token")
    for job, _, line in cargo_tokens:
        if job != "publish":
            errors.append(
                f".github/workflows/release.yml:{line + 1}: crates.io token is "
                f"available to non-publish job {job}"
            )

    npm_tokens = [reference for reference in secrets if reference[1] == "NPM_TOKEN"]
    if not npm_tokens:
        errors.append(".github/workflows/release.yml: publish-typescript job lacks npm token")
    for job, _, line in npm_tokens:
        if job != "publish-typescript":
            errors.append(
                f".github/workflows/release.yml:{line + 1}: npm token is "
                f"available to non-TypeScript-publish job {job}"
            )

    github_tokens = [reference for reference in secrets if reference[1] == "GITHUB_TOKEN"]
    if not github_tokens:
        errors.append(".github/workflows/release.yml: github-release lacks GITHUB_TOKEN")
    for job, _, line in github_tokens:
        if job != "github-release":
            errors.append(
                f".github/workflows/release.yml:{line + 1}: GITHUB_TOKEN is "
                f"explicitly exposed to non-release job {job}"
            )

    for job, name, line in secrets:
        expected = {
            "publish": {"CARGO_REGISTRY_TOKEN"},
            "publish-typescript": {"NPM_TOKEN"},
            "github-release": {"GITHUB_TOKEN"},
        }.get(job, set())
        if name not in expected:
            errors.append(
                f".github/workflows/release.yml:{line + 1}: unauthorized secret "
                f"{name} in job {job}"
            )

    for job, line in inherited_secret_jobs(lines, blocks):
        errors.append(
            f".github/workflows/release.yml:{line + 1}: job {job} inherits all secrets"
        )

    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    errors = check(args.root)
    if errors:
        print("release permission violations:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 1

    print("release permission audit:")
    print("  default and verification jobs: contents: read")
    print("  publish: crates-io environment boundary and bootstrap token")
    print("  publish-typescript: npmjs environment boundary and npm token")
    print("  github-release: contents: write for gh release create")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
