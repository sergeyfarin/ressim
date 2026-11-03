"""Regression fixtures for the Coverage grid's scenario-test check.

Run with `python3 -m unittest discover -s tools/benchmarks/tests`.
"""
from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path
from unittest import mock

MODULE_PATH = Path(__file__).resolve().parents[1] / "benchmarks.py"
SPEC = importlib.util.spec_from_file_location("ressim_benchmarks", MODULE_PATH)
assert SPEC and SPEC.loader
benchmarks = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(benchmarks)

FIXTURES = Path(__file__).resolve().parent / "fixtures" / "coverage"


class ScenarioTestCoverageTests(unittest.TestCase):
    """A `<key>.test.ts` counts only when it declares an active test that names the scenario."""

    def check(self, stem: str) -> str | None:
        with mock.patch.object(benchmarks, "SCENARIOS", FIXTURES):
            return benchmarks.scenario_test(FIXTURES / f"{stem}.ts", stem)

    def test_active_test_counts(self) -> None:
        self.assertEqual(self.check("active"), "active.test.ts")

    def test_only_test_counts(self) -> None:
        self.assertEqual(self.check("only"), "only.test.ts")

    def test_skipped_test_does_not_count(self) -> None:
        self.assertIsNone(self.check("skip"))

    def test_todo_test_does_not_count(self) -> None:
        self.assertIsNone(self.check("todo"))

    def test_nested_skip_modifier_does_not_count(self) -> None:
        self.assertIsNone(self.check("concurrent_skip"))

    def test_skip_after_argument_list_does_not_count(self) -> None:
        self.assertIsNone(self.check("each_skip"))

    def test_comment_only_mention_does_not_count(self) -> None:
        self.assertIsNone(self.check("comment_only"))

    def test_active_test_for_another_scenario_does_not_count(self) -> None:
        self.assertIsNone(self.check("wrong_key"))

    def test_file_without_a_test_does_not_count(self) -> None:
        self.assertIsNone(self.check("no_test"))

    def test_missing_file_does_not_count(self) -> None:
        self.assertIsNone(self.check("missing"))


if __name__ == "__main__":
    unittest.main()
