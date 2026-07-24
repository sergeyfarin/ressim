---
name: opm-reference-pipeline
description: Generate, run, and parse OPM Flow reference simulations used as offline ground truth for ResSim scenarios (decks, artifacts, comparison). Use when working under tools/opm_flow/, adding OPM comparison data for a scenario, or benchmarking IMPES/FIM against OPM Flow.
---

# OPM Flow Reference Pipeline

OPM Flow is the industrial open-source simulator used as offline ground truth. The browser never runs OPM — precomputed JSON artifacts are committed into the frontend catalog. The summary parser gap is **closed** (2026-07-16) — both committed artifacts are now `status: "parsed"` with real series data. See "Known deck-physics caveat" below for what's still open.

## Requirements

- `flow` binary installed locally; check `flow --version`. Not guaranteed present in every sandbox, but was available and used for the 2026-07-16 real runs (`flow 2026.04`) — don't assume it's absent without checking.
- Python via `uv` (project convention for all Python tooling). Dev dependency group `dev` (pytest) — run tests with `uv run pytest` from `tools/opm_flow/`.

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

- `tools/opm_flow/opm_flow_tool/cases.py` — `OpmCase` deck definitions, now including a `curve_display` map (mnemonic, or `"MNEMONIC:NAME"` for well/group vectors → `{panelKey, curveKey, label}` matching the frontend's existing panel-key conventions). Currently only `wf_bl1d` and `spe1_gas_injection`. The SPE1 deck contains the **real tabulated SWOF/SGOF** that the ResSim scenario still only Corey-approximates (known confound for match quality).
- `tools/opm_flow/opm_flow_tool/summary.py` — hand-rolled `.RSM` text-summary parser. Fixed-width columns, uniform gap derived from the mnemonic row's own token spacing (not from dividing separator width evenly — that fails when there's non-column margin). Header/data separator is found by scanning *forward* from the `TIME` row, not by taking the first dashed line in the page (a title line is flanked by two decorative separators that come first). See the module docstring for the full validated layout.
- `tools/opm_flow/opm_flow_tool/artifacts.py::build_artifact()` — writes `src/lib/catalog/opm-flow-results/<case>.json`. Looks for `<run-root>/<case-key>/*.RSM`; parses it via `summary.py` if present, mapping vectors through the case's `curve_display`. Never raises — a parse failure or missing expected curve degrades to `status: "error"` with the reason in `notes`, so one bad case can't crash `build-artifacts all`.
- Artifact status model: `deck-ready → flow-run → parsed → error`. **As of 2026-07-16, both committed artifacts are `parsed`** with real series from actual `flow 2026.04` runs.
- Frontend: `src/lib/catalog/opmFlowArtifacts.ts::getOpmFlowPublishedReferenceSeries()` renders series **only when `status === 'parsed'`**. Scenarios opt in via `opmFlowReferenceArtifactKeys`.

## Deck-physics caveat — RESOLVED 2026-07-24 (COMPDAT item shift)

Both decks put the wellbore *radius* in `COMPDAT` **item 8** (connection transmissibility factor)
instead of item 9 (wellbore *diameter*). Flow then used that number as the CF verbatim, choking every
connection by ~2 orders of magnitude. Symptoms: `wf_bl1d` FOPR ~1e-4 sm3/day with FWPR tracking FWIR
from the first timestep; `spe1_gas_injection` FOPR ~24 sm3/day with **both** wells pinned to their
BHP limits from day 1. Fixed by defaulting items 7-8 (`2*`) and passing the diameter in item 9
(`0.2` m for `wf_bl1d`, `0.1524` m for SPE1). **When adding a deck, remember COMPDAT item 9 is a
diameter, and sanity-check that a rate-controlled well actually holds its target rate.**

Fixed in the same pass for SPE1 (it was depth-degenerate and Case-2-flavoured): `TOPS` 0 → 2537.46 m
with matching `EQUIL`/`RSVD`/`WELSPECS` datum depths, added `EQLDIMS`, and added `DRSDT 0` (Case 1
has no re-dissolution, matching the scenario's `gasRedissolutionEnabled: false`).

Post-fix validation — the generated deck now tracks the canonical `OPM/opm-common/tests/SPE1CASE1.DATA`
(run with `flow SPE1CASE1.DATA --output-dir=.`, WELLDIMS raised to 4 so the RFT wells load) within
~4 % on FOPR and ~8 % on late-time GOR, with the same ~day-1000 BHP-floor decline onset. `wf_bl1d`
now shows a proper Buckley-Leverett front: FOPR flat ~70 sm3/day to breakthrough at ~14.5 d
(≈0.55 PV, as expected for a 1920 m³ PV at ~70 m³/day), then declining as FWPR rises.

Also fixed in the 2026-07-16 pass in the same pass, two pre-existing deck bugs unrelated to parsing that were silently blocking any real `flow` run of these two cases: `wf_bl1d`'s `PVDO` table had non-monotonic (flat) Bo values, which Flow rejects — now uses `c_o = 1e-5/bar` matching the ResSim `wf_bl1d` scenario's own declared compressibility; `spe1_gas_injection`'s `TABDIMS` declared `NTSFUN=2, NTPVT=15` while every PVT/SCAL keyword only supplied one region's table — corrected to `1 1`.

## Next after the parser (Phase C)

Add decks for `gas_injection` and `gas_drive` (no OPM ground truth exists for them today), then define quantitative acceptance bands vs OPM (analogous to `docs/P4_TWO_PHASE_BENCHMARKS.md`). The `wf_bl1d` deck-physics blocker on this is cleared (see above), so the bands are now worth defining.

## Units warning

OPM decks are in METRIC units matching ResSim conventions (bar, m, m³/day). When parsing summaries, verify unit strings from the summary file itself — don't assume. Record deck + flow version in artifact provenance.

## FIM-vs-OPM solver comparison (separate use case)

For solver-convergence benchmarking (not product artifacts): branch `origin/fim-opm-continuation-plan` has `opm/reference-decks/` (gas-rate 10x10x3, water-medium cases, with DT4/DT16 variants) and `scripts/opm-ressim-compare.sh`. OPM source checkouts live under `OPM/` at repo root for algorithm reference. Useful flow flags for diagnosis: `--solver-verbosity=3 --time-step-verbosity=3`. OPM's benchmark to beat: ~2.5 Newton iterations/step, zero cut timesteps on SPE1-class cases.

Related docs: `docs/OPM_FLOW_MINIMAL_MAPPING.md` (solver mapping), `docs/20260426.md` (track-OPM vs originality analysis).
