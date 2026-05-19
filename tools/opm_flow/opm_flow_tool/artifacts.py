from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
from datetime import datetime, timezone
from pathlib import Path

from .cases import CASES, OpmCase

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


def build_artifact(case: OpmCase, artifact_dir: Path = DEFAULT_ARTIFACT_DIR, generated_at: str | None = None) -> Path:
    artifact_dir.mkdir(parents=True, exist_ok=True)
    generated_at = generated_at or datetime.now(timezone.utc).replace(microsecond=0).isoformat()
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
        "series": [],
        "status": "deck-ready",
        "notes": "Generated artifact metadata is available. Run Flow and attach parsed summary series before treating this as numerical reference data.",
    }
    output = artifact_dir / f"{case.key}.json"
    output.write_text(json.dumps(artifact, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return output
