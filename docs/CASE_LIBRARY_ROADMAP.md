# Case Library Roadmap — Where to Find Good Cases

Date: 2026-07-02; Tier 5 + enabler-gap sections added 2026-07-07. Companion to the `add-scenario` skill (`.claude/skills/add-scenario/`). This is the sourcing map for growing the scenario/case library: which cases to add, where their reference data comes from, and what each needs from the engine.

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

## Tier 5 — Decision-insight cases (2026-07-07 review)

Tiers 1–4 are correctness/teaching cases: "does the simulator match the reference?" This tier answers a different question the current library never poses: **"which parameters actually matter for the decision, and can the data even tell you?"** These reuse the existing sensitivity-variant machinery (a variant patching two params at once is already supported) but frame variants as competing interpretations rather than one-at-a-time perturbations. Ordered by (value ÷ effort).

### 5.1 "Matched history, different reserves" — N·c_t ambiguity (no engine gap, cheapest)

Undersaturated depletion under BHP control: pressure decline goes as ΔP ≈ Np/(N·c_t·B), so (OOIP, total compressibility) pairs with equal product produce near-identical pressure and rate history — but recovery factor Np/N differs by construction. Variants: 3 (N, c_t) pairs with equal N·c_t. Teaching point: pressure data alone cannot separate OOIP from compressibility (the classic material-balance non-uniqueness; Dake ch. 3, Havlena-Odeh 1963 — and the existing Havlena-Odeh diagnostics panel displays exactly the ambiguous quantity). Extends the `dep_*` family; analytics all exist. Needs only the history/forecast chart affordance (gap E5 below) to land the "match vs outcome" framing.

### 5.2 "The tornado plot lies" — interaction amplification (no engine gap)

One-at-a-time sensitivities (what a tornado chart encodes) miss interactions. Variant set: base / +A alone / +B alone / +A+B, where each single change moves RF little and the pair moves it a lot. Two candidate physics pairs, both in-envelope:
- **kv/kh × density contrast** in a 2D vertical-section waterflood: raising kv/kh with weak gravity ≈ no RF change; raising density contrast alone modest; both together → Dietz gravity tongue, early breakthrough, large RF loss. References: Dietz (1953); Shook, Li & Lake (1992) scaling groups; Zhou, Fayers & Muggeridge (1997) gravity-viscous regime map.
- **capillary entry pressure × layer contrast**: capillary crossflow is invisible in a homogeneous model and decisive in a layered one (Willhite §5).

### 5.3 "Two fluid models, one calibration point" — PVT representation risk (no engine gap)

Correlation-based vs tabular black-oil PVT, both honoring the same initial bubble point and solution GOR, diverge in Rs(p)/Bo(p) away from the calibration point → different producing GOR and RF in blowdown. This is the in-envelope analog of the compositional-lumping question ("does a 5–7 component match reproduce the full-EOS forecast?") — same epistemic point, no compositional engine needed. Black-oil machinery (correlation + tabular modes) already exists. Reference: OPM Flow run with each PVT table.

### 5.4 "A perfect match is not a proof" — Tavassoli, Carter & King (SPE 86883, 2004) (needs per-cell perm, E1)

The canonical published demonstration that an excellent history match can have wrong parameters and a bad forecast. Their model is deliberately tiny: 2D vertical-section waterflood, alternating high/low-perm layers offset by a fault at mid-length; 3 unknowns (fault throw, k_high, k_low); thousands of parameter triplets match the truth-case water-cut history near-perfectly yet spread widely in forecast. Fits ResSim exactly (two-phase O/W, small grid, fault is pure layer juxtaposition — no fault transmissibility model needed) **except** the offset layer geometry requires per-cell permeability input (gap E1). Implementation: offline sweep (node wasm runner or OPM) selects 3–4 matched-but-divergent triplets; the "truth" history becomes a published-reference-series overlay; variants are the competing matches. Flagship case for the library — this is the one to advertise.

### 5.5 Immiscible WAG cycle study (needs schedule driver E2; hysteresis E4 caveat)

