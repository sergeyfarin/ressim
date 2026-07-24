# Case Library Roadmap — Where to Find Good Cases

Date: 2026-07-02; Tier 5 + enabler-gap sections added 2026-07-07; **Tier 7 gap audit + Tier 5/6/E-status reconciliation 2026-07-24**. Companion to the `add-scenario` skill (`.claude/skills/add-scenario/`). This is the sourcing map for growing the scenario/case library: which cases to add, where their reference data comes from, and what each needs from the engine.

## Selection criteria

A good ResSim case has, in priority order:

1. **An independent reference** — analytical solution, published benchmark results, or an OPM Flow run (see `.claude/skills/opm-reference-pipeline/`). No reference → teaching-only, label it honestly.
2. **Physics inside the engine's envelope** — 3D Cartesian grid, two-phase O/W (validated), three-phase O/W/G (experimental), black-oil PVT, Peaceman wells, gravity, Brooks-Corey capillary. **Not supported:** radial grids/LGR, aquifer models, well schedules, compositional, thermal, polymer/chemical EOR, dual porosity, horizontal wells.
3. **Browser-scale grid** — comfortably ≤ ~30k cells for interactive IMPES runs.
4. **One clear teaching point** per sensitivity dimension.

## Tier 1 — Analytical-backed, near-term (cheapest, highest correctness value)

None of these had shipped as of the 2026-07-24 audit; each now carries its Tier 7 ID, which is what `TODO.md` and `ROADMAP.md` reference.

| Case | ID | Reference | Engine gap | Notes |
|---|---|---|---|---|
| Gas-cap depletion / blowdown | T7.2 | p/z material balance; Havlena-Odeh with gas-cap ratio `m` | none (black-oil machinery exists) | ROADMAP 5.1; best next case per `docs/COMPARISON_TOOLBOX_REVIEW_2026-07-01.md` §4 |
| Aquifer-supported depletion | T7.3 | Fetkovich aquifer; Carter-Tracy; van Everdingen-Hurst | needs an aquifer boundary model (E9, new physics) | ROADMAP 5.4; OPM supports AQUFETP/AQUCT → OPM cross-check possible |
| Well test / pressure transient (drawdown, buildup, Horner) | T7.1 | radial diffusivity solution, Horner/MDH | none for a first version (fine Cartesian grid near well + Peaceman) | New analytical module (E10); classic RE teaching content |
| Unfavorable-M waterflood / fingering sensitivity | T7.5 | BL with high M (stability limit discussion); Koval (1963) | none | Parameter sensitivity on `wf_bl1d` family |
| Directly-simulated quarter five-spot vs Craig correlation | T7.11 | Craig (1971) correlation vs own simulation | none | Shows where the correlation's assumptions break; pairs with the grid-orientation study |
| Dykstra-Parsons with vertical communication sweep | — | D-P (1950) + Warren-Root style kv/kh blending | none | Extends `sweep_vertical`; ROADMAP 5.3 |

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

**Status (2026-07-24 audit):** 5.1, 5.2 and 5.3 have **shipped**; 5.4–5.6 remain open. See the status line under each heading.

### 5.1 "Matched history, different reserves" — N·c_t ambiguity (no engine gap, cheapest)

**DONE** — shipped as `src/lib/catalog/scenarios/dep_nct.ts` ("Matched History, Different Reserves"), dimension `nct_ambiguity`, three equal-N·c_t variants.

Undersaturated depletion under BHP control: pressure decline goes as ΔP ≈ Np/(N·c_t·B), so (OOIP, total compressibility) pairs with equal product produce near-identical pressure and rate history — but recovery factor Np/N differs by construction. Variants: 3 (N, c_t) pairs with equal N·c_t. Teaching point: pressure data alone cannot separate OOIP from compressibility (the classic material-balance non-uniqueness; Dake ch. 3, Havlena-Odeh 1963 — and the existing Havlena-Odeh diagnostics panel displays exactly the ambiguous quantity). Extends the `dep_*` family; analytics all exist. Needs only the history/forecast chart affordance (gap E5 below) to land the "match vs outcome" framing.

### 5.2 "The tornado plot lies" — interaction amplification (no engine gap)

