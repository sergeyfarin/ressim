"""Compositional run-record comparison and scorecard merge contracts.

Runs with Flow's system Python via uv. The artifact-only environment lacks numpy, so these tests
skip there, like the inventory-accounting tests.
"""
import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

try:
    from compare_small_direct import compare_comp_case, merge_scorecard
except ModuleNotFoundError:
    compare_comp_case = None
    merge_scorecard = None


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

    def test_null_final_saturation_is_rejected(self):
        self.record["reports"][0]["sg"] = [None]
        with self.assertRaisesRegex(SystemExit, r"non-finite"):
            self.compare()

    def test_null_water_saturation_is_rejected(self):
        self.record["reports"][0]["sw"] = [None]
        with self.assertRaisesRegex(SystemExit, r"non-finite"):
            self.compare()

    def test_nonfinite_composition_is_rejected(self):
        self.record["reports"][0]["z"][1] = [float("nan")]
        with self.assertRaisesRegex(SystemExit, r"non-finite"):
            self.compare()

    def test_null_pressure_is_rejected(self):
        self.record["reports"][0]["p"] = [None]
        with self.assertRaisesRegex(SystemExit, r"non-finite"):
            self.compare()


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


if __name__ == "__main__":
    unittest.main()
