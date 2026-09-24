---
name: opm-reference-pipeline
description: Generate, run, and parse OPM Flow reference simulations used as offline ground truth for ResSim scenarios (decks, artifacts, comparison). Use when working under tools/opm_flow/, adding OPM comparison data for a scenario, or benchmarking IMPES/FIM against OPM Flow.
---

# OPM Flow Reference Pipeline

OPM Flow is the industrial open-source simulator used as offline ground truth. The browser never runs OPM — precomputed JSON artifacts are committed into the frontend catalog. Every committed artifact is `status: "parsed"`, unit-checked and provenance-stamped (#20, 2026-09-24).

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

- `tools/opm_flow/opm_flow_tool/cases.py` — `OpmCase` definitions. Eleven cases as of 2026-09-24: `wf_bl1d`, `spe1_gas_injection`, `gas_drive`, `wf_gravity`, `wf_numerics` / `wf_numerics_fine`, `dep_gas_pz` / `dep_gas_pz_geopressured`, `gas_injection`, `dep_pvt_correlation` / `dep_pvt_lab_report`. A pair drawn on one chart needs distinct `curveKey`s (`opm-fine-`, `opm-geo-`, `opm-lab-` prefixes) and says its rung in every label.
  - **Inline vs committed decks.** Most decks are text in `cases.py`. The last three read a committed deck (`deck_source`) that `src/lib/ressim/src/tests/opm_small_direct.rs` generates from the same ResSim simulator the scenario configures — prefer this for a new case: relperm and PVT are sampled from ResSim's own functions, so the deck cannot disagree with the engine about inputs, and `compare_small_direct.py` grades ResSim cell by cell against the same Flow run. `flow_args` carries options the deck must run with (`--enable-gravity=false` for small-direct).
  - `curve_display` maps a summary vector (mnemonic, or `"MNEMONIC:NAME"` for well vectors) to `{panelKey, curveKey, label}`.
- **The vector contract** — `tools/opm_flow/opm_flow_tool/vectors.py` (#20). Each mnemonic has the METRIC unit Flow prints for it, the panels it may be drawn on, and a token (`oil`, `water`, `gas`, `injection`, `pressure`, …) its curve key *and* label must contain. `build_artifact()` checks the `.RSM` unit row against it: a non-METRIC deck, a TIME column not in DAYS, a unit mismatch, or a mapped mnemonic with no contract makes the artifact `status: "error"` and `build-artifacts` exits 1. Adding a vector means adding its row here first. (Before the contract, four waterflood cases declared `m3/day` while Flow wrote `SM3/DAY`, and `wf_bl1d` drew Sm³/day rates on the water-cut panel.)
- **x-axis mapping.** Three independent mappings, each from a declared vector:
  - `cumulative_injection_curve` (FVIT, **reservoir** RM3) + `pore_volume_m3` -> `xAxis.pvi` / `cumulativeInjectionM3`. PVI only. Flow's reservoir-volume vectors convert at the field-average pressure (RESV convention; FVIT = ∫ FGIR·Bg(FPR) dt to 5e-5 on `gas_injection`), and so does the simulation's PVI since #43 (`total_injection_resv`). The engine's `total_injection_reservoir` converts at the injector cell instead and is **not** comparable to FVIT for gas.
  - `cumulative_surface_injection_curve` (FWIT/FGIT, **surface** SM3) -> `xAxis.cumulativeInjectionSm3`, the only source for the cumulative-injection axis, because ResSim's cum-injection is surface volume. For gas the two bases differ by Bg; never feed one axis from the other.
  - `cumulative_gas_curve` (FGPT) -> `xAxis.cumulativeGasSm3`, for p/z charts.
  The frontend's `mapReferenceTimesToXAxis` returns `null` — the series is **dropped**, not misplaced — on any axis the artifact publishes no mapping for.
- `tools/opm_flow/opm_flow_tool/summary.py` — hand-rolled `.RSM` text-summary parser (records the TIME unit too). Fixed-width columns, uniform gap derived from the mnemonic row's own token spacing (not from dividing separator width evenly — that fails when there's non-column margin). Header/data separator is found by scanning *forward* from the `TIME` row, not by taking the first dashed line in the page. See the module docstring for the full validated layout.
- `tools/opm_flow/opm_flow_tool/artifacts.py::build_artifact()` — writes `src/lib/catalog/opm-flow-results/<case>.json`, **schema 2**: each series carries `mnemonic` and verified `unit`; `provenance` records `deckSource`, `flowArgs`, the `replay` command and `origin` (where the model inputs come from and their licence — record this *before* bundling any third-party-derived deck). `deckHash` is the SHA-256 of the deck text. Never raises; a failure degrades to `status: "error"` with the reason in `notes`.
- Frontend: `src/lib/catalog/opmFlowArtifacts.ts::resolveScenarioReferenceSeries()` renders series only when `status === 'parsed'`. Scenarios opt in with `referenceSources: [{ kind: 'opm-flow', artifactKeys: [...] }]` — no implicit scenarioKey match.

### Gates

- `tools/opm_flow/tests` (pytest; PR CI step "Run OPM artifact pipeline tests"): the unit contract, every case's mappings against it, every committed artifact `parsed`, schema-current, attributed, and generated from the deck its case holds now (deck edited without a Flow re-run fails).
- `src/lib/catalog/opmReferenceWiring.test.ts` (Vitest, also on master): every artifact is declared by its scenario, lands on a panel that scenario draws, can be placed on the axis its chart opens on, and has one unit per panel; committed-deck artifacts re-hash their deck file.

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

## Adding a case

1. Prefer a small-direct case: add it to `CASES`/`build` in `opm_small_direct.rs`, mirroring the worker's `configureReservoirSimulator` order, and regenerate the decks (`OPM_SMALL_DECK_DIR=$PWD/opm/reference-decks/small-direct cargo test --release --manifest-path src/lib/ressim/Cargo.toml opm_small_direct_write_decks -- --ignored`). Any scenario input that is not a scalar (a PVT table) goes in a committed fixture both sides assert against (`dep-pvt-tables.json`).
2. Add the `OpmCase` (deck via `_committed_deck`), run `run-flow <key>` then `build-artifacts <key>`, register the JSON in `opmFlowArtifacts.ts`, declare it on the scenario, and extend `EXPECTED` in `opmReferenceWiring.test.ts`.
3. Grade ResSim against the same run with `compare_small_direct.py` and record it in `opm/reference-decks/small-direct/README.md`.

## Units

Enforced, not advisory: see the vector contract above. OPM decks are METRIC (bar, m, Sm³/day); `vectors.py` is the single statement of what unit each vector must carry.

## FIM-vs-OPM solver comparison (separate use case)

For solver-convergence benchmarking (not product artifacts): branch `origin/fim-opm-continuation-plan` has `opm/reference-decks/` (gas-rate 10x10x3, water-medium cases, with DT4/DT16 variants) and `scripts/opm-ressim-compare.sh`. OPM source checkouts live under `OPM/` at repo root for algorithm reference. Useful flow flags for diagnosis: `--solver-verbosity=3 --time-step-verbosity=3`. OPM's benchmark to beat: ~2.5 Newton iterations/step, zero cut timesteps on SPE1-class cases.

Related docs: `docs/OPM_FLOW_MINIMAL_MAPPING.md` (solver mapping), `docs/20260426.md` (track-OPM vs originality analysis).
