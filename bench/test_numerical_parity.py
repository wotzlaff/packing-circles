"""Check the numerical comparison gate independently of installed probe binaries."""

import unittest

from check_numerical_parity import compare, finite_difference_error, packet


class NumericalParityTests(unittest.TestCase):
    def test_gate_rejects_trajectory_and_flag_changes(self):
        with self.assertRaises(ValueError):
            compare({"trace":[1., 2.]}, {"trace":[1.]})
        with self.assertRaises(ValueError):
            compare({"refresh":False}, {"refresh":0})
        with self.assertRaises(ValueError):
            compare({"gradient":[1., 2.]}, {"gradient":[1., 2.01]})
        with self.assertRaises(ValueError):
            compare(float("nan"), 0.)
        self.assertLess(compare([1., 2.], [1.+1e-10, 2.]), 1.)

    def test_independent_gradient_gate_detects_wrong_derivative(self):
        radii, state, weights = [1.], [2., 2., 0.], [0.1, 0.2]
        # f=R+.1*(x²-(R-r)²)²; derivatives (-.2,2.4,0).
        self.assertLess(finite_difference_error(radii,state,weights,[-.2,2.4,0.]), 1e-5)
        self.assertGreater(finite_difference_error(radii,state,weights,[1.,0.,0.]), .1)

    def test_common_packet_has_unambiguous_parameter_positions(self):
        tokens = packet([1.,2.],[3.,-2.,0.,1.,0.],[1e-7,1e-5],previous=0.).split()
        self.assertEqual(len(tokens), 24)
        self.assertEqual(tokens[15:17], ["1","0.0"])


if __name__ == "__main__":
    unittest.main()
