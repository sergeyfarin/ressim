"""Surface accounting contracts; run with Flow's system Python via uv.

The artifact-only uv environment lacks numpy/opm.io and skips these tests.
"""
import unittest

try:
    from compare_small_direct import inventory_relative_error, ressim_cumulatives
except ModuleNotFoundError:
    inventory_relative_error = None


@unittest.skipIf(inventory_relative_error is None, "requires Flow's numpy/opm.io environment")
class InventoryAccountingTests(unittest.TestCase):
    def run_record(self, injected="water"):
        # Irregular accepted timesteps: 1 day, then 1.5 days. A report-step
        # rectangle using only the last rate would silently miscount production.
        return {
            "injected": injected,
            "history": [[1.0, 3.0, 2.0, 1.0, 4.0], [2.5, 4.0, 1.0, 2.0, 6.0]],
            "initial_inventory_sc": [20.0, 30.0, 40.0],
            "final_inventory_sc": [29.5, 21.0, 36.0] if injected == "water" else [16.5, 21.0, 49.0],
        }

    def test_irregular_steps_and_injection_units(self):
        for injected in ("water", "gas"):
            with self.subTest(injected=injected):
                run = self.run_record(injected)
                cum = ressim_cumulatives(run)
                self.assertEqual(cum["FOPT"], 9.0)
                self.assertEqual(cum["FWPT"], 3.5)
                self.assertEqual(inventory_relative_error(run, cum), dict(water=0.0, oil=0.0, gas=0.0))

    def test_detects_created_oil_relative_to_production(self):
        run = self.run_record()
        run["final_inventory_sc"][1] += 1.0
        self.assertAlmostEqual(inventory_relative_error(run, ressim_cumulatives(run))["oil"], 1 / 9)

    def test_legacy_capture_is_missing_evidence(self):
        run = self.run_record()
        del run["initial_inventory_sc"]
        self.assertIsNone(inventory_relative_error(run, ressim_cumulatives(run)))

    def test_nonfinite_capture_cannot_close_inventory(self):
        run = self.run_record()
        run["final_inventory_sc"][1] = float("inf")
        self.assertEqual(inventory_relative_error(run, ressim_cumulatives(run))["oil"], float("inf"))
