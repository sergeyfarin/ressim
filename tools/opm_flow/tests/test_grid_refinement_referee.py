"""Unit contracts for the grid-refinement referee.

Runs with Flow's system Python via uv; the artifact-only environment lacks numpy and skips.
"""
import unittest

try:
    from grid_refinement_referee import refinement_ratio, richardson
except ModuleNotFoundError:
    refinement_ratio = None
    richardson = None


@unittest.skipIf(richardson is None, "requires Flow's numpy environment")
class RichardsonTests(unittest.TestCase):
    def test_exact_finest_pair_passes(self):
        result = richardson([1.0, 1.5, 1.5], 2.0)
        self.assertTrue(result["passes"])
        self.assertEqual(result["contraction"], 0.0)
        self.assertEqual(result["fine_gap"], 0.0)
        self.assertIsNone(result["order"])

    def test_all_levels_equal_passes(self):
        self.assertTrue(richardson([1.0, 1.0, 1.0], 2.0)["passes"])

    def test_non_monotone_sequence_fails(self):
        self.assertFalse(richardson([1.0, 1.5, 1.4], 2.0)["passes"])

    def test_medium_equal_to_coarse_fails(self):
        self.assertFalse(richardson([1.0, 1.0, 1.5], 2.0)["passes"])

    def test_first_order_doubling_order_and_extrapolation(self):
        result = richardson([1.0, 2.0, 2.5], 2.0)
        self.assertTrue(result["passes"])
        self.assertAlmostEqual(result["order"], 1.0)
        self.assertAlmostEqual(result["extrapolated"], 3.0)

    def test_first_order_ratio_three_uses_actual_spacing(self):
        result = richardson([1.0, 2.0, 2.0 + 1.0 / 3.0], 3.0)
        self.assertTrue(result["passes"])
        self.assertAlmostEqual(result["order"], 1.0)
        self.assertAlmostEqual(result["extrapolated"], 2.5)

    def test_contraction_above_limit_fails(self):
        self.assertFalse(richardson([1.0, 2.0, 2.9], 2.0)["passes"])


@unittest.skipIf(refinement_ratio is None, "requires Flow's numpy environment")
class LevelLadderTests(unittest.TestCase):
    def test_doubling_ladder(self):
        self.assertEqual(refinement_ratio([10, 20, 40]), 2.0)

    def test_tripling_ladder(self):
        self.assertEqual(refinement_ratio([10, 30, 90]), 3.0)

    def test_repeated_level_is_rejected(self):
        with self.assertRaisesRegex(SystemExit, "strictly increasing"):
            refinement_ratio([10, 10, 10])

    def test_non_constant_ratio_is_rejected(self):
        with self.assertRaisesRegex(SystemExit, "constant refinement ratio"):
            refinement_ratio([10, 20, 30])

    def test_wrong_length_is_rejected(self):
        with self.assertRaisesRegex(SystemExit, "three grid levels"):
            refinement_ratio([10, 20])

    def test_non_positive_level_is_rejected(self):
        with self.assertRaisesRegex(SystemExit, "positive"):
            refinement_ratio([0, 10, 20])


if __name__ == "__main__":
    unittest.main()
