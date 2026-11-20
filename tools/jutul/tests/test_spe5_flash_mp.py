"""Input, oracle and report contracts for the SPE5 line-search precision tool."""
import contextlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

try:
    import mpmath as mp
except ModuleNotFoundError:
    mp = None
else:
    import spe5_flash_mp


@unittest.skipIf(mp is None, "install mpmath to run the high-precision flash tool tests")
class LineSearchFlashTests(unittest.TestCase):
    def compare(self, data):
        with tempfile.TemporaryDirectory() as directory:
            src = Path(directory) / "input.json"
            dst = Path(directory) / "output.json"
            src.write_text(json.dumps(data))
            with contextlib.redirect_stdout(io.StringIO()):
                spe5_flash_mp.line_search_pairs(src, dst)
            return json.loads(dst.read_text())

    def test_rejects_unknown_schema(self):
        with self.assertRaisesRegex(ValueError, "schema"):
            self.compare({"schema": "other"})

    def test_rejects_incomplete_or_duplicate_pair_matrix(self):
        pairs = [{"cell": cell, "recovery": recovery} for cell in (33, 39) for recovery in (False, True)]
        for malformed in (pairs[:-1], [*pairs[:-1], pairs[0]]):
            with self.subTest(pairs=malformed), self.assertRaisesRegex(ValueError, "paired states"):
                self.compare({"schema": "ressim-spe5-line-search-flash/2", "pairs": malformed})

    def test_rejects_trivial_and_higher_gibbs_solutions(self):
        point = {"p_pa": 100, "z": [0.5, 0.5], "k": [1.2, 0.8], "beta": 0.5}
        split = {"beta": 0.5, "x": [0.45, 0.55], "y": [0.55, 0.45]}
        # The toy coefficients either favor the split or leave the homogeneous feed preferred.
        # This exercises the acceptance guard without depending on a particular PR root.
        for ln_k, favour_split in (([0.0, 0.0], True), ([0.2, -0.2], False)):
            def ln_phi(_eos, composition, _p, _phase):
                return ([-1, -1] if favour_split and composition[0] != 0.5 else [0, 0]), 1

            with (
                self.subTest(ln_k=ln_k, favour_split=favour_split),
                patch.object(spe5_flash_mp, "solve", return_value=([*ln_k, 0.5], split, 0, 0)),
                patch.object(spe5_flash_mp, "ln_phi", side_effect=ln_phi),
                self.assertRaisesRegex(ValueError, "non-trivial lower-Gibbs split"),
            ):
                spe5_flash_mp.line_search_split(None, point)

    def test_reports_point_errors_and_both_refinement_outcomes(self):
        current = {"p_pa": 100, "beta": 0.375, "k": [1, 1], "iterations": 3,
                   "equilibrium_residual": 1e-12, "refinement_outcome": "move_cap",
                   "unrefined_beta": 0.375, "wide_cap_beta": 0.25, "wide_cap_outcome": "kept"}
        trial = {**current, "p_pa": 101, "beta": 0.5, "unrefined_beta": 0.625,
                 "wide_cap_beta": 0.5, "refinement_outcome": "kept"}
        pairs = [{"cell": cell, "recovery": recovery, "temperature_k": 300,
                  "current": current, "trial": trial}
                 for cell in (33, 39) for recovery in (False, True)]

        def solved(_eos, point, _initial=None):
            return [mp.mpf(0), mp.mpf(0), mp.mpf("0.25" if point["p_pa"] == 100 else "0.5")], mp.mpf(0)

        with (
            patch.object(spe5_flash_mp, "setup"),
            patch.object(spe5_flash_mp, "line_search_split", side_effect=solved),
        ):
            out = self.compare({"schema": "ressim-spe5-line-search-flash/2", "pairs": pairs,
                                "fluid": {}, "tau": 1e-6, "dt_days": 0.044,
                                "wide_refinement_move": 0.1})
        self.assertEqual(len(out["pairs"]), 4)
        row = out["pairs"][0]
        self.assertEqual(row["float_abs_beta_errors"], [0.125, 0])
        self.assertEqual(row["unrefined_abs_beta_errors"], [0.125, 0.125])
        self.assertEqual(row["wide_cap_abs_beta_errors"], [0, 0])
        self.assertEqual(row["wide_cap_delta_beta"], 0.25)
        self.assertEqual(row["refinement_outcomes"], ["move_cap", "kept"])
        self.assertEqual(row["wide_cap_outcomes"], ["kept", "kept"])
