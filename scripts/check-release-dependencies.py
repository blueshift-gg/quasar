#!/usr/bin/env python3
"""Reject mutable dependencies in the tag-triggered release closure."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


REMOTE_ACTION = re.compile(r"^[^\s/@]+/[^\s/@]+(?:/[^\s@]+)?@[0-9a-f]{40}$")
USES = re.compile(r"^\s*(?:-\s*)?uses:\s*([^#\s]+)", re.MULTILINE)
DOCKERFILE = re.compile(r"^\s*(?:--file|-f)\s+([^\s\\]+)", re.MULTILINE)
FROM = re.compile(r"^\s*FROM(?:\s+--platform=\S+)?\s+(\S+)", re.MULTILINE)
SHA256 = re.compile(r"^[0-9a-f]{64}$")
REVISION = re.compile(r"^[0-9a-f]{40}$")
VERSION = re.compile(r"^[0-9]+(?:\.[0-9]+)+(?:[-+][0-9A-Za-z.-]+)?$")
SNAPSHOT = re.compile(r"^[0-9]{8}T[0-9]{6}Z$")
ENV_REFERENCE = re.compile(r"^\$\{\{\s*env\.([A-Z][A-Z0-9_]*)\s*\}\}$")


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def relative(root: Path, path: Path) -> str:
    return path.relative_to(root).as_posix()


def read(root: Path, path: Path, errors: list[str]) -> str | None:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as error:
        errors.append(f"{relative(root, path)}: cannot read dependency: {error}")
        return None


def release_workflow_closure(root: Path, errors: list[str]) -> dict[Path, str]:
    pending = [root / ".github" / "workflows" / "release.yml"]
    workflows: dict[Path, str] = {}

    while pending:
        workflow = pending.pop()
        if workflow in workflows:
            continue
        text = read(root, workflow, errors)
        if text is None:
            continue
        workflows[workflow] = text

        for match in USES.finditer(text):
            reference = match.group(1)
            location = f"{relative(root, workflow)}:{line_number(text, match.start())}"
            if reference.startswith("./"):
                dependency = root / reference[2:]
                if not dependency.exists():
                    errors.append(f"{location}: missing local action or workflow: {reference}")
                elif dependency.parent == root / ".github" / "workflows":
                    pending.append(dependency)
                continue
            if not REMOTE_ACTION.fullmatch(reference):
                errors.append(f"{location}: mutable action reference: {reference}")

    return workflows


def referenced_dockerfiles(
    root: Path, workflows: dict[Path, str], errors: list[str]
) -> dict[Path, str]:
    dockerfiles: dict[Path, str] = {}
    for workflow, text in workflows.items():
        for match in DOCKERFILE.finditer(text):
            path = root / match.group(1)
            dockerfile = read(root, path, errors)
            if dockerfile is not None:
                dockerfiles[path] = dockerfile

    for path, text in dockerfiles.items():
        for match in FROM.finditer(text):
            image = match.group(1)
            if not re.search(r"@sha256:[0-9a-f]{64}$", image):
                errors.append(
                    f"{relative(root, path)}:{line_number(text, match.start())}: "
                    f"mutable container image: {image}"
                )
    return dockerfiles


def declared_values(text: str, name: str) -> list[tuple[str, int]]:
    pattern = re.compile(rf"^\s*{re.escape(name)}:\s*([^\s#]+)", re.MULTILINE)
    return [(match.group(1), line_number(text, match.start())) for match in pattern.finditer(text)]


def check_checkout_revisions(path: Path, root: Path, text: str, errors: list[str]) -> None:
    lines = text.splitlines()
    for index, line in enumerate(lines):
        repository_match = re.match(r"^(\s*)repository:\s*([^\s#]+)", line)
        if repository_match is None:
            continue
        indentation = len(repository_match.group(1))
        repository = repository_match.group(2)
        reference = "<default branch>"
        for candidate in lines[index + 1 :]:
            if not candidate.strip():
                continue
            candidate_indentation = len(candidate) - len(candidate.lstrip())
            if candidate_indentation < indentation:
                break
            reference_match = re.match(r"^\s*ref:\s*([^#]+?)\s*$", candidate)
            if reference_match:
                reference = reference_match.group(1)
                break

        if REVISION.fullmatch(reference):
            continue
        env_match = ENV_REFERENCE.fullmatch(reference)
        if env_match:
            values = declared_values(text, env_match.group(1))
            if len(values) == 1 and REVISION.fullmatch(values[0][0]):
                continue
        errors.append(
            f"{relative(root, path)}:{index + 1}: mutable checkout reference "
            f"for {repository}: {reference}"
        )


def check_workflow_containers(path: Path, root: Path, text: str, errors: list[str]) -> None:
    for pattern in (
        re.compile(r"^\s*container:\s*(\S(?:[^#]*\S)?)\s*$", re.MULTILINE),
        re.compile(r"^\s*image:\s*([^\s#]+)", re.MULTILINE),
    ):
        for match in pattern.finditer(text):
            image = match.group(1)
            if not re.search(r"@sha256:[0-9a-f]{64}$", image):
                errors.append(
                    f"{relative(root, path)}:{line_number(text, match.start())}: "
                    f"mutable workflow container image: {image}"
                )


def check_pinned_inputs(
    root: Path,
    workflows: dict[Path, str],
    dockerfiles: dict[Path, str],
    errors: list[str],
) -> None:
    checksums: set[str] = set()
    for path, text in workflows.items():
        location = relative(root, path)
        check_checkout_revisions(path, root, text, errors)
        check_workflow_containers(path, root, text, errors)
        if "release.anza.xyz" in text:
            line = line_number(text, text.index("release.anza.xyz"))
            errors.append(f"{location}:{line}: mutable Solana installer URL")
        for match in re.finditer(r"\b(?:curl|wget)\b", text):
            errors.append(
                f"{location}:{line_number(text, match.start())}: "
                "direct network fetch in release workflow"
            )

        if "SOLANA_VERSION:" in text:
            values = declared_values(text, "SOLANA_LINUX_SHA256")
            if not values:
                errors.append(f"{location}: missing SOLANA_LINUX_SHA256")
            for value, line in values:
                if not SHA256.fullmatch(value):
                    errors.append(f"{location}:{line}: invalid Solana artifact SHA-256: {value}")
                checksums.add(value)

        for name, validator, description in (
            ("CARAVEL_REV", REVISION, "Caravel revision"),
            ("Z3_VERSION", VERSION, "z3 package version"),
        ):
            for value, line in declared_values(text, name):
                if not validator.fullmatch(value):
                    errors.append(f"{location}:{line}: invalid {description}: {value}")

        for match in re.finditer(r"apt-get install[^\n]*\bz3(?:\s|$|\")", text):
            command = match.group(0)
            if 'z3=${{ env.Z3_VERSION }}' not in command:
                errors.append(
                    f"{location}:{line_number(text, match.start())}: "
                    "mutable z3 package reference"
                )

    if len(checksums) > 1:
        errors.append(
            "release workflow closure: inconsistent SOLANA_LINUX_SHA256 values: "
            + ", ".join(sorted(checksums))
        )

    for path, text in dockerfiles.items():
        location = relative(root, path)
        if "release.anza.xyz" in text:
            line = line_number(text, text.index("release.anza.xyz"))
            errors.append(f"{location}:{line}: mutable Solana installer URL")
        if "apt-get update" in text:
            match = re.search(r"^ARG DEBIAN_SNAPSHOT=([^\s#]+)", text, re.MULTILINE)
            if match is None or not SNAPSHOT.fullmatch(match.group(1)):
                line = line_number(text, match.start()) if match else 1
                value = match.group(1) if match else "<missing>"
                errors.append(f"{location}:{line}: mutable Debian snapshot: {value}")
            for archive in ("debian", "debian-security"):
                expected = f"snapshot.debian.org/archive/{archive}/${{DEBIAN_SNAPSHOT}}"
                if expected not in text:
                    errors.append(f"{location}: missing immutable {archive} snapshot source")
        logical_text = text.replace("\\\n", " ")
        for match in re.finditer(r"cargo\s+install\s+--path\b[^&\n]*", logical_text):
            if "--locked" not in match.group(0):
                original_offset = text.find("cargo install --path")
                errors.append(
                    f"{location}:{line_number(text, original_offset)}: "
                    "unlocked local cargo install"
                )

    installer = root / "scripts" / "install-solana-tools.sh"
    installer_text = read(root, installer, errors)
    if installer_text is None:
        return
    installer_location = relative(root, installer)
    expected_url = (
        "https://github.com/anza-xyz/agave/releases/download/${version}/"
        "solana-release-${target}.tar.bz2"
    )
    if expected_url not in installer_text:
        errors.append(f"{installer_location}: Solana artifact URL is not version-pinned")
    if "sha256sum -c" not in installer_text:
        errors.append(f"{installer_location}: Solana artifact is not SHA-256 verified")


def check(root: Path) -> list[str]:
    root = root.resolve()
    errors: list[str] = []
    workflows = release_workflow_closure(root, errors)
    dockerfiles = referenced_dockerfiles(root, workflows, errors)
    check_pinned_inputs(root, workflows, dockerfiles, errors)
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    errors = check(args.root)
    if errors:
        print("mutable release dependencies:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 1
    print("release dependency closure is immutable")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
