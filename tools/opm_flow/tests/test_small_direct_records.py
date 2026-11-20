"""Compositional run-record comparison and scorecard merge contracts.

Runs with Flow's system Python via uv. The artifact-only environment lacks numpy, so these tests
skip there, like the inventory-accounting tests.
"""
import json
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

try:
    from compare_small_direct import (
        case_family,
        compare_comp_case,
        comp_record_cumulatives,
        coverage_error,
        merge_scorecard,
        main,
        scorecard_metrics,
    )
except ModuleNotFoundError:
    case_family = None
    compare_comp_case = None
    comp_record_cumulatives = None
    coverage_error = None
    main = None
    merge_scorecard = None
    scorecard_metrics = None


@unittest.skipIf(compare_comp_case is None, "requires Flow's numpy environment")
class CompositionalRecordTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)
        self.reference = self.dir / "reference.json"
        self.reference.write_text(json.dumps({
            "schema": "ressim-compositional-reference/1",
            "report_steps": [{
                "pressure": [100.0],
                "sgas": [0.2],
                "zmf1": [0.5],
                "zmf2": [0.3],
                "zmf3": [0.2],
            }],
            "summary": {"TIME": [1.0]},
        }))
        self.record = {
            "case": "tiny",
            "solver": "compositional",
            "run": "comp",
            "wall_ms": 1.0,
            "warnings": [],
            "reports": [{
                "t": 1.0,
                "p": [100.0],
                "sw": [0.0],
                "sg": [0.2],
                "z": [[0.5], [0.3], [0.2]],
            }],
        }

    def tearDown(self):
        self.tmp.cleanup()

    def compare(self, record=None):
        (self.dir / "tiny.comp.json").write_text(json.dumps(record or self.record))
        return compare_comp_case("tiny", self.reference, SimpleNamespace(ressim_dir=self.dir))

    def test_finite_record_compares(self):
        worst = self.compare()["runs"]["comp"]["vs_flow"]["worst"]
        self.assertEqual(worst, {"p": 0.0, "sg": 0.0, "z": 0.0})

    def test_nonfinite_report_time_is_rejected(self):
        self.record["reports"][0]["t"] = float("nan")
        with self.assertRaisesRegex(SystemExit, r"non-finite report time"):
            self.compare()

    def test_null_final_saturation_is_rejected(self):
        self.record["reports"][0]["sg"] = [None]
        with self.assertRaisesRegex(SystemExit, r"non-finite"):
            self.compare()

    def test_null_water_saturation_is_rejected(self):
        self.record["reports"][0]["sw"] = [None]
        with self.assertRaisesRegex(SystemExit, r"non-finite"):
            self.compare()

    def test_whole_field_null_water_saturation_is_rejected(self):
        self.record["reports"][0]["sw"] = None
        with self.assertRaisesRegex(SystemExit, r"non-finite"):
            self.compare()

    def test_scalar_water_saturation_is_rejected(self):
        self.record["reports"][0]["sw"] = 0.0
        with self.assertRaisesRegex(SystemExit, r"must be 1-d"):
            self.compare()

    def test_one_dimensional_composition_is_rejected(self):
        self.record["reports"][0]["z"] = [0.5, 0.3, 0.2]
        with self.assertRaisesRegex(SystemExit, r"must be 2-d"):
            self.compare()

    def test_record_cell_count_must_match_reference(self):
        self.record["reports"][0].update({
            "p": [100.0, 100.0],
            "sw": [0.0, 0.0],
            "sg": [0.2, 0.2],
            "z": [[0.5, 0.5], [0.3, 0.3], [0.2, 0.2]],
        })
        with self.assertRaisesRegex(SystemExit, r"reference"):
            self.compare()

    def test_nonfinite_composition_is_rejected(self):
        self.record["reports"][0]["z"][1] = [float("nan")]
        with self.assertRaisesRegex(SystemExit, r"non-finite"):
            self.compare()

    def test_null_pressure_is_rejected(self):
        self.record["reports"][0]["p"] = [None]
        with self.assertRaisesRegex(SystemExit, r"non-finite"):
            self.compare()

    def test_final_surface_cumulatives_are_compared(self):
        reference = json.loads(self.reference.read_text())
        reference["summary"]["FOPT"] = [10.0]
        reference["summary"]["FGPT"] = [40.0]
        self.reference.write_text(json.dumps(reference))
        self.record["reports"][0]["surface"] = {
            "cumulative_liquid_m3": 10.5,
            "cumulative_vapour_m3": 41.0,
        }
        report = self.compare()
        cum = report["runs"]["comp"]["cum_rel_vs_flow"]
        self.assertAlmostEqual(cum["FOPT"], 0.05)
        self.assertAlmostEqual(cum["FGPT"], 0.025)
        self.assertEqual(report["flow"]["cum"], {"FOPT": 10.0, "FGPT": 40.0})

    def test_missing_surface_ledger_is_not_read_as_zero(self):
        reference = json.loads(self.reference.read_text())
        reference["summary"]["FOPT"] = [10.0]
        self.reference.write_text(json.dumps(reference))
        report = self.compare()
        self.assertEqual(report["runs"]["comp"]["cum_rel_vs_flow"], {})
        self.assertEqual(report["flow"]["cum_unreported"], ["FOPT"])

    def test_nonfinite_surface_cumulative_is_rejected(self):
        self.record["reports"][0]["surface"] = {
            "cumulative_liquid_m3": float("nan"),
            "cumulative_vapour_m3": 0.0,
        }
        with self.assertRaisesRegex(SystemExit, r"not finite"):
            self.compare()


