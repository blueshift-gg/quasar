#!/usr/bin/env python3
import importlib.util
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "test-matrix.py"
spec = importlib.util.spec_from_file_location("test_matrix", SCRIPT)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)

POSITIVE = """
#[test]
fn writes_state() {
    assert_eq!(decode(), 42);
}
"""
NEGATIVE = """
#[test]
fn rejects_bad_input() {
    result.assert_error(ProgramError::InvalidAccountData);
}
"""


class MatrixTests(unittest.TestCase):
    def with_suite(self, files: dict[str, str]):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        root = Path(tmp.name)
        (root / "suite").mkdir()
        for name, body in files.items():
            (root / "suite" / name).write_text(body)
        self.old_suite = mod.SUITE
        mod.SUITE = str(root / "suite")
        self.addCleanup(lambda: setattr(mod, "SUITE", self.old_suite))
        return root

    def row(self, **overrides):
        row = {
            "feature": "demo",
            "modules": ["demo.rs"],
            "kani": [],
            "trybuild": [],
            "required": ["ok", "err"],
            "notes": "",
        }
        row.update(overrides)
        return row

    def test_counts_positives_and_exact_negatives(self) -> None:
        self.with_suite({"demo.rs": POSITIVE + NEGATIVE})
        table, problems = mod.build([self.row()])
        self.assertEqual(problems, [])
        self.assertEqual(table[0][1]["ok"], 1)
        self.assertEqual(table[0][1]["err"], 1)

    def test_empty_required_cell_is_a_problem(self) -> None:
        self.with_suite({"demo.rs": POSITIVE})  # no exact-error test
        _, problems = mod.build([self.row()])
        self.assertEqual(problems, ["demo: required cell 'err' is empty"])

    def test_missing_module_is_a_problem(self) -> None:
        self.with_suite({})
        _, problems = mod.build([self.row(required=["ok"])])
        self.assertIn("demo: listed suite module missing: demo.rs", problems)

    def test_unclaimed_module_is_a_problem(self) -> None:
        self.with_suite({"demo.rs": POSITIVE + NEGATIVE, "orphan.rs": POSITIVE})
        _, problems = mod.build([self.row()])
        self.assertEqual(
            problems,
            ["unclaimed suite module (add a feature-matrix.tsv row): orphan.rs"],
        )

    def test_helpers_and_lib_are_exempt_from_claiming(self) -> None:
        self.with_suite({"demo.rs": POSITIVE + NEGATIVE, "lib.rs": "", "helpers.rs": ""})
        _, problems = mod.build([self.row()])
        self.assertEqual(problems, [])


if __name__ == "__main__":
    unittest.main()