**DONE (kv/kh × density contrast pair only)** — shipped as `src/lib/catalog/scenarios/wf_tornado.ts`, dimension `interaction` (base / kv only / Δρ only / both). The **capillary entry pressure × layer contrast** pair below was never built and is carried forward as Tier 7.16.

One-at-a-time sensitivities (what a tornado chart encodes) miss interactions. Variant set: base / +A alone / +B alone / +A+B, where each single change moves RF little and the pair moves it a lot. Two candidate physics pairs, both in-envelope:
- **kv/kh × density contrast** in a 2D vertical-section waterflood: raising kv/kh with weak gravity ≈ no RF change; raising density contrast alone modest; both together → Dietz gravity tongue, early breakthrough, large RF loss. References: Dietz (1953); Shook, Li & Lake (1992) scaling groups; Zhou, Fayers & Muggeridge (1997) gravity-viscous regime map.
- **capillary entry pressure × layer contrast**: capillary crossflow is invisible in a homogeneous model and decisive in a layered one (Willhite §5).

### 5.3 "Two fluid models, one calibration point" — PVT representation risk (no engine gap)

**DONE (live case)** — shipped as `src/lib/catalog/scenarios/dep_pvt.ts`, dimension `pvt_model`. The OPM reference deck for this case is still missing (`TODO.md`).

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

**Status (2026-07-24 audit): E7 has landed and the OPM summary parser is done.** `capabilities.runMode: 'prerun-artifacts'` exists in `src/lib/catalog/scenarios.ts` (validated: such scenarios must set `default3DScalar: null`), and `src/lib/catalog/scenarios/wf_bl1d_opm.ts` is the shipped precedent. Both committed artifacts are `status: "parsed"` with real series. **Every Tier 6 entry below is therefore unblocked on plumbing and waits only on data curation** — except the multi-artifact *ensemble/fan* rendering needed by 6.1 and 6.6, which is Tier 7 enabler E8.

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

## Tier 7 — Gap audit of the shipped library (2026-07-24)

Tiers 1–6 were written as *sourcing* lists. This tier is the complement: an audit of what the **14 shipped scenarios** and the **four shipped analytical modules** (`fractionalFlow`, `materialBalance`, `depletionAnalytical`, `sweepEfficiency`) do *not* cover. Items carry stable IDs `T7.n`; `TODO.md` and `ROADMAP.md` reference these IDs rather than restating them.

Two structural findings frame the whole tier:

- **Capillarity is shipped, validated and never used.** `capillaryEnabled: false` in all 14 scenarios; only `wf_tornado` and `spe1_gas_injection` enable gravity. An entire validated physics module is invisible to users.
- **Every uncertainty case in the library renders as discrete labelled curves.** There is no ensemble/fan primitive, so no case can pose "P10/P50/P90" or "N realizations match, forecasts diverge". This blocks all of §7.D, not physics.

### 7.A — Analytical methods missing from `src/lib/analytical/`

| ID | Case | Reference | Engine gap | Effort |
|---|---|---|---|---|
| T7.1 | **Well test — drawdown / buildup / Horner** | Line-source (Ei) solution; Horner (1951); Earlougher (1977); Bourdet derivative | none (fine Cartesian near-well + existing Peaceman) | new `wellTest.ts` module, moderate |
| T7.2 | **Dry-gas p/z material balance + gas-cap blowdown** | p/z straight line; water-drive p/z curvature (Agarwal et al. 1965); Havlena-Odeh with `m` | none — `materialBalance.ts` already carries `m` and `driveIndex_gasCap` but has no p/z path and **no scenario exercises them** | extends existing module, small |
| T7.3 | **Aquifer influx — Fetkovich / Carter-Tracy / van Everdingen-Hurst** | Fetkovich (1980); Carter-Tracy (1960); van Everdingen & Hurst (1949) | **new engine boundary model** | large |
| T7.4 | **Capillary/gravity equilibrium — Leverett J-function transition zone** | Leverett (1941); hydrostatic Pc–Sw equilibrium | none (capillary module shipped, unused) | small |
| T7.5 | **Koval correction to Buckley-Leverett** | Koval (1963) | none | small; gives the high-M `wf_bl1d` rungs an honest reference instead of a BL curve everyone knows is wrong there |

