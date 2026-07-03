---
name: opm-reference-pipeline
description: Generate, run, and parse OPM Flow reference simulations used as offline ground truth for ResSim scenarios (decks, artifacts, comparison). Use when working under tools/opm_flow/, adding OPM comparison data for a scenario, or benchmarking IMPES/FIM against OPM Flow.
---

# OPM Flow Reference Pipeline

OPM Flow is the industrial open-source simulator used as offline ground truth. The browser never runs OPM — precomputed JSON artifacts are committed into the frontend catalog. This pipeline is **real but incomplete at exactly one step** (the summary parser); see "Known gap" below.

## Requirements

- `flow` binary installed locally (proven working on the author's machine; check `flow --version`). Not available in typical sandboxes — deck generation and parser development still work; fresh runs don't.
- Python via `uv` (project convention for all Python tooling).

## Commands (from repo root)

```bash
# generate an Eclipse-style deck from a ResSim case definition
uv run --directory tools/opm_flow python -m opm_flow_tool.cli generate-deck wf_bl1d --output tmp/opm-flow-runs/decks/wf_bl1d.DATA
# run flow on a case
uv run --directory tools/opm_flow python -m opm_flow_tool.cli run-flow wf_bl1d
# build frontend artifacts (also: pnpm run opm:artifacts / opm:deck)
uv run --directory tools/opm_flow python -m opm_flow_tool.cli build-artifacts all
```

## How the pieces connect

- `tools/opm_flow/opm_flow_tool/cases.py` — `OpmCase` deck definitions. Currently only `wf_bl1d` and `spe1_gas_injection`. The SPE1 deck contains the **real tabulated SWOF/SGOF** that the ResSim scenario still only Corey-approximates (known confound for match quality).
- `tools/opm_flow/opm_flow_tool/artifacts.py::build_artifact()` — writes `src/lib/catalog/opm-flow-results/<case>.json`.
- Artifact status model: `deck-ready → flow-run → parsed → error`.
- Frontend: `src/lib/catalog/opmFlowArtifacts.ts::getOpmFlowPublishedReferenceSeries()` renders series **only when `status === 'parsed'`**. Scenarios opt in via `opmFlowReferenceArtifactKeys`.

## Known gap (highest-leverage task in this area)

`build_artifact()` never parses flow output: it unconditionally writes `"series": []`, `"status": "deck-ready"`. Both committed artifacts are stubs, so **no real OPM number has ever reached the UI** despite full type support downstream. To close it (plan: `docs/COMPARISON_TOOLBOX_REVIEW_2026-07-01.md` §5 Phase A):

1. Parse flow's summary output from `tmp/opm-flow-runs/<case>/` — simplest: hand-rolled `.RSM` reader (zero new deps); alternative: `resdata`/`opm` Python bindings for `.SMSPEC`/`.UNSMRY`. Curve keys to extract are already listed in `supported_curves` (FOPR, FWPR, FGIR, WBHP, WGOR, …).
2. Populate `OpmFlowArtifactSeries[]`, set `status: 'parsed'`, keep provenance/unit metadata.
3. Unit-test the parser against a committed fixture file.
4. Regenerate `wf_bl1d.json` and `spe1_gas_injection.json` on a machine with `flow`.
5. Add an integrity test: every scenario's `opmFlowReferenceArtifactKeys` resolves to an artifact whose `scenarioKey` matches; fail if a committed artifact regresses from `parsed` to `deck-ready`.

After that: add decks for `gas_injection` and `gas_drive` (no OPM ground truth exists for them today), then define quantitative acceptance bands vs OPM (analogous to `docs/P4_TWO_PHASE_BENCHMARKS.md`).

## Units warning

OPM decks are in METRIC units matching ResSim conventions (bar, m, m³/day). When parsing summaries, verify unit strings from the summary file itself — don't assume. Record deck + flow version in artifact provenance.

## FIM-vs-OPM solver comparison (separate use case)

For solver-convergence benchmarking (not product artifacts): branch `origin/fim-opm-continuation-plan` has `opm/reference-decks/` (gas-rate 10x10x3, water-medium cases, with DT4/DT16 variants) and `scripts/opm-ressim-compare.sh`. OPM source checkouts live under `OPM/` at repo root for algorithm reference. Useful flow flags for diagnosis: `--solver-verbosity=3 --time-step-verbosity=3`. OPM's benchmark to beat: ~2.5 Newton iterations/step, zero cut timesteps on SPE1-class cases.

Related docs: `docs/OPM_FLOW_MINIMAL_MAPPING.md` (solver mapping), `docs/20260426.md` (track-OPM vs originality analysis).
