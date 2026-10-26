"""Repository convention checks (Python 3.11+, standard library only).

Source checks are lexical guardrails, not proofs of panic freedom or Rust name
resolution. They cover explicit panic macros and CLI exit/sleep/splitting APIs;
Clippy, tests, Miri and Kani cover other failure paths.
"""

import argparse
from collections import Counter
import json
from pathlib import Path
import re
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[2]
RUNTIME_DIRS = ("lang/src", "spl/src", "metadata/src", "derive/src")

# Exact call text and count, not file-wide exemptions. Removing or duplicating
# a call requires reviewing this list too. Every exception needs a reason.
PANIC_EXCEPTIONS = {
    ("lang/src/data.rs", 'panic!("instruction data has one size: use `ArrayString<N>` or `ArrayVec<T, N>`")'):
        (1, "Const size validation rejects unsupported instruction field types."),
    ("lang/src/data.rs", 'unreachable!("a value of one size fits it")'):
        (1, "Fixed-size serialization invariant after selecting the field-sized output slice."),
    ("spl/src/token.rs", 'panic!("an SPL layout has one size")'):
        (1, "Const size validation for fixed SPL layouts."),
    ("lang/src/syscalls.rs", 'panic!("the program ended")'):
        (1, "Host-only simulation of a program abort."),
    ("lang/src/syscalls.rs", 'unreachable!("syscalls run only on the SBF build")'):
        (4, "Host-only stubs reject syscalls unavailable outside SBF."),
    ("lang/src/verify/models.rs", 'panic!("the program ended")'):
        (1, "Kani-only model of a program abort."),
    ("lang/src/verify/models.rs", 'unimplemented!("calls to other programs have no model yet")'):
        (1, "Kani must fail explicitly for unsupported CPI models."),
    ("lang/src/verify/world.rs", 'unreachable!()'):
        (3, "Kani-only loading follows successful-decode assumptions or assertions."),
    ("derive/src/property.rs", 'unreachable!()'):
        (1, "Generated Kani harness assumes successful decoding before reading data."),
}


def mask_non_code(source):
    """Blank Rust comments/literals, preserving offsets and line numbers."""
    masked = list(source)
    at = 0
    while at < len(source):
        end = at + 1
        if source.startswith("//", at):
            end = source.find("\n", at)
            if end < 0:
                end = len(source)
        elif source.startswith("/*", at):
            end, depth = at + 2, 1
            while end < len(source) and depth:
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            if depth:
                raise ValueError("unterminated block comment")
        else:
            raw = re.match(r'(?:br|cr|r)(#*)"', source[at:]) if source[at] in "bcr" else None
            char = re.match(r"'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]+\}|.)|[^'\\\n])'", source[at:]) if source[at] == "'" else None
            if raw:
                closing = '"' + raw[1]
                end = source.find(closing, at + raw.end())
                if end < 0:
                    raise ValueError("unterminated raw string")
                end += len(closing)
            elif source[at] == '"':
                end = at + 1
                while end < len(source) and source[end] != '"':
                    end += 2 if source[end] == "\\" else 1
                if end >= len(source):
                    raise ValueError("unterminated string")
                end += 1
            elif char:
                end = at + char.end()
            else:
                at += 1
                continue
        masked[at:end] = ["\n" if c == "\n" else " " for c in source[at:end]]
        at = end
    return "".join(masked)


def normalize(call):
    return re.sub(r"\s+", "", call)


def panic_calls(source):
    code = mask_non_code(source)
    for match in re.finditer(r"\b(?:panic|unreachable|todo|unimplemented)\s*!\s*([({\[])", code):
        stack = [match[1]]
        end = match.end()
        while stack and end < len(code):
            char = code[end]
            if char in "({[":
                stack.append(char)
            elif char in ")}]":
                if stack.pop() != {")": "(", "}": "{", "]": "["}[char]:
                    raise ValueError("unbalanced panic macro")
            end += 1
        if stack:
            raise ValueError("unterminated panic macro")
        yield source.count("\n", 0, match.start()) + 1, source[match.start():end]


def runtime_panics(sources, exceptions=PANIC_EXCEPTIONS):
    allowed = {(path, normalize(call)): (count, reason) for (path, call), (count, reason) in exceptions.items()}
    seen = Counter()
    errors = []
    for path, source in sources.items():
        for line, call in panic_calls(source):
            key = (path, normalize(call))
            seen[key] += 1
            if key not in allowed or seen[key] > allowed[key][0]:
                errors.append(f"{path}:{line}: unexpected panic macro: {call.strip()}")
    for key, (count, reason) in allowed.items():
        if not reason.strip():
            errors.append(f"{key[0]}: panic exception needs a reason")
        if seen[key] < count:
            errors.append(f"{key[0]}: stale panic exception ({seen[key]}/{count}): {key[1]}")
    return errors


def cli_conventions(sources):
    errors = []
    rules = (
        (r"\bexit\b", "cli/src/main.rs", "process exits belong at the CLI boundary"),
        (r"\bsleep\b", "cli/src/build_watch.rs", "polling sleeps belong in build_watch.rs"),
        (r"\bsplit_(?:ascii_)?whitespace\b", None, "use structured arguments or quote-aware command parsing"),
    )
    for path, source in sources.items():
        code = mask_non_code(source)
        for pattern, permitted, message in rules:
            if path == permitted:
                continue
            for match in re.finditer(pattern, code):
                line = code.count("\n", 0, match.start()) + 1
                errors.append(f"{path}:{line}: {message}")
    return errors


def lint_opt_in(manifests):
    return [f"{path}: missing [lints] workspace = true"
            for path, manifest in manifests.items()
            if tomllib.loads(manifest).get("lints", {}).get("workspace") is not True]


def sources(root, directories):
    return {path.relative_to(root).as_posix(): path.read_text()
            for directory in directories for path in sorted((root / directory).rglob("*.rs"))}


def check(root, policy):
    if policy == "workspace-lints":
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"], cwd=root, text=True))
        members = set(metadata["workspace_members"])
        paths = [Path(p["manifest_path"]) for p in metadata["packages"] if p["id"] in members]
        return lint_opt_in({str(p.relative_to(root)): p.read_text() for p in paths})
    if policy == "runtime-panics":
        return runtime_panics(sources(root, RUNTIME_DIRS))
    return cli_conventions(sources(root, ("cli/src",)))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("policy", choices=("workspace-lints", "runtime-panics", "cli"))
    args = parser.parse_args()
    errors = check(ROOT, args.policy)
    if errors:
        raise SystemExit("\n".join(errors))
    print(f"{args.policy}: passed")


if __name__ == "__main__":
    main()
