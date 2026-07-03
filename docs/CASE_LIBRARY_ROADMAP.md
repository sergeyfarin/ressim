# Case Library Roadmap — Where to Find Good Cases

Date: 2026-07-02. Companion to the `add-scenario` skill (`.claude/skills/add-scenario/`). This is the sourcing map for growing the scenario/case library: which cases to add, where their reference data comes from, and what each needs from the engine.

## Selection criteria

A good ResSim case has, in priority order:

1. **An independent reference** — analytical solution, published benchmark results, or an OPM Flow run (see `.claude/skills/opm-reference-pipeline/`). No reference → teaching-only, label it honestly.
2. **Physics inside the engine's envelope** — 3D Cartesian grid, two-phase O/W (validated), three-phase O/W/G (experimental), black-oil PVT, Peaceman wells, gravity, Brooks-Corey capillary. **Not supported:** radial grids/LGR, aquifer models, well schedules, compositional, thermal, polymer/chemical EOR, dual porosity, horizontal wells.
3. **Browser-scale grid** — comfortably ≤ ~30k cells for interactive IMPES runs.
4. **One clear teaching point** per sensitivity dimension.

## Tier 1 — Analytical-backed, near-term (cheapest, highest correctness value)

| Case | Reference | Engine gap | Notes |
|---|---|---|---|
| Gas-cap depletion / blowdown | p/z material balance; Havlena-Odeh with gas-cap ratio `m` | none (black-oil machinery exists) | Already ROADMAP 5.1; best next case per `docs/COMPARISON_TOOLBOX_REVIEW_2026-07-01.md` §4 |
| Aquifer-supported depletion | Fetkovich aquifer; Carter-Tracy; van Everdingen-Hurst | needs an aquifer boundary model (new physics, moderate) | OPM supports AQUFETP/AQUCT → OPM cross-check possible |
| Well test / pressure transient (drawdown, buildup, Horner) | radial diffusivity solution, Horner/MDH | none for a first version (fine Cartesian grid near well + Peaceman) | New analytical module; classic RE teaching content |
| Unfavorable-M waterflood / fingering sensitivity | BL with high M (stability limit discussion) | none | Parameter sensitivity on `wf_bl1d` family |
| Directly-simulated quarter five-spot vs Craig correlation | Craig (1971) correlation vs own simulation | none | Shows where the correlation's assumptions break — strong teaching contrast |
| Dykstra-Parsons with vertical communication sweep | D-P (1950) + Warren-Root style kv/kh blending | none | Extends `sweep_vertical`; ROADMAP 5.2 |

## Tier 2 — SPE Comparative Solution Projects (published simulator results as reference)

Source: SPE papers (OnePetro), decks in OPM's `opm-tests`/`opm-data` GitHub repos (ready-to-run, METRIC/FIELD variants).

| Case | Fit | Why / why not |
|---|---|---|
| SPE1 (Odeh 1981, black-oil gas injection, 300 cells) | in progress | Scenario exists; remaining: tabular SCAL, surface-rate control, quantitative acceptance criteria |
| SPE2 (Weinstein 1986, coning) | poor | Radial grid — not supported |
| SPE3 (Kenyon 1987, gas cycling) | poor | Compositional — not supported |
| SPE9 (Killough 1995, 9000 cells, heterogeneous black-oil, 25 wells) | medium | Grid size OK; needs well schedules (dev gap); good stress test once SPE1 closes |
| SPE10 Model 1 (Christie & Blunt 2001, 2D 2000 cells, gas-oil) | good | Small, two-phase, published fine-grid reference; upscaling teaching content |
| SPE10 Model 2 (1.1M cells) | poor as-is | Use published single-layer subsets (e.g. layer 36/59) as heterogeneity stress cases instead |

## Tier 3 — Public field datasets (real-data flavor)

| Dataset | Source | Fit |
|---|---|---|
| Egg Model (TU Delft) | Jansen et al. 2014, data DOI 10.4121/uuid:916c86cd-3558-4672-829a-105c62985ab2 | **Best field-like candidate**: ~18.5k active cells, 8 inj / 4 prod waterflood, widely benchmarked |
| Volve (Equinor) | data.equinor.com (open license) | Full field incl. Eclipse model; too big to run whole — use for decline/MB diagnostics against real production data |
| Norne (Equinor/NTNU) | opm-data GitHub, IO Center NTNU | Real full-field black-oil benchmark; deck runs in OPM → offline reference only |
| UNISIM-I/II (UNICAMP) | unisim.cepetro.unicamp.br | Synthetic-from-real (Namorado); benchmark community standard |
| Brugge (TNO) | SPE benchmark | Waterflood optimization benchmark, 20 wells, ~44k cells (coarsenable) |

Approach for field-scale data: don't run the full model in the browser. Either (a) extract a sector/coarsened model that IMPES handles, with the OPM full-model run as reference, or (b) use production data only, compared against ResSim's decline/material-balance analytical diagnostics (Arps/Fetkovich/Havlena-Odeh) — that is itself a strong screening-tool demo.

## Tier 4 — Textbook & paper sensitivities (teaching depth)

- Dake, *Fundamentals of Reservoir Engineering* — worked examples for material balance, Welge, decline; parameters are fully specified and reproducible.
- Willhite, *Waterflooding* (SPE Textbook) — pattern flood examples, injectivity, D-P worked cases.
- Craig, *The Reservoir Engineering Aspects of Waterflooding* (SPE Monograph 3) — five-spot experimental sweep data behind `sweep_areal`.
- Lake, *Enhanced Oil Recovery* — fractional-flow theory extensions (polymer/Koval are future physics).
- Ahmed, *Reservoir Engineering Handbook* — decline/MB example problems.
- Original papers already cited in-code (Buckley-Leverett 1942, Welge 1952, Dykstra-Parsons 1950, Stiles 1949, Dietz 1965, Fetkovich 1980, Arps 1945, Havlena-Odeh 1963, Odeh 1981, Peaceman 1978) — each contains tables/curves that can become `publishedReferenceSeries` overlays.

## Where to search for more

- **OnePetro / SPE** (onepetro.org is on the tool allowlist) — comparative solution projects, type-curve papers, field case studies with published data tables.
- **OPM repos** (github.com/OPM: `opm-tests`, `opm-data`) — ready decks: SPE1/3/5/9, Norne, model variants.
- **MRST** (SINTEF, mrst.no) — example library mirrors many classic cases with full parameter sets; good for cross-checking case setups.
- **SPE Reservoir Simulation Conference benchmark sessions** — newer CSP-style benchmarks (e.g. 11th CSP on CO2, currently out of engine scope).
- **University groups**: Stanford SUPRI/ECRB, Heriot-Watt, UNICAMP UNISIM, TU Delft (Egg), NTNU (Norne).

## Process per case

1. Verify reference + engine envelope (criteria above). 2. Follow `.claude/skills/add-scenario/`. 3. If OPM-comparable, add a deck via `.claude/skills/opm-reference-pipeline/`. 4. Define tolerance bands in the style of `docs/P4_TWO_PHASE_BENCHMARKS.md`. 5. Record provenance (paper/dataset, units, license) in the scenario file and README inventory.
