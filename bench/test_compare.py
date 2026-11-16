import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import compare


class CompareTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.input = self.directory / "tiny.txt"
        self.input.write_text("1\n1\n", encoding="utf-8")
        self.exe = self.directory / "solver"
        self.exe.write_text("mock executable", encoding="utf-8")

    def config(self, **updates):
        value = {"schema_version": 1, "instance_paths": [str(self.input)],
                 "budgets_seconds": [0.1], "seeds": [5], "repetitions": 1,
                 "solvers": [{"name": "a", "kind": "rust", "algorithm": "a",
                              "executable": str(self.exe)}]}
        value.update(updates)
        return value

    def test_config_rejects_duplicate_names_and_invalid_limits(self):
        solvers = [{"name": "same", "kind": "rust", "algorithm": "a", "executable": str(self.exe)}] * 2
        with self.assertRaisesRegex(ValueError, "unique"):
            compare.validate_config(self.config(solvers=solvers), self.input)
        with self.assertRaisesRegex(ValueError, "canonical"):
            alias = self.input.parent / "sub" / ".." / "tiny.txt"
            compare.validate_config(self.config(instance_paths=[str(self.input), str(alias)]), self.input)
        with self.assertRaisesRegex(ValueError, "budgets_seconds must be unique"):
            compare.validate_config(self.config(budgets_seconds=[0.1, 0.1]), self.input)
        with self.assertRaisesRegex(ValueError, "positive finite"):
            compare.validate_config(self.config(budgets_seconds=[float("inf")]), self.input)
        with self.assertRaisesRegex(ValueError, "u64"):
            compare.validate_config(self.config(seeds=[-1]), self.input)
        with self.assertRaisesRegex(ValueError, "positive integer"):
            compare.validate_config(self.config(repetitions=True), self.input)

    def test_commands_keep_legacy_and_rust_arguments_scoped(self):
        rust = self.config()["solvers"][0]
        cmd = compare._command(rust, 0.1, 5, self.input)
        self.assertEqual(cmd[1:4], ["solve", "-", "--algorithm"])
        self.assertIn("a", cmd)
        legacy = {"name": "legacy", "kind": "legacy", "legacy_variant": "main",
                  "executable": str(self.exe)}
        cmd = compare._command(legacy, 0.5, 7, self.input)
        self.assertEqual(cmd[0], str(self.exe.resolve()))
        self.assertNotIn("--algorithm", cmd)

    def test_failed_and_overrun_rows_are_excluded_from_quality(self):
        base = {"solver": "a", "kind": "rust", "successful": True, "timeout": False,
                "returncode": 0, "overrun": False, "validation": {"feasible": True,
                "enclosing_radius": 1.0}, "algorithm": "a", "seed": 5,
                "budget_seconds": 0.1, "summary": {"schema_version": 1, "algorithm": "a",
                "seed": 5, "evaluations": 1, "iterations": 1, "restarts": 0,
                "radius": 1.0, "output_radius": 1.0, "elapsed_seconds": 0.08,
                "time_limit_seconds": 0.1, "termination": "time_limit",
                "feasible": True, "output_feasible": True}}
        failed = dict(base, successful=False)
        late = dict(base, overrun=True)
        self.assertTrue(compare._valid(base))
        self.assertFalse(compare._valid(failed))
        self.assertFalse(compare._valid(late))

    def test_b_does_not_accept_a_summary_and_trace_errors_fail(self):
        summary, error = compare._summary(json.dumps({"schema_version": 1, "algorithm": "a"}), "b", 5, 0.1)
        self.assertIsNone(summary)
        self.assertTrue(error)
        invalid_summary, error = compare._summary(json.dumps({"schema_version": 1, "algorithm": "a",
            "seed": 5, "evaluations": 1, "iterations": 1, "restarts": 0,
            "radius": -1, "output_radius": 1, "elapsed_seconds": 0.01,
            "time_limit_seconds": 0.1, "termination": "unknown",
            "feasible": True, "output_feasible": True}), "a", 5, 0.1)
        self.assertIsNone(invalid_summary)
        self.assertTrue(error)
        row = {"solver": "b", "kind": "rust", "successful": True, "timeout": False,
               "returncode": 0, "overrun": False, "validation": {"feasible": True},
               "summary": {"feasible": True, "output_feasible": True}, "trace_error": "trace unavailable"}
        self.assertFalse(compare._valid(row))

    def test_summary_reproduces_from_raw_rows(self):
        rows = []
        for name, radius in (("base", 2.0), ("candidate", 1.0)):
            rows.append({"solver": name, "budget_seconds": 0.1, "kind": "legacy",
                         "successful": True, "timeout": False, "returncode": 0,
                         "overrun": False, "validation": {"feasible": True,
                         "enclosing_radius": radius}, "instance": "one.txt", "circle_count": 1,
                         "elapsed_seconds": 0.08})
        raw = self.directory / "results.jsonl"
        raw.write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")
        report = compare.summarize(raw)
        self.assertIn("| base | 1 | 0.1 | 1 | 1 |", report)
        self.assertIn("| candidate | 1 | 0.50000000 |", report)

    def test_failed_executable_and_nonfinite_extra_summary_are_preserved_as_failures(self):
        with patch("compare.subprocess.run",side_effect=PermissionError("cannot execute")):
            stdout,stderr,status,timeout,elapsed=compare._run([str(self.exe)],"1\n1\n",.1)
        self.assertEqual(stdout,"")
        self.assertIn("cannot execute",stderr)
        self.assertIsNone(status)
        self.assertFalse(timeout)
        self.assertGreaterEqual(elapsed,0.)
        summary,error=compare._summary('{"extra":1e999}',"a",5,.1)
        self.assertIsNone(summary)
        self.assertIn("nonfinite",error)

    def test_missing_or_failed_repetitions_cannot_count_as_improvements(self):
        rows=[]
        for name,radius in (("base",2.),("candidate",1.)):
            for repetition in range(2):
                rows.append({"solver":name,"kind":"legacy","instance":"one.txt",
                    "budget_seconds":.1,"circle_count":1,"expected_repetitions":2,
                    "repetition":repetition,"returncode":0,"timeout":False,"overrun":False,
                    "elapsed_seconds":.08,"successful":True,
                    "validation":{"feasible":True,"enclosing_radius":radius}})
        rows[-1]["successful"]=False
        raw=self.directory/"failed-cohort.jsonl"
        raw.write_text("".join(json.dumps(row)+"\n" for row in rows))
        self.assertIn("| candidate | 0 | — |",compare.summarize(raw))
        rows.pop()
        raw.write_text("".join(json.dumps(row)+"\n" for row in rows))
        self.assertIn("| candidate | 0 | — |",compare.summarize(raw))


if __name__ == "__main__":
    unittest.main()
