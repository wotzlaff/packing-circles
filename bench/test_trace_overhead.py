import unittest

from pathlib import Path
from unittest.mock import patch

from measure_trace_overhead import COUNTERS, _run, analyze_pairs, validate_run_summary


def row(instance="a", repetition=0):
    summary = {key: ("evaluation_limit" if key == "termination" else 10) for key in COUNTERS}
    summary.update(schema_version=1, seed=5 + repetition, algorithm="a", time_limit_seconds=None,
                   evaluation_limit=10, elapsed_seconds=1.1, radius=3.0, output_radius=3.0,
                   iteration_limit=1000000, restart_limit=10000)
    plain = {"stdout": "solution", "summary": dict(summary), "returncode": 0, "elapsed_seconds": 1.0}
    traced = {"stdout": "solution", "summary": dict(summary), "returncode": 0, "elapsed_seconds": 1.01}
    return {"instance": instance, "repetition": repetition, "seed": 5 + repetition,
            "plain": plain, "traced": traced,
            "plain_validation": {"feasible": True}, "traced_validation": {"feasible": True},
            "trace_replay": {"valid": True, "seed": 5 + repetition,
                             "algorithm": "a",
                             "options": {"time_limit_seconds": None, "evaluation_limit": 10,
                                         "iteration_limit": 1000000, "restart_limit": 10000},
                             "evaluations": 10, "iterations": 10, "restarts": 10,
                             "final_radius": 3.0, "exported_radius": 3.0, "elapsed_seconds": 1.0}}


class TraceOverheadAnalysisTests(unittest.TestCase):
    def test_accepts_complete_identical_pairs(self):
        audit = analyze_pairs([row("a", 0), row("a", 1), row("b", 0), row("b", 1)], {"a", "b"}, 2)
        self.assertTrue(audit["valid"])

    def test_detects_changed_output_or_work_counts(self):
        output_changed = row()
        output_changed["traced"]["stdout"] = "different"
        counts_changed = row("b")
        counts_changed["traced"]["summary"]["evaluations"] = 11
        audit = analyze_pairs([output_changed, counts_changed], {"a", "b"}, 1)
        self.assertEqual(audit["changed_outputs"], [("a", 0)])
        self.assertEqual(audit["changed_counters"], [("b", 0)])
        self.assertFalse(audit["valid"])

    def test_detects_invalid_pair_and_missing_cohort(self):
        invalid = row("a")
        invalid["traced_validation"]["feasible"] = False
        audit = analyze_pairs([invalid], {"a", "b"}, 1)
        self.assertFalse(audit["complete"])
        self.assertEqual(audit["invalid_pairs"], [("a", 0)])
        self.assertFalse(audit["valid"])

    def test_rejects_missing_trace_return_code_and_bad_numeric_fields(self):
        missing_trace = row()
        missing_trace["trace_replay"]["valid"] = False
        bad_status = row("b")
        bad_status["traced"]["returncode"] = 2
        bad_status["plain"]["summary"]["evaluations"] = True
        audit = analyze_pairs([missing_trace, bad_status], {"a", "b"}, 1)
        self.assertEqual(audit["invalid_pairs"], [("a", 0)])
        self.assertTrue(audit["bad_status"])
        self.assertFalse(audit["valid"])

    def test_checks_trace_summary_linkage(self):
        sample = row()
        validate_run_summary(sample["trace_replay"], sample["traced"]["summary"])
        sample["trace_replay"]["options"]["restart_limit"] += 1
        with self.assertRaisesRegex(ValueError, "restart_limit"):
            validate_run_summary(sample["trace_replay"], sample["traced"]["summary"])

    def test_timeout_preserves_partial_process_output(self):
        timeout = __import__("subprocess").TimeoutExpired(["solver"], 30, output=b"partial out", stderr=b"partial err")
        with patch("measure_trace_overhead.subprocess.run", side_effect=timeout):
            result = _run(Path("solver"), "input", 5, 50000, None)
        self.assertTrue(result["timeout"])
        self.assertIsNone(result["returncode"])
        self.assertEqual(result["stdout"], "partial out")
        self.assertEqual(result["stderr"], "partial err")


if __name__ == "__main__":
    unittest.main()
