#!/usr/bin/env python3
"""Unsafe-code policy (STYLE.md): every unsafe site carries its soundness
argument.

Rules, applied to runtime sources in lang/src, spl/src, metadata/src (test
modules, tests.rs files, and kani harnesses excluded):

  1. Every `unsafe fn` has a `# Safety` section in its doc comment.
     Exception: methods inside `unsafe impl` or trait-impl (`impl T for U`)
     blocks inherit the trait's contract (the impl line itself, when
     `unsafe impl`, needs rule 2).
  2. Every `unsafe {` block and `unsafe impl` has a `SAFETY:` argument that
     touches it: inline, in its statement paragraph above (up to the nearest
     blank line, max eight lines), or as the first lines inside the block.
     `unsafe trait` declarations satisfy the rule with a `# Safety` doc
     section.

The allowlist below names accepted exceptions with reasons; it only shrinks.
"""
import os
import re
import sys

ROOT = os.path.normpath(os.path.join(os.path.dirname(__file__), ".."))
SCAN_DIRS = ("lang/src", "spl/src", "metadata/src")

# (path-suffix, line-content-regex): accepted exceptions, each with a reason.
ALLOWLIST: list[tuple[str, str, str]] = []

DOC_OR_ATTR = re.compile(r"^\s*(///|//!|//|#\[|#!\[)")
UNSAFE_FN = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?unsafe\s+fn\s+(\w+)")
UNSAFE_SITE = re.compile(r"\bunsafe\s*(\{|impl\b|trait\b)")
CFG_TEST = re.compile(r"^\s*#\[cfg\(test\)\]")


def allowlisted(path: str, line: str) -> bool:
    return any(
        path.endswith(suffix) and re.search(pattern, line)
        for suffix, pattern, _reason in ALLOWLIST
    )


def scan_file(path: str, rel: str) -> list[str]:
    with open(path) as handle:
        lines = handle.readlines()
    violations = []
    in_test_module = False
    impl_depth = 0  # brace depth of an enclosing `unsafe impl`, 0 = outside
    for i, line in enumerate(lines):
        # Test modules trail the file by convention (same rule as
        # check-runtime-panics): stop scanning at the first #[cfg(test)].
        if CFG_TEST.match(line):
            in_test_module = True
        if in_test_module:
            break
        stripped = line.strip()
        if stripped.startswith("//"):
            continue

        # Methods inside `unsafe impl` and trait-impl blocks inherit the
        # trait's contract, so rule 1 does not apply to them.
        if impl_depth > 0:
            impl_depth += line.count("{") - line.count("}")
            if impl_depth < 0:
                impl_depth = 0
        elif re.search(r"\bunsafe\s+impl\b", line) or re.search(
            r"^\s*impl\b[^;]*\bfor\b", line
        ):
            impl_depth = line.count("{") - line.count("}")
            if "{" not in line:
                impl_depth = 1

        m = UNSAFE_FN.match(line)
        if m and impl_depth == 0:
            # Walk back over attributes to the doc block; require # Safety.
            j = i - 1
            has_safety = False
            while j >= 0:
                prev = lines[j]
                if DOC_OR_ATTR.match(prev) or not prev.strip():
                    if "# Safety" in prev:
                        has_safety = True
                        break
                    j -= 1
                    continue
                break
            if not has_safety and not allowlisted(rel, line):
                violations.append(
                    f"{rel}:{i + 1}: unsafe fn `{m.group(1)}` has no `# Safety` doc section"
                )
            continue

        if UNSAFE_SITE.search(line):
            if "SAFETY:" in line:
                continue
            # The argument must be in the contiguous comment/attribute block
            # touching this line; `unsafe trait` contracts live in the doc's
            # `# Safety` section instead.
            is_trait_decl = re.search(r"\bunsafe\s+trait\b", line) is not None
            satisfied = False
            # A SAFETY comment covers its statement paragraph: walk up to the
            # nearest blank line (max eight lines), through code and comments.
            j = i - 1
            while j >= 0 and i - j <= 8:
                prev = lines[j]
                if not prev.strip():
                    break
                if "SAFETY:" in prev or (is_trait_decl and "# Safety" in prev):
                    satisfied = True
                    break
                j -= 1
            # Or it opens the block: `unsafe {` with the argument first inside.
            if not satisfied:
                satisfied = any("SAFETY:" in w for w in lines[i + 1 : i + 3])
            if not satisfied and not allowlisted(rel, line):
                kind = "# Safety doc section" if is_trait_decl else "`// SAFETY:` comment above"
                violations.append(f"{rel}:{i + 1}: unsafe site without a {kind}")
    return violations


def main() -> int:
    violations = []
    for scan_dir in SCAN_DIRS:
        base = os.path.join(ROOT, scan_dir)
        for dirpath, _dirnames, filenames in os.walk(base):
            for fname in sorted(filenames):
                if not fname.endswith(".rs") or fname == "tests.rs":
                    continue
                path = os.path.join(dirpath, fname)
                rel = os.path.relpath(path, ROOT)
                violations.extend(scan_file(path, rel))
    if violations:
        print("unsafe sites without soundness arguments (STYLE.md):", file=sys.stderr)
        for violation in violations:
            print(f"  {violation}", file=sys.stderr)
        print(
            "\nName the invariant in a // SAFETY: comment / # Safety section,"
            "\nor add an allowlist entry with justification.",
            file=sys.stderr,
        )
        return 1
    print("unsafe policy passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
