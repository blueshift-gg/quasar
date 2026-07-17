#!/usr/bin/env python3
"""Feature -> test traceability matrix (TESTING.md).

Reads tests/feature-matrix.tsv and counts, per feature:
  ok        #[test] fns in the listed suite modules without an exact-failure
            oracle (state-asserting positives)
  err       #[test] fns WITH an exact-failure oracle (assert_error /
            ProgramResult::Failure / Err(InstructionError::...))
  kani      #[kani::proof] harnesses in the listed globs
  trybuild  compile-fail cases in the listed globs

Default mode prints the matrix. --check exits non-zero when:
  - any `required` cell for any feature is zero,
  - a listed suite module does not exist (manifest drift), or
  - a suite module exists that no feature claims (untracked surface).
"""
import argparse
import glob
import os
import re
import sys

ROOT = os.path.normpath(os.path.join(os.path.dirname(__file__), ".."))
MANIFEST = os.path.join(ROOT, "tests", "feature-matrix.tsv")
SUITE = os.path.join(ROOT, "tests", "suite", "src")
UNCLAIMED_EXEMPT = {"lib.rs", "helpers.rs"}
EXACT_MARKERS = ("assert_error(", "ProgramResult::Failure(", "Err(InstructionError::")
CELLS = ("ok", "err", "kani", "trybuild")


def parse_manifest(path):
    rows = []
    with open(path) as handle:
        for lineno, line in enumerate(handle, 1):
            line = line.rstrip("\n")
            if not line or line.startswith("#"):
                continue
            parts = line.split("\t")
            if len(parts) != 6:
                raise SystemExit(f"{path}:{lineno}: expected 6 tab-separated fields, got {len(parts)}")
            feature, modules, kani, trybuild, required, notes = parts
            bad = [cell for cell in required.split(",") if cell not in CELLS]
            if bad:
                raise SystemExit(f"{path}:{lineno}: unknown required cells: {bad}")
            rows.append({
                "feature": feature,
                "modules": [] if modules == "-" else modules.split(","),
                "kani": [] if kani == "-" else kani.split(","),
                "trybuild": [] if trybuild == "-" else trybuild.split(","),
                "required": required.split(","),
                "notes": notes,
            })
    return rows


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


def count_module(path):
    ok = err = 0
    with open(path) as handle:
        src = handle.read()
    for _, body in test_fns(src):
        if any(marker in body for marker in EXACT_MARKERS):
            err += 1
        else:
            ok += 1
    return ok, err


def count_kani(globs):
    total = 0
    for pattern in globs:
        for path in glob.glob(os.path.join(ROOT, pattern)):
            with open(path) as handle:
                total += handle.read().count("#[kani::proof]")
    return total


def count_trybuild(globs):
    total = 0
    for pattern in globs:
        total += sum(1 for p in glob.glob(os.path.join(ROOT, pattern)) if p.endswith(".rs"))
    return total


def build(rows):
    problems = []
    claimed = set()
    table = []
    for row in rows:
        ok = err = 0
        for module in row["modules"]:
            claimed.add(module)
            path = os.path.join(SUITE, module)
            if not os.path.isfile(path):
                problems.append(f"{row['feature']}: listed suite module missing: {module}")
                continue
            module_ok, module_err = count_module(path)
            ok += module_ok
            err += module_err
        counts = {
            "ok": ok,
            "err": err,
            "kani": count_kani(row["kani"]),
            "trybuild": count_trybuild(row["trybuild"]),
        }
        for cell in row["required"]:
            if counts[cell] == 0:
                problems.append(f"{row['feature']}: required cell '{cell}' is empty")
        table.append((row, counts))

    for fname in sorted(os.listdir(SUITE)):
        if fname.endswith(".rs") and fname not in UNCLAIMED_EXEMPT and fname not in claimed:
            problems.append(f"unclaimed suite module (add a feature-matrix.tsv row): {fname}")
    return table, problems


def render(table):
    header = f"{'feature':<22} {'ok':>4} {'err':>4} {'kani':>5} {'trybuild':>8}  required"
    lines = [header, "-" * len(header)]
    for row, counts in table:
        mark = lambda cell: f"{counts[cell]}{'*' if cell in row['required'] else ''}"
        lines.append(
            f"{row['feature']:<22} {mark('ok'):>4} {mark('err'):>4} "
            f"{mark('kani'):>5} {mark('trybuild'):>8}  {','.join(row['required'])}"
        )
    totals = {cell: sum(c[cell] for _, c in table) for cell in CELLS}
    lines.append("-" * len(header))
    lines.append(
        f"{'total':<22} {totals['ok']:>4} {totals['err']:>4} "
        f"{totals['kani']:>5} {totals['trybuild']:>8}  (* = required)"
    )
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail on empty required cells or manifest drift")
    args = parser.parse_args()

    table, problems = build(parse_manifest(MANIFEST))
    print(render(table))
    if problems:
        print("\nmatrix problems:", file=sys.stderr)
        for problem in problems:
            print(f"  {problem}", file=sys.stderr)
        if args.check:
            return 1
    elif args.check:
        print("\nfeature matrix check passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
