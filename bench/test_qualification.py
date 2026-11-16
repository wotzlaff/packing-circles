"""Synthetic tests for version A qualification gates and row preservation."""

import json
from pathlib import Path
import tempfile
import unittest

import qualify_version_a as qualification


class QualificationTests(unittest.TestCase):
    def setUp(self):
        self.input_hashes = {"development/a.txt": "a" * 64,
                             "development/b.txt": "b" * 64}
        self.legacy = []
        for instance in ("development/a.txt", "development/b.txt"):
            for budget in (0.1, 0.5, 1.95):
                for repetition in range(5):
                    self.legacy.append({"instance": instance, "budget_seconds": budget,
                                        "repetition": repetition,
                                        "input_sha256": self.input_hashes[instance],
                                        "validation": {"enclosing_radius": 100.0}})

    @staticmethod
    def rust_row(instance, budget, repetition, radius=100.0, **changes):
        row = {"schema_version": 1, "variant": "rust-a", "instance": instance,
               "input_sha256": "a" * 64 if instance.endswith("a.txt") else "b" * 64,
               "budget_seconds": budget, "repetition": repetition, "seed": 5 + repetition,
               "elapsed_seconds": budget, "returncode": 0, "timeout": False, "overrun": False,
               "summary": {"schema_version": 1, "algorithm": "a", "seed": 5 + repetition,
                           "time_limit_seconds": budget, "evaluations": 100, "iterations": 10,
                           "restarts": 1, "elapsed_seconds": budget, "termination": "time_limit",
                           "radius": radius, "radius_internal": radius, "output_radius": radius,
                           "feasible": True, "output_feasible": True},
               "validation": {"feasible": True, "enclosing_radius": radius}}
        row.update(changes)
        return row

    def full_rows(self):
        return [self.rust_row(legacy["instance"], legacy["budget_seconds"],
                              legacy["repetition"])
                for legacy in self.legacy]

    def test_median_is_per_instance_before_geometric_mean(self):
        rows = self.full_rows()
        # Instance A: median ratio 1.0; instance B: median ratio 1.21.
        a = [r for r in rows if r["instance"].endswith("a.txt") and r["budget_seconds"] == 1.95]
        b = [r for r in rows if r["instance"].endswith("b.txt") and r["budget_seconds"] == 1.95]
        for row, radius in zip(a, [50, 100, 100, 100, 200]):
            row["summary"]["radius_internal"] = radius
            row["summary"]["radius"] = radius
            row["summary"]["output_radius"] = radius
            row["validation"]["enclosing_radius"] = radius
        for row in b:
            row["summary"]["radius_internal"] = 121
            row["summary"]["radius"] = 121
            row["summary"]["output_radius"] = 121
            row["validation"]["enclosing_radius"] = 121
        report = qualification.analyze(rows, self.legacy)
        observed = report["budgets"]["1.95"]["geometric_mean"]
        self.assertAlmostEqual(observed, (1.0 * 1.21) ** 0.5)
        self.assertEqual(report["budgets"]["1.95"]["worst"], 1.21)

    def test_complete_quality_gate_uses_geometric_mean_and_worst(self):
        rows = self.full_rows()
        report = qualification.analyze(rows, self.legacy)
        self.assertEqual(report["status"], "QUALIFIED")
        self.assertTrue(report["budgets"]["1.95"]["qualified"])
        # Ratios 0.9 and 1.10 would have pooled mean 1.0, but worst fails.
        for row in rows:
            if row["budget_seconds"] == 1.95 and row["instance"].endswith("a.txt"):
                row["validation"]["enclosing_radius"] = 90
                row["summary"]["radius_internal"] = 90
                row["summary"]["radius"] = 90
                row["summary"]["output_radius"] = 90
            elif row["budget_seconds"] == 1.95:
                row["validation"]["enclosing_radius"] = 110
                row["summary"]["radius_internal"] = 110
                row["summary"]["radius"] = 110
                row["summary"]["output_radius"] = 110
        report = qualification.analyze(rows, self.legacy)
        self.assertEqual(report["status"], "INCOMPLETE_OR_FAILED")

    def test_invalid_and_incomplete_rows_cannot_pass(self):
        rows = self.full_rows()
        rows[0]["summary"]["feasible"] = False
        report = qualification.analyze(rows, self.legacy)
        self.assertFalse(report["all_outputs_valid"])
        self.assertEqual(report["status"], "INCOMPLETE_OR_FAILED")
        rows = self.full_rows()[:-1]
        report = qualification.analyze(rows, self.legacy)
        self.assertFalse(report["complete"])
        self.assertEqual(report["status"], "INCOMPLETE_OR_FAILED")

    def test_malformed_summary_and_mismatched_run_provenance_cannot_count(self):
        for mutate in (
            lambda row: row["summary"].update(evaluations=True),
            lambda row: row["summary"].update(seed=row["seed"] + 1),
            lambda row: row["summary"].update(time_limit_seconds=2.0),
            lambda row: row["summary"].update(output_radius=101.0),
            lambda row: row.update(input_sha256="0" * 64),
            lambda row: row.update(variant="rust-b"),
        ):
            rows = self.full_rows()
            mutate(rows[0])
            report = qualification.analyze(rows, self.legacy)
            self.assertFalse(report["all_outputs_valid"])
            self.assertEqual(report["status"], "INCOMPLETE_OR_FAILED")
    def test_invalid_summary_json_is_retained_as_row_error(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "rows.jsonl"
            path.write_text('{"x": 1}\nnot json\n{"x": NaN}\n', encoding="utf-8")
            rows = qualification.read_jsonl(path)
        self.assertEqual(rows[0], {"x": 1})
        self.assertIn("_row_error", rows[1])
        self.assertIn("_row_error", rows[2])
        valid = self.full_rows()[0]["summary"]
        parsed, error = qualification._summary_from_stderr(json.dumps(valid))
        self.assertIsNone(error)
        self.assertEqual(parsed["radius_internal"], valid["radius"])
        parsed, error = qualification._summary_from_stderr("{\"evaluations\": 1}")
        self.assertIsNone(parsed)
        self.assertIn("missing required", error)
        parsed, error = qualification._summary_from_stderr("{\"elapsed_seconds\": NaN}")
        self.assertIsNone(parsed)
        self.assertIn("invalid summary", error)


if __name__ == "__main__":
    unittest.main()
