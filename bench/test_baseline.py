"""Checks for benchmark provenance, budget placement, and invalid-run handling."""

import json
from pathlib import Path
import tempfile
import unittest

from generate_instances import expected_files, radius_stream
from legacy import command_for
from run_baseline import check_output, summarize


class BaselineTests(unittest.TestCase):
    def test_frozen_splits_are_disjoint_and_in_range(self):
        files = expected_files()
        instances = [v for k, v in files.items() if k.endswith(".txt")]
        self.assertEqual(len(instances), 100)
        self.assertEqual(len(set(instances)), 100)
        for content in instances:
            values = list(map(int, content.split()))
            self.assertEqual(values[0], len(values) - 1)
            self.assertTrue(all(20 <= r <= 99 for r in values[1:]))
        self.assertEqual(radius_stream("seed", 25), radius_stream("seed", 25))

    def test_budget_occupies_each_variants_actual_argument(self):
        self.assertEqual(command_for("contest", 0.5)[6], "0.5")
        self.assertEqual(command_for("main", 0.5)[5], "0.5")
        for budget in (0, -1, float("nan"), float("inf")):
            with self.assertRaises(ValueError):
                command_for("contest", budget)

    def test_malformed_or_invalid_output_cannot_count_as_valid(self):
        self.assertFalse(check_output("nan", [1])["feasible"])
        self.assertFalse(check_output("1\n1 2 0", [1])["feasible"])
        self.assertTrue(check_output("1\n1 0 0", [1])["feasible"])

    def test_summary_excludes_invalid_radius_from_quality(self):
        common = {"variant": "contest", "budget_seconds": 0.1, "circle_count": 10,
                  "elapsed_seconds": 0.1, "returncode": 0, "timeout": False, "overrun": False}
        rows = [{**common, "successful": True, "validation": {"area_bound_ratio": 1.25}},
                {**common, "successful": False, "validation": {"area_bound_ratio": 0.1}}]
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "results.jsonl"
            path.write_text("\n".join(json.dumps(r) for r in rows))
            output = summarize(path)
        self.assertIn("1.250000", output)
        self.assertNotIn("0.675000", output)


if __name__ == "__main__":
    unittest.main()