@unittest.skipIf(main is None, "requires Flow's numpy environment")
class Spe5DiagnosticTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.dir = Path(tmp.name)
        self.out = self.dir / "comparison.json"
        reference = self.dir / "reference.json"
        reference.write_text(json.dumps({
            "schema": "ressim-spe5-reference/1",
            "completed": True,
            "watercut_limit": 0.8,
            "gor_limit_m3_m3": 300.0,
            "stopped": {"day": 2 * 30.4375, "reason": "watercut"},
            "reports": [{
                "day": month * 30.4375,
                "cell33_p_bar": 100.0,
                "cell33_sw": 0.2,
                "cell33_sg": 0.3,
                "mean_p_bar": 100.0,
                "producer_oil_m3_day": 30.0,
                "producer_gor_m3_m3": 200.0,
                "producer_watercut": 0.25 if month == 1 else 0.875,
            } for month in range(1, 241)],
        }))
        (self.dir / "trajectory_cases.json").write_text(json.dumps({
            "cases": [],
            "diagnostics": [{"case": "spe5", "reference": str(reference), "format": "spe5-summary-v1"}],
        }))
        self.record = {
            "case": "spe5",
            "solver": "compositional",
            "run": "comp",
            "wall_ms": 1.0,
            "reports": [{
                "t": month * 30.4375,
                "p": [101.0] * 147,
                "sw": [0.0] * 33 + [0.325] + [0.0] * 113,
                "sg": [0.3] * 147,
                "average_pressure_bar": 90.0 if month == 3 else 101.0,
                "surface": {"liquid_rate_m3_per_day": 20.0, "gas_liquid_ratio": 250.0,
                            "water_cut": {1: 0.125, 2: 0.75, 3: 0.99}[month]},
                "wall_ms": 1.0,
            } for month in (1, 2, 3)],
        }

    def run_cli(self, *options):
        (self.dir / "spe5.comp.json").write_text(json.dumps(self.record))
        argv = ["compare_small_direct.py", "--ressim-dir", str(self.dir),
                "--comp-reference-dir", str(self.dir), "--json", str(self.out), *options]
        with patch("sys.argv", argv), redirect_stdout(StringIO()) as output:
            self.assertEqual(main(), 0)
        return output.getvalue()

    def test_spe5_compares_cell_33_and_summary_over_the_declared_window(self):
        output = self.run_cli("--case", "spe5", "--markdown")
        entry = json.loads(self.out.read_text())["spe5"]
        self.assertIn("JutulDarcy", output)
        self.assertIn("2 of 240", output)
        self.assertIn("cell 33", output)
        self.assertIn("ResSim 0.750000", output)
        self.assertIn("JutulDarcy 0.875000", output)
        self.assertIsNone(entry["comparison_scope"]["ressim_stop"])
        run = entry["runs"]["comp"]
        self.assertEqual(run["vs_flow"]["worst"], {"p": 1.0, "sw": 0.125, "sg": 0.0})
        self.assertEqual(run["summary_errors"]["worst"], {
            "mean_p_bar": 1.0, "producer_oil_relative": 1 / 3, "producer_gor_relative": 0.2,
            "producer_water_cut": 0.125,
        })

    def test_spe5_stops_earlier_if_ressim_reaches_a_limit_first(self):
        self.record["reports"][0]["surface"]["gas_liquid_ratio"] = 300.0
        self.record["reports"] = self.record["reports"][:1]
        self.run_cli("--case", "spe5")
        scope = json.loads(self.out.read_text())["spe5"]["comparison_scope"]
        self.assertEqual(scope["reports"], 1)
        self.assertEqual(scope["ressim_stop"], {"report": 1, "day": 30.4375, "reason": "gor"})

    def test_spe5_requires_the_entire_window_and_matching_times(self):
        self.record["reports"][1]["t"] = 4 * 30.4375
        with self.assertRaisesRegex(SystemExit, r"report 1 is at"):
            self.run_cli("--case", "spe5")
        self.record["reports"] = self.record["reports"][:1]
        with self.assertRaisesRegex(SystemExit, r"ends before the reference shut-in"):
            self.run_cli("--case", "spe5")

    def test_diagnostic_is_excluded_from_the_default_coverage(self):
        with self.assertRaisesRegex(SystemExit, r"no compositional cases"):
            self.run_cli()

    def test_diagnostic_rejects_an_unknown_reference_format(self):
        path = self.dir / "trajectory_cases.json"
        manifest = json.loads(path.read_text())
        manifest["diagnostics"][0]["format"] = "unsupported"
        path.write_text(json.dumps(manifest))
        with self.assertRaisesRegex(SystemExit, r"unsupported diagnostic format"):
            self.run_cli("--case", "spe5")

    def test_diagnostic_cannot_check_or_write_a_scorecard(self):
        for flag in ("--check", "--write-scorecard"):
            with self.subTest(flag=flag), self.assertRaisesRegex(SystemExit, r"diagnostic"):
                self.run_cli("--case", "spe5", "--scorecard", str(self.dir / "card.json"), flag)
        self.assertFalse(self.out.exists())


