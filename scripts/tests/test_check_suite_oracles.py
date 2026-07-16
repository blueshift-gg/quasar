#!/usr/bin/env python3
import importlib.util
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "check-suite-oracles.py"
spec = importlib.util.spec_from_file_location("check_suite_oracles", SCRIPT)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)


def write_module(root: Path, name: str, body: str) -> None:
    (root / name).write_text(body)


class SuiteOraclePolicyTests(unittest.TestCase):
    def check(self, files: dict[str, str]) -> list[str]:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for name, body in files.items():
                write_module(root, name, body)
            return mod.violations(str(root))

    def test_bare_is_err_test_is_flagged(self) -> None:
        found = self.check({
            "weak.rs": """
#[test]
fn rejects_something() {
    let result = run();
    assert!(result.is_err(), "should fail");
}
"""
        })
        self.assertEqual(found, ["weak.rs::rejects_something"])

    def test_assert_error_satisfies_policy(self) -> None:
        found = self.check({
            "exact.rs": """
#[test]
fn rejects_something() {
    let result = run();
    assert!(result.is_err(), "should fail");
    result.assert_error(ProgramError::Custom(3005));
}
"""
        })
        self.assertEqual(found, [])

    def test_mollusk_failure_satisfies_policy(self) -> None:
        found = self.check({
            "mollusk.rs": """
#[test]
fn rejects_something() {
    let result = run();
    assert!(result.program_result.is_err());
    assert_eq!(result.program_result, ProgramResult::Failure(ProgramError::InvalidSeeds));
}
"""
        })
        self.assertEqual(found, [])

    def test_raw_instruction_error_satisfies_policy(self) -> None:
        found = self.check({
            "raw.rs": """
#[test]
fn rejects_something() {
    let result = run();
    assert!(result.raw_result.is_err());
    assert_eq!(result.raw_result, Err(InstructionError::ProgramFailedToComplete));
}
"""
        })
        self.assertEqual(found, [])

    def test_success_only_test_is_not_flagged(self) -> None:
        found = self.check({
            "happy.rs": """
#[test]
fn works() {
    let result = run();
    assert!(result.is_ok());
}
"""
        })
        self.assertEqual(found, [])

    def test_allowlisted_file_is_exempt(self) -> None:
        found = self.check({
            "optional_accounts.rs": """
#[test]
fn rejects_something() {
    assert!(run().is_err());
}
"""
        })
        self.assertEqual(found, [])


if __name__ == "__main__":
    unittest.main()
