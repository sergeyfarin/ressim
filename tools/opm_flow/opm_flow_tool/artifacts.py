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


def _build_series(case: OpmCase, run_dir: Path) -> tuple[list[dict], str, str]:
    """Return (series, status, notes) for a case's run directory.

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
        )

    try:
        summary = parse_rsm(summary_path.read_text(encoding="utf-8"))
    except ValueError as exc:
        return [], "error", f"Failed to parse {summary_path.name}: {exc}"

    vectors_by_id = summary.by_curve_id()
    series: list[dict] = []
    missing = [curve_id for curve_id in case.curve_display if curve_id not in vectors_by_id]
    if missing:
        return (
            [],
            "error",
            f"Parsed {summary_path.name} but it is missing expected curve(s): {', '.join(sorted(missing))}",
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

    return series, "parsed", "Series parsed from a real Flow run."


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
        series, status, notes = _build_series(case, run_dir)
    else:
        series, status, notes = (
            [],
            "deck-ready",
            "Generated artifact metadata is available. Run Flow and attach parsed summary series before treating this as numerical reference data.",
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
    output = artifact_dir / f"{case.key}.json"
    output.write_text(json.dumps(artifact, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return output