@unittest.skipIf(comp_record_cumulatives is None, "requires Flow's numpy environment")
class CumulativeReportTests(unittest.TestCase):
    def test_only_the_final_report_is_read(self):
        run = {
            "reports": [
                {"surface": {"cumulative_liquid_m3": 9.0, "cumulative_vapour_m3": 99.0}},
                {"p": [100.0]},
            ]
        }
        self.assertEqual(comp_record_cumulatives(run), {})

    def test_final_report_carries_the_cumulatives(self):
        run = {
            "reports": [
                {"surface": {"cumulative_liquid_m3": 9.0, "cumulative_vapour_m3": 99.0}},
                {"surface": {"cumulative_liquid_m3": 10.0, "cumulative_vapour_m3": 100.0}},
            ]
        }
        self.assertEqual(
            comp_record_cumulatives(run),
            {"FOPT": 10.0, "FGPT": 100.0},
        )


@unittest.skipIf(merge_scorecard is None, "requires Flow's numpy environment")
class ScorecardMergeTests(unittest.TestCase):
    def old_provenance(self):
        return {
            "commit": "abc1234",
            "dirty": False,
            "date": "2026-01-01",
            "flow": "flow 2026.04",
            "command": "bash scripts/validate-cross-solver.sh --update",
        }

    def new_provenance(self):
        return {
            "commit": "def5678",
            "dirty": False,
            "date": "2026-01-02",
            "flow": "flow 2026.04",
            "command": "bash scripts/validate-compositional.sh update-scorecard",
            "cases": ["new"],
        }

    def test_preserved_case_inherits_legacy_provenance(self):
        existing = {
            "provenance": self.old_provenance(),
            "cases": {"old": {"flow": {}, "runs": {}, "pairs": {}}},
        }
        measured = {"new": {"flow": {}, "runs": {}, "pairs": {}, "measured": self.new_provenance()}}
        card, kept = merge_scorecard(existing, measured, self.new_provenance(), prune=False)
        self.assertEqual(kept, 1)
        self.assertEqual(card["cases"]["old"]["measured"], self.old_provenance())
        self.assertEqual(card["cases"]["new"]["measured"]["commit"], "def5678")
        self.assertEqual(card["provenance"]["cases"], ["new"])

    def test_existing_per_case_provenance_wins(self):
        existing = {
            "provenance": self.old_provenance(),
            "cases": {"old": {"measured": {"commit": "earlier"}}},
        }
        card, _ = merge_scorecard(existing, {}, self.new_provenance(), prune=False)
        self.assertEqual(card["cases"]["old"]["measured"], {"commit": "earlier"})

    def test_prune_drops_preserved_cases(self):
        existing = {
            "provenance": self.old_provenance(),
            "cases": {"old": {"flow": {}, "runs": {}, "pairs": {}}},
        }
        card, kept = merge_scorecard(existing, {}, self.new_provenance(), prune=True)
        self.assertEqual(kept, 0)
        self.assertNotIn("old", card["cases"])