T7.1 is the largest missing pillar of classical reservoir engineering in the product, and it doubles as a numerics case (how much near-well refinement before the Horner slope recovers the input `k`?). T7.3 unblocks T7.2's water-drive variant, T7.17, and live PUNQ-S3 (5.6).

### 7.B — Published benchmarks not yet covered

| ID | Case | Blocker |
|---|---|---|
| T7.6 | **SPE10 Model 1** (Christie & Blunt 2001) — 2D gas-oil, 2000 cells, published fine-grid reference; upscaling-error content | E1 single-run path |
| T7.7 | **SPE10 Model 2 single layers (36 / 59)** as heterogeneity stress cases | E1 single-run path |
| T7.8 | **SPE9** (Killough 1995) | E2 well schedule |
| T7.9 | **Tavassoli, Carter & King (SPE 86883)** — Tier 5.4, the flagship "perfect match, wrong forecast" case | E1 single-run path only; **nothing started** |
| T7.10 | **Tier 6 pre-run exhibits** — 6.5 SPE11 (zero simulation), 6.1 PUNQ-S3, 6.3/6.4 SPE5 WAG ± hysteresis, 6.6 Egg | none for 6.5/6.3/6.4; E8 for the ensemble cases (6.1, 6.6) |

6.5 remains the cheapest high-impact exhibit in the repo: published data only, no simulation, and E7 now exists.

### 7.C — Sensitivities with no engine gap (cheapest new content)

| ID | Case | Teaching point |
|---|---|---|
| T7.11 | **Grid-orientation effect** — five-spot on a parallel vs. diagonal grid at high M | The *grid* is a forecast variable. Todd, O'Dell & Hirasaki (1972); Yanosik & McCracken (1979). **Attempted 2026-07-24 and NOT shipped — see "T7.11 negative result" below. Needs multi-well pattern support, not just existing params.** |
| T7.12 | **Numerical vs. physical dispersion ladder** | `wf_bl1d` already has a grid dimension; what is missing is the framing — front smearing from Δx is indistinguishable from a physically wrong `n_o` |
| T7.13 | **Timestep × solver (IMPES vs FIM on one case)** | `docs/BLACK_OIL_VALIDATION.md` §2 records a ~10 % liberated-gas disagreement between two shipped solver paths on the same column. Today that is a dev-only defect note; it is also the most honest available demonstration that solver choice is a forecast uncertainty |
| T7.14 | **Joint relperm-endpoint uncertainty** | `S_or` and Corey exponents exist as *separate* `wf_bl1d` dimensions, never as a joint RF fan with "would SCAL data resolve this?" framing |
| T7.15 | **Well count / pattern density** | The library currently poses no development-*decision* question at all |

### 7.D — Combined-uncertainty cases (highest impact; none shipped beyond `wf_tornado`)

| ID | Case | Depends on |
|---|---|---|
| T7.16 | **Capillary entry pressure × layer contrast** — capillary crossflow is invisible homogeneous, decisive layered (Willhite §5). The unbuilt half of Tier 5.2 | T7.4 (turn capillarity on somewhere first) |
| T7.17 | **Aquifer strength × OOIP** — the two-parameter `dep_nct`: near-perfect history equifinality, factor-2 reserves spread | T7.3 |
| T7.18 | **Relperm endpoints × heterogeneity (V_DP)** — sweep loss attributable to rock curves vs. geology is not separable from production data | none |
| T7.19 | **Ensemble / fan-chart affordance** — N-realization P10/P50/P90 band plus "history shaded, forecast diverging" | E8 (below). **Prerequisite for all of 7.D and for Tier 6.1/6.6** |

### Delivery record (2026-07-24)

**T7.4 — SHIPPED (waterflood half).** `src/lib/catalog/scenarios/wf_capillary.ts` + `wf_capillary.test.ts`. First scenario in the catalog with `capillaryEnabled: true`. Two dimensions: a Brooks-Corey entry-pressure ladder against a fixed BL overlay, and a "physics or truncation error?" dimension contrasting a coarse capillary-free grid with a fine capillary one.

