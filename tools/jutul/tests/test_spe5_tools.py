"""Regression tests for the #116 SPE5 tools (stdlib only).

    python3 -I -m unittest discover -s tools/jutul/tests
"""
import contextlib
import copy
import io
import json
import os
import sys
import tempfile
import unittest

TOOLS = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(0, TOOLS)

import spe5_compare  # noqa: E402
import spe5_cross_flash_fixture as fixture_tool  # noqa: E402

ROOT = os.path.abspath(os.path.join(TOOLS, "..", ".."))
FIXTURE = os.path.join(ROOT, "opm", "compositional", "spe5", "cross_flash_205_216.json")


class CrossFlashMatrixTests(unittest.TestCase):
    def setUp(self):
        with open(FIXTURE) as f:
            self.points, self.ressim, self.jutul = fixture_tool.split_fixture(json.load(f))

    def test_committed_fixture_is_valid(self):
        self.assertEqual(fixture_tool.validate(self.points, self.ressim, self.jutul), [])

    def replace(self, target, source, label):
        """Replace `target` by a copy of `source` under `label`, flash outputs included."""
        points = [p for p in self.points if p["label"] != target]
        dup = copy.deepcopy(next(p for p in self.points if p["label"] == source))
        dup["label"] = label
        points.append(dup)
        ressim, jutul = dict(self.ressim), dict(self.jutul)
        for side in (ressim, jutul):
            del side[target]
            side[label] = {**copy.deepcopy(side[source]), "label": label}
        return fixture_tool.validate(points, ressim, jutul)

    def test_duplicate_under_a_new_label_is_refused(self):
        problems = self.replace("m205 c33 ressim", "m205 c32 ressim", "m205 c32 ressim (copy)")
        self.assertTrue(any("duplicates" in p for p in problems), problems)
        self.assertTrue(any("required points missing" in p for p in problems), problems)

    def test_duplicate_under_the_missing_label_is_refused(self):
        problems = self.replace("m205 c33 ressim", "m205 c32 ressim", "m205 c33 ressim")
        self.assertTrue(any("duplicates" in p for p in problems), problems)

    def test_label_not_matching_metadata_is_refused(self):
        points = copy.deepcopy(self.points)
        next(p for p in points if p["label"] == "m205 c33 ressim")["cell"] = 39
        problems = fixture_tool.validate(points, self.ressim, self.jutul)
        self.assertTrue(any("does not match its metadata" in p for p in problems), problems)

    def test_duplicate_disguised_as_the_missing_point_is_refused(self):
        """Another cell's state with metadata and label rewritten to the missing point's: only the
        swaps, which must reuse each original's p and z, can tell."""
        points = [p for p in self.points if p["label"] != "m205 c33 ressim"]
        dup = copy.deepcopy(next(p for p in self.points if p["label"] == "m205 c32 ressim"))
        dup.update(label="m205 c33 ressim", cell=33)
        points.append(dup)
        problems = fixture_tool.validate(points, self.ressim, self.jutul)
        self.assertTrue(any("p or z differs from the originals" in p for p in problems), problems)

    def test_swap_not_built_from_the_originals_is_refused(self):
        points = copy.deepcopy(self.points)
        swap = next(p for p in points if p["label"] == "m210 c39 ressim-z@jutul-p")
        swap["p_bar"] += 1e-9
        problems = fixture_tool.validate(points, self.ressim, self.jutul)
        self.assertEqual(problems, ["'m210 c39 ressim-z@jutul-p': p or z differs from the originals"])


def snapshot(month):
    cells = 147
    return {
        "time_days": 30.4375 * month,
        "pressure": [200.0 + month] * cells,
        "well_surface_rates": [{"liquid_m3_per_day": -10.0 - month, "vapour_m3_per_day": -50.0,
                                "water_m3_per_day": -1.0}],
        "water": {"saturation": [0.2] * cells},
        "vapour_saturation": [0.01 * month] * cells,
        "well_bhp_bar": [100.0],
        "cumulative_well_surface_production_volumes": [{"liquid_m3": 100.0 * month,
                                                        "vapour_m3": 500.0 * month}],
    }


def replay(months, stopped, cap=None):
    return {"schema": "ressim-spe5-replay/1",
            "settings": {"max_dt_days": cap},
            "stopped": stopped,
            "months": [{"month": m, "accepted_steps": 2, "failed_attempts": 0,
                        "accepted_newton": 8, "wall_s": 1.0, "balance_rel": [1e-12, -2e-12],
                        "snapshot": snapshot(m)} for m in months]}


def jutul_reports(path):
    return {m: {"mean_p_bar": 200.0 + m, "cell33_p_bar": 200.0 + m, "producer_oil_m3_day": 10.0 + m,
                "producer_gor_m3_m3": 5.0, "producer_watercut": 0.1, "cell33_sg": 0.01 * m,
                "producer_control": "orat"} for m in range(1, 241)}, None


WALL_BUDGET = {"reason": "wall budget", "month": 3, "day": 70.0,
               "month_so_far": {"calls": 1, "accepted_steps": 0}}
FAILED = {"reason": "step failed", "month": 218, "day": 6612.703, "attempts": 10,
          "failure": 'Some(StepFailure { kind: "Budget", message: "last failure was Nonlinear: '
                     'Newton did not converge in 20 iterations (scaled residual 6.39e-8)" })'}


class TimestepConvergenceTests(unittest.TestCase):
    def run_tool(self, runs, last=None):
        original = spe5_compare.reference_reports
        spe5_compare.reference_reports = jutul_reports
        try:
            with tempfile.TemporaryDirectory() as tmp:
                paths = []
                for i, data in enumerate(runs):
                    path = os.path.join(tmp, f"r{i}.json")
                    with open(path, "w") as f:
                        json.dump(data, f)
                    paths.append(path)
                out = io.StringIO()
                with contextlib.redirect_stdout(out):
                    spe5_compare.timestep_convergence(paths + ["j"] * 3, last)
                return out.getvalue()
        finally:
            spe5_compare.reference_reports = original

    def test_stop_formats(self):
        self.assertEqual(spe5_compare.format_stop(None), "no")
        self.assertEqual(spe5_compare.format_stop(WALL_BUDGET), "month 3, day 70.000: wall budget")
        self.assertEqual(spe5_compare.format_stop(FAILED),
                         "month 218, day 6612.703: step failed, 6.39e-8")

    def test_wall_budget_stop_is_reported(self):
        out = self.run_tool([replay([1, 2], WALL_BUDGET), replay([1, 2, 3], None, 5.0),
                             replay([1, 2, 3], None, 0.5)])
        self.assertIn("wall budget", out)
        self.assertIn("Worst difference over months 1–2", out)

    def test_replay_with_no_completed_report_is_inconclusive(self):
        out = self.run_tool([replay([], FAILED), replay([1, 2], None, 5.0),
                             replay([1, 2], None, 0.5)])
        self.assertIn("| none |", out)
        self.assertIn("No comparison", out)
        self.assertNotIn("Worst difference", out)

    def test_seeded_replays_compare_their_shared_months(self):
        out = self.run_tool([replay([218, 219], FAILED), replay([218, 219, 220], None, 5.0),
                             replay([217, 218, 219], None, 0.5)])
        self.assertIn("Worst difference over months 218–219", out)


if __name__ == "__main__":
    unittest.main()