Three-phase 2D vertical section or small 3D pattern, single injector alternating water and gas. Variants: continuous water / continuous gas / WAG 1:1 at 3-, 6-, 12-month cycles / WAG ratio 2:1. Metrics: RF, producing GOR, and the 3D view showing gas override vs water underride — WAG's mechanism (compensating both) is unusually visual. Wasm API already supports mid-run `setWellSchedule` + `setInjectedFluid`; what's missing is a declarative time-schedule in scenario params driven by the worker (E2). **Honesty constraint:** relperm hysteresis (Larsen & Skauge 1998) is the physics that makes real WAG work; without it (E4) ResSim will understate/distort the WAG benefit — label teaching-grade, and pre-run OPM Flow with Killough/Carlson hysteresis on/off so the gap itself becomes content ("what missing physics does to EOR screening"). References: Christensen, Stenby & Skauge (2001) WAG review; Skauge & Larsen hysteresis papers.

### 5.6 PUNQ-S3 — the history-match-uncertainty benchmark

19×28×5, ~1761 active cells — the community-standard benchmark for forecast uncertainty after history matching (Floris et al. 2001), thematically the field-scale big brother of 5.4. As a *live* case it is blocked on analytical aquifer support (Tier 1) and inactive-cell/null-block support (E6) — but it does not need to be live: see Tier 6.1 for the pre-run version, which is unblocked today.

### Compositional lumping study — moved to Tier 6

The original framing (5–7 pseudo-component EOS matched to a full compositional model, compare RF) needs compositional simulation ResSim doesn't have. As a pre-run exhibit it is viable — see Tier 6.2. The in-envelope analog 5.3 carries the same lesson with a live run.

## Tier 6 — Pre-run exhibits (OPM-only or published-data-only, no live sim) (2026-07-07)

**Product concept.** A scenario class that ships entirely precomputed: variants map to bundled artifact keys, charts replay artifact curves, the parameter panel is read-only (documents the deck), and the 3D view is off (or later fed by optional precomputed field snapshots). No live tweaking — and that's acceptable for cases whose teaching point is comparison between pre-defined runs. This removes the engine envelope as a constraint: anything OPM Flow can run offline — or anything with published result datasets — becomes eligible. The `opm-flow-precomputed` artifact plumbing (`src/lib/catalog/opmFlowArtifacts.ts`, `opmFlowReferenceArtifactKeys`, sourceType badges) already exists; the missing piece is a `runPolicy: 'prerun-artifacts'` that skips the worker (gap E7). Prerequisite for all OPM-generated entries: the summary parser stub (TODO.md).

Label these honestly in the UI (existing sourceType badge machinery): "precomputed with OPM Flow yyyy.mm, deck hash …" — provenance and dataset license recorded per case.

| # | Case | Reference data | What it shows | Feasibility |
|---|---|---|---|---|
| 6.1 | **PUNQ-S3 forecast-uncertainty ensemble** | Free Eclipse deck + history data (Imperial College); truth + N history-matched realizations pre-run in OPM (corner-point, AQUCT aquifers supported) | The canonical "many models match, forecasts fan out" benchmark (Floris et al. 2001); pairs with live case 5.4 | Unblocked now; needs E7 + parser; E5 divider makes it land |
| 6.2 | **Compositional lumping study** (original motivating idea) | Full-EOS fluid (e.g. SPE3/SPE5 fluid) vs 5/6/7-component lumped EOS, all pre-run with OPM `flowexp_comp` (experimental — run a pilot deck first); `opm-tests` has a `compositional` suite as starting point | Same surface match at calibration conditions, divergent RF/GOR under gas injection/cycling — representation risk in fluid modeling | Medium; hinges on `flowexp_comp` pilot verdict; fallback: compositional truth vs black-oil-matched proxy, both pre-run |
| 6.3 | **SPE5 miscible WAG** (Killough & Kossack 1987) | `opm-tests/spe5` incl. hysteresis variants (`spe5_ehystr2_0/1`); published comparative results from the paper as overlays | The WAG comparative solution project itself: WAG ratio / cycle-length sensitivities on a published benchmark | Ready decks; needs E7 + parser |
| 6.4 | **WAG hysteresis on/off** | `opm-tests/waghystr` + `spe5_ehystr*`; Larsen & Skauge (1998) | "What missing physics does to EOR screening" — promoted from a 5.5 caveat to its own exhibit; later becomes the reference band for live 5.5 | Ready decks; needs E7 + parser |
| 6.5 | **SPE11 inter-simulator spread** (CO2 storage, 11th CSP) | Published results of all 18 participating groups + analysis scripts (public GitHub, spe.org/csp/spe11); optional own OPM `co2store` run | Even simulators disagree: forecast uncertainty from numerics/gridding, not just parameters; strong CCS audience draw | **No simulation needed at all** — published-data-only; needs E7 |
| 6.6 | **Egg model geological ensemble** | 101 free channelized-perm realizations (Jansen et al. 2014, 4TU repository, Eclipse format); pre-run 10–20 in OPM | Geological uncertainty fan under waterflood; later pairs with a live coarse ResSim run once E1 lands | Unblocked now; needs E7 + parser |
| 6.7 | **Polymer flood screening** | `opm-tests/polymer*` decks; polymer fractional-flow analytical (Pope 1980) computable in the TS analytical layer | First EOR-chemistry content; analytical overlay keeps it referenced | Ready decks; needs E7 + parser |
| 6.8 | **Real-field diagnostics (Norne / Drogon)** | `opm-tests/norne`, `drogon` (open Equinor models); pre-run summary vectors | ResSim's decline/MB analytical toolbox applied to realistic full-field output — screening-tool demo | Ready decks; needs E7 + parser |