Load-bearing measurement (2026-07-24, this tree, 96 cells, 500 days, replay `pnpm vitest run src/lib/catalog/scenarios/wf_capillary.test.ts`): at the 400 bar drawdown `wf_bl1d` uses, even an 8 bar entry pressure changes front width by only ~3 % — the case is viscous-dominated to an unrealistic degree. At a representative 40 bar interwell drawdown the ladder is clean and monotonic:

| P_e (bar) | 0 | 1 | 3 | 8 |
|---|---|---|---|---|
| breakthrough (PVI) | 0.5650 | 0.5457 | 0.5157 | 0.4498 |
| front width (PVI) | 0.0805 | 0.1211 | 0.1776 | 0.2892 |

Both claims are guarded by tests: monotonic front spreading with P_e, earlier first water (imbibition ahead of the viscous front), and capillary smearing surviving grid refinement where numerical smearing does not.

**Still open in T7.4:** the gravity-capillary *transition zone* — the hydrostatic P_c = drho.g.h profile and Leverett J-function scaling. That is a saturation-versus-depth comparison and the chart stack is time-series only (`SwProfileChart.svelte` is dormant and unwired). Needs a profile-chart primitive, related to but separate from E8.

**T7.1 — analytical module SHIPPED, scenario NOT.** `src/lib/analytical/wellTest.ts` + 37 tests: exponential integral E1 (series/continued-fraction, verified to 1e-12 relative against independently computed 40-digit values), line-source and semilog drawdown, Horner buildup, semilog line fitting, and the two inverse problems — permeability from the semilog slope and skin from the one-hour intercept, both verified by round trip over skin in [-3, 12]. Every constant is derived from the engine's own `DARCY_METRIC_FACTOR` rather than lifted from a field-unit textbook formula, so it cannot drift from the simulator's transmissibility convention. Remaining for T7.1: the `AnalyticalMethod` union member, the adapter, a semilog chart layout, and the scenario itself (the rest of E10).

**T7.11 negative result — attempted, refuted, not shipped.** A `wf_orientation` scenario was built and measured: same 31x31 grid, same pore volume, wells moved between edge-to-edge ("parallel") and corner-to-corner ("diagonal"), crossed with favorable and adverse mobility ratio. The construction cannot demonstrate the classical effect, and the measurements say so:

- Comparing at equal *days* is invalid under BHP control — the diagonal path has higher resistance, injects less per day, and sits at an earlier point in its flood. That alone produced a 36 % spread unrelated to orientation.
- Balanced rate control removes the confound at the source but drove IMPES substep counts so high that one 961-cell variant did not finish in ten minutes headless — not a browser-viable scenario.
- Comparing at equal PVI, controlled for each variant's own breakthrough (recovery gained over the 0.15 PV following breakthrough), the orientation gap was **larger at the favorable mobility ratio (35 %) than the adverse one (21 %)** — the opposite of the grid-orientation signature, and both far too large to be a discretization artifact.

