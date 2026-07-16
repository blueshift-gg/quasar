#!/usr/bin/env python3
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "bench-tracked-programs.sh"
BASELINE = ROOT / "benchmarks" / "v0.1.0.env"


def read_metrics() -> dict[str, int]:
    metrics = {}
    for line in BASELINE.read_text().splitlines():
        if not line or line.startswith("#"):
            continue
        key, value = line.split("=", 1)
        metrics[key] = int(value)
    return metrics


class BenchmarkPolicyTests(unittest.TestCase):
    def compare(self, candidate_lines: list[str]) -> subprocess.CompletedProcess[str]:
        with tempfile.NamedTemporaryFile("w", delete=False) as candidate:
            candidate.write("\n".join(candidate_lines) + "\n")
            candidate_path = Path(candidate.name)
        try:
            return subprocess.run(
                ["bash", str(SCRIPT), "compare-files", str(BASELINE), str(candidate_path)],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
        finally:
            candidate_path.unlink()

    def test_exact_baseline_passes(self) -> None:
        metrics = read_metrics()
        result = self.compare([f"{key}={value}" for key, value in metrics.items()])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("baseline=1556", result.stdout)
        self.assertIn("candidate=1556", result.stdout)

    def test_improvement_passes(self) -> None:
        metrics = read_metrics()
        metrics["VAULT_DEPOSIT_CU"] -= 1
        result = self.compare([f"{key}={value}" for key, value in metrics.items()])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("delta=-1", result.stdout)

    def test_any_regression_fails(self) -> None:
        metrics = read_metrics()
        metrics["MULTISIG_SIZE"] += 1
        result = self.compare([f"{key}={value}" for key, value in metrics.items()])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("delta=+1", result.stdout)
        self.assertIn("tracked metric regression detected", result.stderr)

    def test_missing_metric_fails(self) -> None:
        metrics = read_metrics()
        metrics.pop("ESCROW_REFUND_CU")
        result = self.compare([f"{key}={value}" for key, value in metrics.items()])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing tracked metric: ESCROW_REFUND_CU", result.stderr)

    def test_unknown_metric_fails(self) -> None:
        metrics = read_metrics()
        lines = [f"{key}={value}" for key, value in metrics.items()]
        lines.append("UNREVIEWED_ALLOWANCE=1")
        result = self.compare(lines)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unknown tracked metric: UNREVIEWED_ALLOWANCE", result.stderr)

    def test_duplicate_metric_fails(self) -> None:
        metrics = read_metrics()
        lines = [f"{key}={value}" for key, value in metrics.items()]
        lines.append(f"VAULT_DEPOSIT_CU={metrics['VAULT_DEPOSIT_CU']}")
        result = self.compare(lines)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("duplicate tracked metric: VAULT_DEPOSIT_CU", result.stderr)

    def test_non_numeric_metric_fails(self) -> None:
        metrics = read_metrics()
        lines = [f"{key}={value}" for key, value in metrics.items()]
        lines[0] = "VAULT_DEPOSIT_CU=unbounded"
        result = self.compare(lines)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("non-numeric value for VAULT_DEPOSIT_CU: unbounded", result.stderr)


class ReadCuTests(unittest.TestCase):
    """The capture path reads CU records written by examples/cu_bench.rs as
    JSON lines under target/cu-bench/ — never from test stdout."""

    def read_cu(self, lines: list[str], instruction: str) -> subprocess.CompletedProcess[str]:
        with tempfile.NamedTemporaryFile("w", suffix=".jsonl", delete=False) as record:
            record.write("\n".join(lines) + "\n")
            record_path = Path(record.name)
        try:
            return subprocess.run(
                ["bash", str(SCRIPT), "read-cu", str(record_path), instruction],
                cwd=ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
        finally:
            record_path.unlink()

    def test_reads_recorded_instruction(self) -> None:
        result = self.read_cu(
            [
                '{"instruction":"deposit","cu":1556}',
                '{"instruction":"withdraw","cu":392}',
            ],
            "withdraw",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), "392")

    def test_missing_instruction_fails(self) -> None:
        result = self.read_cu(['{"instruction":"deposit","cu":1556}'], "withdraw")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing or non-numeric cu record", result.stderr)

    def test_missing_record_file_fails(self) -> None:
        result = subprocess.run(
            ["bash", str(SCRIPT), "read-cu", "/nonexistent/cu.jsonl", "deposit"],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing cu-bench record file", result.stderr)

    def test_non_numeric_record_fails(self) -> None:
        result = self.read_cu(['{"instruction":"deposit","cu":"lots"}'], "deposit")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing or non-numeric cu record", result.stderr)


if __name__ == "__main__":
    unittest.main()