Suggested order: 6.5 first (zero simulation work, highest wow), then 6.1, then the WAG pair 6.3/6.4, then 6.2 pilot, then 6.6–6.8.

## Enabler gaps surfaced by Tiers 5–6 (backlog)

| Key | Gap | Unblocks | Effort |
|---|---|---|---|
| E1 | `permMode: 'field'` — per-cell perm arrays in payload + a `setPermeabilityField` wasm setter (core already stores per-cell `perm_x/y/z` vecs; only `uniform`/`perLayer`/`random` are exposed) | 5.4 Tavassoli, SPE10 M1/layer subsets, Egg | small |
| E2 | Declarative time-based well schedule in scenario params (`[{day, wellId, patch}]`) applied by the worker between report steps — wasm `setWellSchedule`/`setInjectedFluid` already exist, worker currently applies schedules once at create | 5.5 WAG, SPE9 | moderate |
| E3 | Per-well injected fluid (currently one global `injected_fluid`) | simultaneous water+gas injector patterns only — NOT needed for single-injector WAG | moderate, defer |
| E4 | Relperm hysteresis (Killough/Carlson) | quantitative WAG; scanning-curve teaching content | large |
| E5 | History/forecast chart affordance: vertical divider + shaded history window, so "all variants match here, diverge there" reads at a glance | 5.1, 5.4, PUNQ-S3 | small (chart layer) |
| E6 | Inactive-cell / null-block support | *live* PUNQ-S3, Norne-like sectors (pre-run versions don't need it) | moderate |
| E7 | `runPolicy: 'prerun-artifacts'` scenario class — no worker run; variants map to bundled artifact keys; read-only parameter panel; 3D off; multi-artifact fan/ensemble chart support | entire Tier 6 | moderate (frontend only) |

OPM summary parser (TODO.md item, `tools/opm_flow/.../artifacts.py` stub) is a prerequisite for the OPM reference bands in 5.3–5.5 and for every OPM-generated Tier 6 exhibit (6.5 excepted — published data only).

## Where to search for more

- **OnePetro / SPE** (onepetro.org is on the tool allowlist) — comparative solution projects, type-curve papers, field case studies with published data tables.
- **OPM repos** (github.com/OPM: `opm-tests`, `opm-data`) — ready decks: SPE1/3/5/9, Norne, model variants.
- **MRST** (SINTEF, mrst.no) — example library mirrors many classic cases with full parameter sets; good for cross-checking case setups.
- **SPE Reservoir Simulation Conference benchmark sessions** — newer CSP-style benchmarks (e.g. 11th CSP on CO2, currently out of engine scope).
- **University groups**: Stanford SUPRI/ECRB, Heriot-Watt, UNICAMP UNISIM, TU Delft (Egg), NTNU (Norne).

## Process per case

1. Verify reference + engine envelope (criteria above). 2. Follow `.claude/skills/add-scenario/`. 3. If OPM-comparable, add a deck via `.claude/skills/opm-reference-pipeline/`. 4. Define tolerance bands in the style of `docs/P4_TWO_PHASE_BENCHMARKS.md`. 5. Record provenance (paper/dataset, units, license) in the scenario file and README inventory.