Diagnosis: with a single injector-producer pair on a Cartesian grid, moving the wells changes the *pattern geometry* (path length, swept shape, boundary interaction) by far more than it changes the grid alignment, and the geometric term is itself mobility-dependent, so the 2x2 cannot separate them. The honest construction is the Yanosik & McCracken one: the same repeated five-spot represented two ways, which needs **multiple injectors and producers per run** (the worker's `payload.wells` array exists but no scenario drives it) or a rotated/nine-point grid capability. Reclassify T7.11 from "no engine gap" to "needs multi-well pattern support" and do not retry it with a single well pair.

### Suggested order

T7.4 (done) and T7.1 (module done) first (days of work, zero engine risk, and T7.4 closes the shipped-but-unused capillary gap), then T7.1 (the missing classical pillar), then E8/T7.19 — because §7.C and §7.D cannot be told properly without it. T7.3 (aquifer) is the one large physics item worth committing to, since it unlocks T7.2's water-drive variant, T7.17 and live PUNQ-S3.

## Enabler gaps surfaced by Tiers 5–7 (backlog)

| Key | Gap | Unblocks | Effort |
|---|---|---|---|
| E1 | `permMode: 'field'` — per-cell perm arrays in payload + a `setPermeabilityField` wasm setter (core already stores per-cell `perm_x/y/z` vecs; only `uniform`/`perLayer`/`random` are exposed) | 5.4 Tavassoli, SPE10 M1/layer subsets, Egg | small |
| E2 | Declarative time-based well schedule in scenario params (`[{day, wellId, patch}]`) applied by the worker between report steps — wasm `setWellSchedule`/`setInjectedFluid` already exist, worker currently applies schedules once at create | 5.5 WAG, SPE9 | moderate |
| E3 | Per-well injected fluid (currently one global `injected_fluid`) | simultaneous water+gas injector patterns only — NOT needed for single-injector WAG | moderate, defer |
| E4 | Relperm hysteresis (Killough/Carlson) | quantitative WAG; scanning-curve teaching content | large |
| E5 | History/forecast chart affordance: vertical divider + shaded history window, so "all variants match here, diverge there" reads at a glance | 5.1, 5.4, PUNQ-S3 | **LANDED** (`resolveHistoryDivider`; open bug: no `logTime` support — TODO.md) |
| E6 | Inactive-cell / null-block support | *live* PUNQ-S3, Norne-like sectors (pre-run versions don't need it) | moderate |
| E7 | `runPolicy: 'prerun-artifacts'` scenario class — no worker run; variants map to bundled artifact keys; read-only parameter panel; 3D off | entire Tier 6 | **LANDED** as `capabilities.runMode`; precedent `wf_bl1d_opm.ts`. Multi-artifact fan/ensemble split out to E8 |
| E8 | **Ensemble / fan-curve chart primitive** — N realizations as a P10/P50/P90 band rather than N labelled curves, across both live variants and multiple pre-run artifacts | T7.19 and therefore all of Tier 7.D; Tier 6.1 PUNQ-S3, 6.6 Egg | large. Lands in `buildChartData.ts` as a new sequential section (which the frontend-architecture skill explicitly permits — what it forbids is inlining analytical-method physics there) plus band-fill support in `ChartSubPanel`/curve types. Needs visual verification, so it is not a headless-testable change |
| E9 | Aquifer boundary model (analytical influx into boundary cells) | T7.2 water-drive gas, T7.3, T7.17, live 5.6 PUNQ-S3 | large (engine) |
| E10 | Well-test analytical module (`src/lib/analytical/wellTest.ts`) + `AnalyticalMethod` union entry and adapter | T7.1 | **module LANDED 2026-07-24**; union entry, adapter and semilog chart layout still open |
| E11 | **Multi-well patterns** — more than one injector/producer per run driven from scenario params. The worker already honors a `payload.wells` array; no scenario populates it | T7.11 done properly (Yanosik-McCracken five-spot pair), SPE9, pattern-density studies (T7.15) | moderate |

E1's sweep path is wired but the **single-run** path is not (`parameterStore.fieldPermX/Y/Z` default `[]`, `applyResolvedParams` doesn't map them) — see `TODO.md`. Close it with the first consuming scenario (T7.9 / T7.6 / Egg).

OPM summary parser (`tools/opm_flow/.../artifacts.py`) is **done** — both committed artifacts are `status: "parsed"`. What remains per-case is deck authoring: 5.3 `dep_pvt` has no OPM deck, and neither do `gas_injection` / `gas_drive`.

## Where to search for more

- **OnePetro / SPE** (onepetro.org is on the tool allowlist) — comparative solution projects, type-curve papers, field case studies with published data tables.
- **OPM repos** (github.com/OPM: `opm-tests`, `opm-data`) — ready decks: SPE1/3/5/9, Norne, model variants.
- **MRST** (SINTEF, mrst.no) — example library mirrors many classic cases with full parameter sets; good for cross-checking case setups.
- **SPE Reservoir Simulation Conference benchmark sessions** — newer CSP-style benchmarks (e.g. 11th CSP on CO2, currently out of engine scope).
- **University groups**: Stanford SUPRI/ECRB, Heriot-Watt, UNICAMP UNISIM, TU Delft (Egg), NTNU (Norne).

## Process per case

1. Verify reference + engine envelope (criteria above). 2. Follow `.claude/skills/add-scenario/`. 3. If OPM-comparable, add a deck via `.claude/skills/opm-reference-pipeline/`. 4. Define tolerance bands in the style of `docs/P4_TWO_PHASE_BENCHMARKS.md`. 5. Record provenance (paper/dataset, units, license) in the scenario file and README inventory.
