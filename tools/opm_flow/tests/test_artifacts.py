from __future__ import annotations

import json
import shutil
from pathlib import Path

from opm_flow_tool.artifacts import build_artifact
from opm_flow_tool.cases import CASES

FIXTURES = Path(__file__).parent / "fixtures"


def test_build_artifact_stays_deck_ready_when_no_run_directory_exists(tmp_path):
    case = CASES["wf_bl1d"]
    output = build_artifact(case, artifact_dir=tmp_path / "artifacts", run_root=tmp_path / "runs")

    artifact = json.loads(output.read_text(encoding="utf-8"))
    assert artifact["status"] == "deck-ready"
    assert artifact["series"] == []


def test_build_artifact_stays_flow_run_when_run_dir_exists_but_no_rsm(tmp_path):
    case = CASES["wf_bl1d"]
    run_root = tmp_path / "runs"
    (run_root / case.key).mkdir(parents=True)

    output = build_artifact(case, artifact_dir=tmp_path / "artifacts", run_root=run_root)

    artifact = json.loads(output.read_text(encoding="utf-8"))
    assert artifact["status"] == "flow-run"
    assert artifact["series"] == []
    assert "no .RSM summary" in artifact["notes"]


def test_build_artifact_parses_real_summary_into_series(tmp_path):
    case = CASES["wf_bl1d"]
    run_root = tmp_path / "runs"
    run_dir = run_root / case.key
    run_dir.mkdir(parents=True)
    shutil.copy(FIXTURES / "wf_bl1d_sample.RSM", run_dir / f"{case.deck_name.removesuffix('.DATA')}.RSM")

    output = build_artifact(case, artifact_dir=tmp_path / "artifacts", run_root=run_root)
    artifact = json.loads(output.read_text(encoding="utf-8"))

    assert artifact["status"] == "parsed"
    curve_keys = {series["curveKey"] for series in artifact["series"]}
    assert curve_keys == {
        "opm-oil-rate",
        "opm-water-rate",
        "opm-injection-rate",
        "opm-cum-oil",
        "opm-cum-water",
        "opm-avg-pressure",
    }

    oil_rate = next(s for s in artifact["series"] if s["curveKey"] == "opm-oil-rate")
    assert oil_rate["panelKey"] == "rates"
    assert oil_rate["data"] == [
        {"x": 0.25, "y": 14.51805},
        {"x": 0.5, "y": 13.59252},
        {"x": 0.75, "y": 13.44682},
        {"x": 1.0, "y": 13.67147},
        {"x": 1.25, "y": 14.15780},
        {"x": 1.5, "y": 14.62355},
    ]


def test_build_artifact_parses_well_scoped_vectors_for_spe1(tmp_path):
    case = CASES["spe1_gas_injection"]
    run_root = tmp_path / "runs"
    run_dir = run_root / case.key
    run_dir.mkdir(parents=True)
    shutil.copy(FIXTURES / "spe1_gas_injection_sample.RSM", run_dir / f"{case.deck_name.removesuffix('.DATA')}.RSM")

    output = build_artifact(case, artifact_dir=tmp_path / "artifacts", run_root=run_root)
    artifact = json.loads(output.read_text(encoding="utf-8"))

    assert artifact["status"] == "parsed"
    curve_keys = {series["curveKey"] for series in artifact["series"]}
    assert curve_keys == {
        "opm-oil-rate",
        "opm-gas-injection-rate",
        "opm-cum-oil",
        "opm-cum-gas",
        "opm-avg-pressure",
        "opm-injector-bhp",
        "opm-producer-bhp",
        "opm-gor",
    }

    injector_bhp = next(s for s in artifact["series"] if s["curveKey"] == "opm-injector-bhp")
    assert injector_bhp["data"][1] == {"x": 4.0, "y": 621.0}

    producer_bhp = next(s for s in artifact["series"] if s["curveKey"] == "opm-producer-bhp")
    assert producer_bhp["data"][0] == {"x": 1.0, "y": 69.0}
    assert producer_bhp["data"][1] == {"x": 4.0, "y": 69.0}

    cum_gas = next(s for s in artifact["series"] if s["curveKey"] == "opm-cum-gas")
    assert cum_gas["data"][0] == {"x": 1.0, "y": 6629.687}


def test_build_artifact_reports_error_status_on_malformed_summary(tmp_path):
    case = CASES["wf_bl1d"]
    run_root = tmp_path / "runs"
    run_dir = run_root / case.key
    run_dir.mkdir(parents=True)
    (run_dir / f"{case.deck_name.removesuffix('.DATA')}.RSM").write_text("not a real summary file", encoding="utf-8")

    output = build_artifact(case, artifact_dir=tmp_path / "artifacts", run_root=run_root)
    artifact = json.loads(output.read_text(encoding="utf-8"))

    assert artifact["status"] == "error"
    assert artifact["series"] == []
    assert "Failed to parse" in artifact["notes"]


def test_every_case_curve_display_key_is_a_subset_of_its_supported_curves():
    for case in CASES.values():
        for curve_id in case.curve_display:
            mnemonic = curve_id.split(":", 1)[0]
            assert mnemonic in case.supported_curves, (
                f"{case.key}: curve_display key '{curve_id}' has mnemonic "
                f"'{mnemonic}' not in supported_curves {case.supported_curves}"
            )
