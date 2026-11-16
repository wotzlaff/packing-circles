import json
from pathlib import Path
import tempfile
import unittest

from replay_trace import replay_trace


def valid_rows():
    header = {"schema_version": 1, "type": "header", "algorithm": "a", "run_id": "r1",
              "instance": {"radii": [1, 1]}, "seed": 5,
              "options": {"time_limit_seconds": None, "evaluation_limit": 50000,
                          "iteration_limit": 1000000, "restart_limit": 10000}}
    def snap(kind, seq, t, radius=3.0, points=[[1, 0], [-1, 0]]):
        return {"schema_version": 1, "type": "snapshot", "run_id": "r1", "kind": kind,
                "sequence": seq, "elapsed_seconds": t, "evaluations": seq,
                "iterations": seq, "restarts": 0, "radius": radius, "points": points,
                "max_violation": 0.0, "feasible": True,
                "output": "3.0000\n1 1.0000 0.0000\n1 -1.0000 0.0000\n"}
    return [header, snap("initial", 0, 0), snap("incumbent", 1, 0.1), snap("final", 2, 0.11)]


class ReplayTraceTests(unittest.TestCase):
    def test_ceil_uses_the_same_float_value_as_the_native_formatter(self):
        rows=valid_rows()
        for row in rows[1:]:
            row['radius']=3.0000000000000004
            row['output']='3.0001\n1 1.0000 0.0000\n1 -1.0000 0.0000\n'
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'float-boundary.jsonl'
            path.write_text(''.join(json.dumps(row)+'\n' for row in rows))
            self.assertTrue(replay_trace(path)['valid'])

    def check_bad(self, mutate, text=None):
        rows = valid_rows()
        mutate(rows)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "trace.jsonl"
            if text is None:
                path.write_text("".join(json.dumps(row) + "\n" for row in rows))
            else:
                path.write_text(text)
            with self.assertRaises(ValueError):
                replay_trace(path)

    def test_valid_trace_and_expected_final_text(self):
        rows = valid_rows()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "trace.jsonl"
            path.write_text("".join(json.dumps(row) + "\n" for row in rows))
            self.assertEqual(replay_trace(path, rows[-1]["output"])["snapshots"], 3)
            with self.assertRaisesRegex(ValueError, "exactly match"):
                replay_trace(path, "different")

    def test_rejects_bad_order_ids_geometry_and_work(self):
        self.check_bad(lambda rows: rows.__setitem__(1, dict(rows[1], kind="final")))
        self.check_bad(lambda rows: rows[2].update(run_id="other"))
        self.check_bad(lambda rows: rows[2].update(points=[[0, 0], [0, 0]]))
        self.check_bad(lambda rows: rows[3].update(evaluations=0))
        self.check_bad(lambda rows: rows[2].update(max_violation=1))
        self.check_bad(lambda rows: rows[2].update(radius=3.1))
        self.check_bad(lambda rows: rows[2].update(elapsed_seconds=0.05))

    def test_rejects_nonfinite_and_rows_after_final(self):
        self.check_bad(lambda rows: None, '{"x":1e999}\n')
        self.check_bad(lambda rows: rows.append(dict(rows[-1], sequence=3)))

    def test_rejects_final_radius_increase_and_downward_radius_rounding(self):
        self.check_bad(lambda rows: rows[3].update(radius=3.1, output=rows[3]["output"].replace("3.0000", "3.1000", 1)))
        self.check_bad(lambda rows: rows[2].update(radius=3.00001))

    def test_rejects_rounding_allowance_overlap_in_export(self):
        self.check_bad(lambda rows: rows[2].update(output=rows[2]["output"].replace("1.0000", "0.9999", 1)))


if __name__ == "__main__":
    unittest.main()
