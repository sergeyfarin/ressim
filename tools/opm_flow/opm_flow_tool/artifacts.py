from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
from datetime import datetime, timezone
from pathlib import Path

from .cases import CASES, OpmCase
from .summary import find_summary_file, parse_rsm

REPO_ROOT = Path(__file__).resolve().parents[3]
DEFAULT_RUN_ROOT = REPO_ROOT / "tmp" / "opm-flow-runs"
DEFAULT_ARTIFACT_DIR = REPO_ROOT / "src" / "lib" / "catalog" / "opm-flow-results"


def deck_hash(deck: str) -> str:
    return hashlib.sha256(deck.encode("utf-8")).hexdigest()


def write_deck(case: OpmCase, output: Path | None = None) -> Path:
    output = output or DEFAULT_RUN_ROOT / "decks" / case.deck_name
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(case.deck, encoding="utf-8")
    return output


def flow_version() -> str | None:
    flow = shutil.which("flow")
    if not flow:
        return None
    result = subprocess.run([flow, "--version"], check=False, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    return result.stdout.strip() or None


def run_flow(case: OpmCase, run_root: Path = DEFAULT_RUN_ROOT) -> Path:
    flow = shutil.which("flow")
    if not flow:
        raise RuntimeError("OPM Flow executable `flow` was not found on PATH")
    deck_path = write_deck(case, run_root / "decks" / case.deck_name)
    output_dir = run_root / case.key
    output_dir.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [flow, str(deck_path), f"--output-dir={output_dir}", "--enable-terminal-output=false"],
        check=True,
    )
    return output_dir


def _build_x_axis(case: OpmCase, summary) -> dict | None:
    """The run's own time -> volume mappings, or None if it publishes none.

    Reference series are recorded against days. A chart drawn against pore
    volumes injected, cumulative injection or cumulative gas production has to
    convert them, and the only defensible conversion uses this run's own
    volumes rather than the scenario's — the artifact is a fixed run, not a
    re-parameterisable one.

    Each mapping is independent. A waterflood publishes the injection ones and
    no gas one; a depletion case publishes the gas one and has no injector at
    all. Whatever is missing is simply absent, and the frontend drops the
    reference curves on axes it cannot honestly place them on.
    """
    by_curve = summary.by_curve_id()
    axis: dict = {"timeDays": list(summary.time_days)}

    if case.cumulative_injection_curve and case.pore_volume_m3:
        vector = by_curve.get(case.cumulative_injection_curve)
        if vector is not None:
            axis["cumulativeInjectionM3"] = list(vector.values)
            axis["pvi"] = [value / case.pore_volume_m3 for value in vector.values]
            axis["poreVolumeM3"] = case.pore_volume_m3
            axis["cumulativeInjectionCurve"] = case.cumulative_injection_curve

    if case.cumulative_gas_curve:
        vector = by_curve.get(case.cumulative_gas_curve)
        if vector is not None:
            axis["cumulativeGasSm3"] = list(vector.values)
            axis["cumulativeGasCurve"] = case.cumulative_gas_curve

    # timeDays alone is not a mapping — it is what the series already carry.
    return axis if len(axis) > 1 else None


def _build_series(case: OpmCase, run_dir: Path) -> tuple[list[dict], str, str, dict | None]:
    """Return (series, status, notes, x_axis) for a case's run directory.

    Never raises: parsing failures degrade to status 'error' with the
    exception message recorded in notes, so a bad run can't crash
    `build-artifacts all` for every other case.
    """
    summary_path = find_summary_file(run_dir)
    if summary_path is None:
        return (
            [],
            "flow-run",
            f"Flow run directory found at {run_dir} but no .RSM summary file was present "
            "(deck may be missing RUNSUM, or Flow hasn't finished).",
            None,
        )

    try:
        summary = parse_rsm(summary_path.read_text(encoding="utf-8"))
    except ValueError as exc:
        return [], "error", f"Failed to parse {summary_path.name}: {exc}", None

    vectors_by_id = summary.by_curve_id()
    series: list[dict] = []
    missing = [curve_id for curve_id in case.curve_display if curve_id not in vectors_by_id]
    if missing:
        return (
            [],
            "error",
            f"Parsed {summary_path.name} but it is missing expected curve(s): {', '.join(sorted(missing))}",
            None,
        )

    for curve_id, display in case.curve_display.items():
        vector = vectors_by_id[curve_id]
        series.append(
            {
                "panelKey": display["panelKey"],
                "label": display["label"],
                "curveKey": display["curveKey"],
                "data": [{"x": t, "y": v} for t, v in zip(summary.time_days, vector.values)],
            }
        )

    x_axis = _build_x_axis(case, summary)
    notes = "Series parsed from a real Flow run."
    if case.cumulative_injection_curve and not (x_axis or {}).get("pvi"):
        notes += (
            f" No time->PVI mapping: {case.cumulative_injection_curve} was requested by the case"
            " but is missing from the summary (or the case declares no pore volume)."
        )
    if case.cumulative_gas_curve and not (x_axis or {}).get("cumulativeGasSm3"):
        notes += (
            f" No time->cumulative-gas mapping: {case.cumulative_gas_curve} was requested by the"
            " case but is missing from the summary."
        )
    return series, "parsed", notes, x_axis


def build_artifact(
    case: OpmCase,
    artifact_dir: Path = DEFAULT_ARTIFACT_DIR,
    generated_at: str | None = None,
    run_root: Path = DEFAULT_RUN_ROOT,
) -> Path:
    artifact_dir.mkdir(parents=True, exist_ok=True)
    generated_at = generated_at or datetime.now(timezone.utc).replace(microsecond=0).isoformat()

    run_dir = run_root / case.key
    if run_dir.is_dir():
        series, status, notes, x_axis = _build_series(case, run_dir)
    else:
        series, status, notes, x_axis = (
            [],
            "deck-ready",
            "Generated artifact metadata is available. Run Flow and attach parsed summary series before treating this as numerical reference data.",
            None,
        )

    artifact = {
        "schemaVersion": 1,
        "sourceType": "opm-flow-precomputed",
        "caseKey": case.key,
        "scenarioKey": case.scenario_key,
        "label": case.label,
        "flowVersion": flow_version(),
        "deckHash": deck_hash(case.deck),
        "generatedAt": generated_at,
        "units": case.units,
        "supportedCurves": list(case.supported_curves),
        "series": series,
        "status": status,
        "notes": notes,
    }
    if x_axis is not None:
        artifact["xAxis"] = x_axis
    output = artifact_dir / f"{case.key}.json"
    output.write_text(json.dumps(artifact, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return output
