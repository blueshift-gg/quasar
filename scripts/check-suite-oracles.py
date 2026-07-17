#!/usr/bin/env python3
"""Suite oracle policy (TESTING.md): a rejection test must pin the exact
error, not just that *something* failed.

Every #[test] fn under tests/suite/src that asserts `.is_err()` must also
contain an exact-error oracle in the same fn:
  - `assert_error(...)`               (QuasarSvm ExecutionResult), or
  - `ProgramResult::Failure(...)`     (Mollusk), or
  - `Err(InstructionError::...)`      (raw result comparison).

Files in ALLOWLIST are exempt (each entry carries its reason); shrinking the
allowlist is always welcome, growing it requires justification in the PR.
"""
import os
import re
import sys

SUITE = os.path.join(os.path.dirname(__file__), "..", "tests", "suite", "src")

ALLOWLIST = {
    # In-progress working tree module; swept when its feature lands.
    "optional_accounts.rs",
}

EXACT_MARKERS = ("assert_error(", "ProgramResult::Failure(", "Err(InstructionError::")


def test_fns(src):
    for m in re.finditer(r"#\[test\]\s*(?:#\[[^\]]*\]\s*)*fn\s+(\w+)\s*\([^)]*\)[^{]*\{", src):
        depth, i = 1, m.end()
        while i < len(src) and depth:
            if src[i] == "{":
                depth += 1
            elif src[i] == "}":
                depth -= 1
            i += 1
        yield m.group(1), src[m.end() : i - 1]


def violations(root):
    out = []
    for fname in sorted(os.listdir(root)):
        if not fname.endswith(".rs") or fname in ALLOWLIST:
            continue
        with open(os.path.join(root, fname)) as handle:
            src = handle.read()
        for name, body in test_fns(src):
            if ".is_err()" in body and not any(marker in body for marker in EXACT_MARKERS):
                out.append(f"{fname}::{name}")
    return out


def main():
    found = violations(SUITE)
    if found:
        print("rejection tests without an exact-error oracle (TESTING.md):", file=sys.stderr)
        for v in found:
            print(f"  {v}", file=sys.stderr)
        print(
            "\nAssert the exact error (assert_error / ProgramResult::Failure /"
            "\nErr(InstructionError::...)), or add the file to the allowlist"
            "\nwith justification.",
            file=sys.stderr,
        )
        return 1
    print(f"suite oracle policy passed ({SUITE})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