@unittest.skipIf(coverage_error is None, "requires Flow's numpy environment")
class CoverageTests(unittest.TestCase):
    def test_empty_family_is_rejected(self):
        error = coverage_error({}, set(), "compositional", partial=False)
        self.assertIn("no compositional cases", error)

    def test_missing_case_is_rejected(self):
        error = coverage_error({"1d_comp": {}}, {"1d_comp", "injection"}, "compositional", partial=False)
        self.assertIn("injection", error)
        self.assertIn("--case", error)

    def test_complete_run_passes(self):
        self.assertIsNone(coverage_error({"a": {}}, {"a"}, "compositional", partial=False))

    def test_partial_run_is_allowed(self):
        self.assertIsNone(coverage_error({}, {"a", "b"}, "compositional", partial=True))

    def test_family_comes_from_run_ids(self):
        self.assertEqual(case_family({"runs": {"comp": {}}}), "compositional")
        self.assertEqual(case_family({"runs": {"sparse": {}, "dense": {}}}), "black-oil")


@unittest.skipIf(scorecard_metrics is None, "requires Flow's numpy environment")
class WorkRatioTests(unittest.TestCase):
    def entry(self, run_newton):
        return {
            "flow": {"conv": {"substeps": 20, "newton": 50, "retries": 0, "retry_newton": 0}},
            "runs": {
                "sparse": {
                    "vs_flow": {
                        "worst": {"p": 0.1, "sw": 0.0, "sg": 0.0},
                        "final": {"p": 0.1, "sw": 0.0, "sg": 0.0},
                    },
                    "cum_rel_vs_flow": {},
                    "conv": {"substeps": 20, "newton": run_newton, "retries": 0, "retry_newton": 0},
                    "warnings": [],
                    "wall_ms": 1.0,
                }
            },
            "pairs": {},
        }

    def test_fim_newton_ratio_is_recorded(self):
        metrics = scorecard_metrics(self.entry(60))["runs"]["sparse"]["metrics"]
        self.assertAlmostEqual(metrics["newton_ratio"], 1.2)

    def test_impes_without_a_newton_count_omits_the_ratio(self):
        metrics = scorecard_metrics(self.entry(None))["runs"]["sparse"]["metrics"]
        self.assertNotIn("newton_ratio", metrics)


if __name__ == "__main__":
    unittest.main()
