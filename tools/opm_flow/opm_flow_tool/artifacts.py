from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
from datetime import datetime, timezone
from pathlib import Path

from .cases import CASES, REPO_ROOT, OpmCase
from .summary import SummaryData, find_summary_file, parse_rsm
from .vectors import METRIC_VECTORS, TIME_UNIT, mnemonic_of

# 2: series carry their mnemonic and verified unit, the artifact carries a
# provenance block, and xAxis separates surface from reservoir injection (#20).
SCHEMA_VERSION = 2
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
        [flow, str(deck_path), f"--output-dir={output_dir}", "--enable-terminal-output=false", *case.flow_args],
        check=True,
    )
    return output_dir


def _deck_is_metric(deck: str) -> bool:
    return any(line.strip() == "METRIC" for line in deck.splitlines())


def check_unit_contract(case: OpmCase, summary: SummaryData) -> str | None:
    """Why this summary cannot become an artifact, or None if it can.

    Checks what Flow printed, not what the case says it asked for: the deck's
    unit system, the TIME unit, and the unit row under every vector the case
    displays or builds an axis from. A mnemonic with no entry in
    `vectors.METRIC_VECTORS` is refused too, since nothing says what its unit
    should be.
    """
    if not _deck_is_metric(case.deck):
        return "the deck is not METRIC, and the unit contract is METRIC's"
    if summary.time_unit != TIME_UNIT:
        return f"TIME is in {summary.time_unit or 'no unit'}, expected {TIME_UNIT}"

    used = set(case.curve_display)
    used.update(
        curve
        for curve in (
            case.cumulative_injection_curve,
            case.cumulative_surface_injection_curve,
            case.cumulative_gas_curve,
        )
        if curve
    )
    by_curve = summary.by_curve_id()
    problems = []
    for curve_id in sorted(used):
        vector = by_curve.get(curve_id)
        if vector is None:
            continue  # reported as a missing curve / missing mapping, not a unit error
        spec = METRIC_VECTORS.get(mnemonic_of(curve_id))
        if spec is None:
            problems.append(f"{curve_id} has no unit contract")
        elif vector.unit != spec.unit:
            problems.append(f"{curve_id} is in {vector.unit or 'no unit'}, expected {spec.unit or 'no unit'}")
    if problems:
        return "unit mismatch: " + "; ".join(problems)
    return None


def _provenance(case: OpmCase, flow: str | None) -> dict:
    run = f"uv run --directory tools/opm_flow python -m opm_flow_tool.cli run-flow {case.key}"
    build = f"uv run --directory tools/opm_flow python -m opm_flow_tool.cli build-artifacts {case.key}"
    return {
        "generator": "tools/opm_flow (opm_flow_tool)",
        "simulator": flow,
        "deckSource": case.deck_source,
        "flowArgs": ["--enable-terminal-output=false", *case.flow_args],
        "replay": f"{run} && {build}",
        "origin": case.origin,
    }


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

    # PVI is a reservoir-volume ratio: FVIT over the deck's pore volume.
    if case.cumulative_injection_curve and case.pore_volume_m3:
        vector = by_curve.get(case.cumulative_injection_curve)
        if vector is not None:
            axis["cumulativeInjectionM3"] = list(vector.values)
            axis["pvi"] = [value / case.pore_volume_m3 for value in vector.values]
            axis["poreVolumeM3"] = case.pore_volume_m3
            axis["cumulativeInjectionCurve"] = case.cumulative_injection_curve

    # The cumulative-injection axis is ResSim's surface volume, so it is fed from
    # the surface vector, never from FVIT: for gas the two differ by Bg.
    if case.cumulative_surface_injection_curve:
        vector = by_curve.get(case.cumulative_surface_injection_curve)
        if vector is not None:
            axis["cumulativeInjectionSm3"] = list(vector.values)
            axis["cumulativeSurfaceInjectionCurve"] = case.cumulative_surface_injection_curve

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

    contract_error = check_unit_contract(case, summary)
    if contract_error:
        return [], "error", f"{summary_path.name}: {contract_error}", None

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
                "mnemonic": curve_id,
                "unit": vector.unit,
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
    if case.cumulative_surface_injection_curve and not (x_axis or {}).get("cumulativeInjectionSm3"):
        notes += (
            f" No time->cumulative-injection mapping: {case.cumulative_surface_injection_curve} was"
            " requested by the case but is missing from the summary."
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

    flow = flow_version()
    artifact = {
        "schemaVersion": SCHEMA_VERSION,
        "sourceType": "opm-flow-precomputed",
        "caseKey": case.key,
        "scenarioKey": case.scenario_key,
        "label": case.label,
        "flowVersion": flow,
        "deckHash": deck_hash(case.deck),
        "generatedAt": generated_at,
        # Only what the unit contract verified; each series names its own unit.
        "units": {"system": "METRIC", "time": "days"},
        "provenance": _provenance(case, flow),
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
