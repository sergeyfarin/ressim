"""The cases and the committed artifacts agree with each other and with the contract (#20)."""

from __future__ import annotations

import json

import pytest

from opm_flow_tool.artifacts import DEFAULT_ARTIFACT_DIR, SCHEMA_VERSION, deck_hash
from opm_flow_tool.cases import CASES, REPO_ROOT
from opm_flow_tool.vectors import METRIC_VECTORS, mnemonic_of


def _summary_section(deck: str) -> set[str]:
    lines = [line.strip() for line in deck.splitlines()]
    start = lines.index("SUMMARY") + 1
    end = lines.index("SCHEDULE")
    return {line for line in lines[start:end] if line and not line.startswith("--") and line != "/"}


@pytest.mark.parametrize("key", sorted(CASES))
def test_every_displayed_vector_lands_on_a_panel_of_its_own_quantity_and_phase(key):
    case = CASES[key]
    for curve_id, display in case.curve_display.items():
        spec = METRIC_VECTORS.get(mnemonic_of(curve_id))
        assert spec is not None, f"{key}: {curve_id} has no unit contract"
        assert display["panelKey"] in spec.panels, (
            f"{key}: {curve_id} ({spec.description}) mapped to panel '{display['panelKey']}', "
            f"allowed: {sorted(spec.panels)}"
        )
        assert spec.token in display["curveKey"].lower(), f"{key}: {curve_id} curveKey does not name '{spec.token}'"
        assert spec.token in display["label"].lower(), f"{key}: {curve_id} label does not name '{spec.token}'"


@pytest.mark.parametrize("key", sorted(CASES))
def test_every_vector_a_case_uses_is_requested_by_its_deck(key):
    case = CASES[key]
    requested = _summary_section(case.deck)
    used = {mnemonic_of(curve_id) for curve_id in case.curve_display}
    used.update(
        curve
        for curve in (case.cumulative_injection_curve, case.cumulative_surface_injection_curve, case.cumulative_gas_curve)
        if curve
    )
    assert used <= requested, f"{key}: deck SUMMARY lacks {sorted(used - requested)}"
    assert {"RUNSUM", "SEPARATE"} <= requested, f"{key}: deck writes no text summary"


@pytest.mark.parametrize("key", sorted(CASES))
def test_axis_vectors_have_the_basis_their_axis_assumes(key):
    case = CASES[key]
    # PVI is reservoir volume over pore volume; the cumulative-injection and
    # cumulative-gas axes are ResSim's surface volumes.
    if case.cumulative_injection_curve:
        assert METRIC_VECTORS[case.cumulative_injection_curve].unit == "RM3"
    if case.cumulative_surface_injection_curve:
        assert METRIC_VECTORS[case.cumulative_surface_injection_curve].unit == "SM3"
    if case.cumulative_gas_curve:
        assert METRIC_VECTORS[case.cumulative_gas_curve].unit == "SM3"


@pytest.mark.parametrize("key", sorted(CASES))
def test_a_committed_deck_source_is_the_deck_the_case_runs(key):
    case = CASES[key]
    source = REPO_ROOT / case.deck_source
    assert source.is_file(), case.deck_source
    if source.suffix == ".DATA":
        assert source.read_text(encoding="utf-8") == case.deck


def test_every_case_has_a_committed_artifact_and_every_artifact_a_case():
    committed = {path.stem for path in DEFAULT_ARTIFACT_DIR.glob("*.json")}
    assert committed == set(CASES)


@pytest.mark.parametrize("key", sorted(CASES))
def test_the_committed_artifact_is_parsed_versioned_attributed_and_current(key):
    case = CASES[key]
    artifact = json.loads((DEFAULT_ARTIFACT_DIR / f"{key}.json").read_text(encoding="utf-8"))

    assert artifact["status"] == "parsed", artifact.get("notes")
    assert artifact["schemaVersion"] == SCHEMA_VERSION
    assert artifact["scenarioKey"] == case.scenario_key
    assert artifact["flowVersion"], "no Flow version recorded"
    # Reproducible: the artifact was generated from the deck this case holds now.
    # Editing a deck without re-running Flow fails here.
    assert artifact["deckHash"] == deck_hash(case.deck), "deck changed since the artifact was generated"
    provenance = artifact["provenance"]
    assert provenance["deckSource"] == case.deck_source
    assert provenance["origin"] == case.origin
    assert f"run-flow {key}" in provenance["replay"]
    assert provenance["flowArgs"][1:] == list(case.flow_args)
    for series in artifact["series"]:
        spec = METRIC_VECTORS[mnemonic_of(series["mnemonic"])]
        assert series["unit"] == spec.unit, series["curveKey"]
