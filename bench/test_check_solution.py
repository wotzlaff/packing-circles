import unittest

from check_solution import parse_instance, parse_solution, validate_solution


class ParseInstanceTests(unittest.TestCase):
    def test_parses_contest_instance(self):
        self.assertEqual(parse_instance("2\n1 2\n"), [1.0, 2.0])

    def test_rejects_missing_radius(self):
        with self.assertRaisesRegex(ValueError, "expected 2 radii"):
            parse_instance("2\n1\n")

    def test_rejects_nonpositive_or_nonfinite_radius(self):
        for text in ("1 0", "1 -2", "1 nan", "1 inf", "1 -inf"):
            with self.subTest(text=text), self.assertRaises(ValueError):
                parse_instance(text)


class SolutionCheckerTests(unittest.TestCase):
    def test_tangent_circles_are_feasible(self):
        radii = [1.0, 1.0]
        solution = parse_solution("2.0000\n1 -1 0\n1 1 0\n", radii)
        report = validate_solution(radii, solution)
        self.assertTrue(report["feasible"])
        self.assertEqual(report["max_violation"], 0.0)

    def test_rejects_missing_circle(self):
        with self.assertRaisesRegex(ValueError, "expected enclosing radius"):
            parse_solution("2\n1 0 0\n", [1, 1])

    def test_rejects_wrong_reported_circle_radius(self):
        solution = parse_solution("1\n2 0 0\n", [1])
        report = validate_solution([1], solution)
        self.assertFalse(report["radii_match"])
        self.assertFalse(report["feasible"])

    def test_reports_pairwise_overlap(self):
        radii = [1, 1]
        solution = parse_solution("2\n1 -0.9 0\n1 0.9 0\n", radii)
        report = validate_solution(radii, solution)
        self.assertFalse(report["feasible"])
        self.assertAlmostEqual(report["max_separation_violation"], 0.2)

    def test_reports_containment_violation(self):
        solution = parse_solution("1\n1 0.1 0\n", [1])
        report = validate_solution([1], solution)
        self.assertFalse(report["feasible"])
        self.assertAlmostEqual(report["max_containment_violation"], 0.1)

    def test_nonfinite_solution_values_are_rejected(self):
        for text in ("1\n1 nan 0\n", "1\n1 0 inf\n", "nan\n1 0 0\n"):
            with self.subTest(text=text), self.assertRaises(ValueError):
                parse_solution(text, [1])

    def test_rounding_allowance_is_distinct_from_internal_feasibility(self):
        radii = [1, 1]
        solution = parse_solution("2\n1 -0.99995 0\n1 0.99995 0\n", radii)
        report = validate_solution(radii, solution)
        self.assertFalse(report["feasible"])
        self.assertTrue(report["rounding_feasible"])
        self.assertGreater(report["max_separation_violation"], report["internal_tolerance"])
        self.assertIn("separation", report["rounding_allowances"])

    def test_count_mismatch_in_direct_validation_is_reported(self):
        report = validate_solution([1, 1], {"radius": 2, "circles": [(1, 0, 0)]})
        self.assertFalse(report["count_matches"])
        self.assertFalse(report["feasible"])


if __name__ == "__main__":
    unittest.main()
