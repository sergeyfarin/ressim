# Multi-Source Comparison Roadmap (2026-07-19)

What new *kinds* of comparison the app can host, built from combinations of six solution sources.
This is the comparison-axis companion to `docs/CASE_LIBRARY_ROADMAP.md` (which catalogs cases);
here the organizing idea is **which sources are compared and what the disagreement teaches**.
Not every source applies to every exhibit — the value is in choosing the right subset.

## 1. The six sources and their roles

| # | Source | Role in a comparison | Availability |
|---|--------|----------------------|--------------|
| S1 | Analytical solution | Exact truth *within its assumptions*; exposes what the assumptions cost | TS analytical layer (BL/Welge, Dietz PSS, Fetkovich/Arps, Craig, Dykstra-Parsons/Stiles, gas-oil BL, MB diagnostics) |
| S2 | WASM IMPES (live) | The app's interactive baseline; user can perturb it | Now |
| S3 | WASM FIM (live) | Same physics, different time discretization — the only source that isolates *numerical scheme* while holding everything else fixed vs S2 | Soon (separate workstream; `fimEnabled` already plumbed end-to-end) |
| S4 | OPM Flow (pre-run) | Industrial-grade reference; also the only in-house source for physics ResSim lacks (hysteresis, compositional, corner-point, aquifers) | Now (E7 class + parser + 2 artifacts) |
| S5 | Published results | External ground truth: digitized benchmark-paper curves (SPE1 Eclipse precedent), inter-simulator CSP datasets (SPE11), published lab experiments | Per-case curation |
| S6 | Other sources | Everything fitted or borrowed: in-browser reduced-order models (CRM, Arps fits, MB straight-line), MRST textbook results, open field datasets (Norne/Volve/Egg), a second open simulator's published output | Mostly TS-implementable or data curation |

**Comparison semantics** — what each *pairing* isolates:

- S1↔S2/S3: **verification + assumption cost** (existing core of the app).
- S2↔S3: **numerical scheme** — identical physics/grid/wells, only implicitness differs. Stability limit vs accuracy, timestep smearing, wall-time. Nothing else in the app can isolate this.
- S2/S3↔S4: **implementation parity** — ResSim vs an industrial solver on the same deck; residuals localize to well models, PVT treatment, upwinding.
- S4↔S5: **simulator-to-simulator spread** — even reference tools disagree (SPE CSP theme).
- S1↔S5(lab): **theory vs experiment** — the only pairing where physics itself is on trial.
- S6(fitted)↔any: **proxy fidelity** — what a cheap model captures and misses.
- Same source ↔ itself at different numerics/realizations: **uncertainty from numerics/geology**, not parameters.

## 2. Proposed exhibits by comparison pattern

### A. Verification ladders — "one problem, every solution" (S1+S2+S3+S4, S5 where published)

The flagship pattern: maximum sources on a problem simple enough that they should all agree, so
each residual is attributable.

| Exhibit | Sources | Teaching point | Feasibility |
|---|---|---|---|
| A1. **BL waterflood four-way** (extend `wf_bl1d`) | S1+S2+S3+S4 | Welge sharp front vs two numerical-diffusion signatures vs OPM's upwinding — one chart, four provenance badges | Artifact bundled; needs FIM arrival + dual-solver run sets (gap G1) |
| A2. **Dietz depletion four-way** (extend `dep_pss`) | S1+S2+S3+S4 | PSS decline constant reproduced four ways; shape-factor sensitivity as the analytical stressor | New OPM deck is trivial (bounded single-producer); rest exists |
| A3. **Gas-oil BL three/four-way** (extend `gas_injection`) | S1+S2+S3(+S4) | High-mobility-ratio displacement — the case where IMPES visibly strains and FIM should shine | Deck exists conceptually; FIM-dependent |

### B. Solver duels — the scheme itself as the subject (S2↔S3, S1 as referee)

Unlocked the moment FIM lands; the IMPES/FIM pair is the app's unique asset (no public tool lets
you flip implicitness live in a browser).

| Exhibit | Sources | Teaching point | Feasibility |
|---|---|---|---|
| B1. **The timestep wall** | S2+S3 (+S1 referee) | Same case, dt ladder: IMPES fragments into substeps / goes unstable where FIM strides — but FIM's large steps smear the front. Show wall-time and accepted-substep counts *next to* physical curves | Needs G1 + G2 (run metrics surfaced) |
| B2. **Numerical diffusion bake-off** | S1+S2+S3 | BL front sharpness vs dt and scheme, scored against the analytical shock | Needs G1 |
| B3. **Solver disagreement as uncertainty proxy** | S2+S3+S4 | Three independent implementations forecast the same field: their spread is a floor estimate of numerical uncertainty — bridges to SPE11's message | Cheap once A1 exists |

### C. Industrial parity — ResSim vs OPM vs published (S2/S3+S4+S5)

| Exhibit | Sources | Teaching point | Feasibility |
|---|---|---|---|
| C1. **SPE1 full-stack** (extend existing) | S2+S3+S4+S5 | Already has S2+S5+S4-artifact; add FIM when ready — the FIM workstream's own target case becomes a public exhibit | Blocked on review finding 1 (opm-* render) + FIM |
| C2. **SPE10 layer slice** | S2+S4+S5 | Single published layer (e.g. 36 or 59) as heterogeneity stress: per-cell perm (E1, landed) + OPM run + published upscaling results | E1 done; needs data download decision + G3 |
| C3. **Tavassoli SPE-86883** (roadmap 5.4) | S1(MB)+S2+S5 | History-match non-uniqueness at field scale; E5 divider is the payoff feature | E1 done; needs field-perm store wiring (review finding 6) |

### D. Physics-fidelity gaps — OPM carries physics ResSim lacks (S4 vs S2, S1 optional)

The honest-gap pattern piloted by Tier 6.4 (WAG hysteresis): run OPM twice (physics on/off), show
ResSim tracking the "off" run — the missing physics *is* the exhibit.

| Exhibit | Sources | Teaching point |
|---|---|---|
| D1. Relperm hysteresis on/off (Tier 6.3/6.4) | S4×2 + S2 | What Killough hysteresis does to WAG screening |
| D2. Capillary transition zone | S1(Leverett-J)+S2+S4 | ResSim *has* Pc — a rare three-way where ResSim plays on equal terms |
| D3. Compositional vs black-oil (Tier 6.2) | S4×2 | Fluid-model representation risk, big brother of `dep_pvt` |
| D4. Corner-point vs Cartesian geometry | S4×2 | Gridding as a modeling decision; no ResSim run at all — pure pre-run exhibit |

### E. Theory vs experiment — published lab data as truth (S5-lab + S1 + S2)

A source type the app doesn't have yet: **digitized core-flood experiments**. The
Buckley-Leverett/Welge papers and later SCA core-flood studies contain measured saturation
profiles and recovery curves. Overlaying *measured* data puts the physics itself on trial, not
just the numerics — no other exhibit does that.

- E1x. **BL core-flood validation**: digitized lab displacement (e.g. Welge 1952 or a modern SCA
  dataset with clear licensing) vs S1 vs S2. Curation-heavy, high pedagogical value.
- Caution: SPE-copyright figures — follow the SPE1 digitization precedent (small samples,
  attribution, provenance note per `publishedReferenceSeries`).

### F. Fitted proxies — "other source" computed in the browser (S6 + any)

Cheap to build (pure TS, no engine work), and they teach model-hierarchy judgment:

| Exhibit | Sources | Teaching point | Feasibility |
|---|---|---|---|
| F1. **CRM vs full simulation** | S6(CRM)+S2/S4 | Capacitance-resistance model fitted to rate history reproduces interwell connectivity without a grid — where does it break? | CRM fit is a small least-squares in TS |
| F2. **MB straight-line on simulator output** | S6(Havlena-Odeh)+S2+S1 | Extends `dep_nct`: fit the straight line to "observed" sim data, recover N·c_t, show the non-uniqueness *as a fitting exercise* | Mostly exists in MB diagnostics |
| F3. **Arps fit vs physics** | S6(fit)+S2+S1(Fetkovich) | Empirical decline vs the physics that generates it; extrapolation risk beyond the fit window (pairs with E5 divider) | `dep_arps` groundwork exists |

### G. Ensembles — same source, many instances (S4×N, S5×N)

Already roadmapped (SPE11 inter-simulator ×18, PUNQ-S3 realizations, Egg ×101). One cheap
addition: **G1x. numerics-as-uncertainty** — the same OPM deck at 3 grid resolutions / tolerance
settings, fanned. Zero new data sourcing, directly previews the SPE11 message.

## 3. Engineering gaps this roadmap needs (beyond existing E-list)

- **G1 — dual-solver run sets + solver provenance.** Run the same variant twice (IMPES + FIM) in
  one sweep, and tag results so charts can style/badge them separately. Today
  `ReferenceSourceType` has one `'simulation'`; the run spec carries `fimEnabled` but nothing
  downstream distinguishes the two. Small, prerequisite for all of A/B.
- **G2 — run-metrics surfacing.** Wall time, accepted substeps, retry counts already exist in
  rate-history diagnostics; the solver-duel exhibits need them displayed per result (a small
  metrics strip, not a chart).
- **G3 — bundled-data conventions for non-OPM sources.** `publishedReferenceSeries` handles
  digitized curves; per-cell perm fields (SPE10/Egg) and lab datasets need a documented
  bundling/attribution convention (+ the review-finding-6 store wiring for field perms).
- **Prerequisite from the Wave-4 review:** finding 1 (opm-* curves filtered out of panels) blocks
  every exhibit that shows an OPM artifact — fix first.

## 4. Suggested order

1. **Fix review finding 1** (blocks everything OPM-visible).
2. **A2 Dietz four-way minus FIM** (trivial new deck, proves the ladder pattern) and
   **F2/F3 fitted-proxy extensions** (pure TS) — deliverable before FIM lands.
3. When FIM arrives: **G1 → A1 BL four-way → B1 timestep wall** (the app's signature exhibits).
4. **C2 SPE10 layer / C3 Tavassoli** (E1 is ready; close review finding 6 with them).
5. **G ensembles + D physics-gap pairs** per the existing Tier-6 order (SPE11 first when data is
   sourced).
6. **E1x lab-data validation** when a cleanly licensed dataset is identified.

---

# Addendum (2026-07-19): back-to-basics analytics, waterflood, EOR, uncertainty, exploration

Second ideation pass along five requested axes. Two cross-cutting themes emerged that are worth
naming because many exhibits below instantiate them:

- **"Diagnostics on known truth"** — apply a classic interpretation/surveillance method (Horner,
  Chan, X-plot, type-curve match, DST interpretation) to *simulated* data whose true answer is
  known, and score the method. The app is uniquely placed for this: no textbook can show you the
  truth behind the diagnostic; a simulator-backed exhibit can.
- **"Free fans from analytics"** — an analytical model evaluated 1,000× costs nothing, so
  uncertainty propagation through S1 is instant, and the honest question becomes *does the cheap
  fan match the expensive (simulated) fan?* — model fidelity and uncertainty in one exhibit.

## 5. Pattern H — Back-to-basics analytical ladder ("the fundamentals shelf")

Classic solutions every reservoir engineer learns, each as a small verification-plus-teaching
exhibit. Mostly S1+S2 (+S3 later), tiny TS additions, no new data sourcing.

| Exhibit | Sources | Teaching point | Feasibility |
|---|---|---|---|
| H1. **Life of a drawdown** | S1×2 + S2 | One constant-rate run overlaid with the exponential-integral (Ei/Theis) solution early and Dietz PSS late — infinite-acting → transition → boundary-dominated, with the radius-of-investigation `r_inv(t)` marking when boundaries "arrive" | Ei is a few lines of TS; dep_pss params reusable |
| H2. **Horner buildup — recover kh** | S1(fit) + S2 | Shut the well mid-run, Horner-plot the buildup, recover kh/skin, compare to the model's true values | Needs shut-in → **E2 well schedule** |
| H3. **Peaceman well index vs grid** | S1 + S2 | Steady-state radial injectivity vs simulated well PI across a grid-refinement ladder; the Peaceman `r_o ≈ 0.2Δx` correction made visible — "understand what your simulator's well model does" | Pure existing machinery; grid variants |
| H4. **Capillary-gravity equilibrium (zero-flow test)** | S1 + S2 | No wells at all: initialize a column, run, verify Sw(depth) matches the inverted Leverett-J transition zone — a "do nothing correctly" verification, and the cheapest possible FIM/IMPES parity check later | ResSim has Pc + gravity; trivial scenario |
| H5. **Darcy 101** | S1 + S2 | 1D incompressible steady state: exact linear pressure profile, exact rate — the first-touch student exhibit | Trivial |
| H6. **Solution-gas drive (Tarner/Muskat)** | S1 + S2 | Stepwise MB integration of GOR/RF below bubble point vs the three-phase simulator — the classic hand method vs the machine | Tarner is a small TS integration loop; 3-phase sim exists |
| H7. **p/z — the classic misread** | S1 + S2 + S4 | Volumetric dry-gas depletion gives an exact p/z straight line; the same field with an aquifer (OPM AQUCT pre-run) curves late — early-data extrapolation overstates OGIP. Doubles as an exploration exhibit (L5) | p/z diagnostics exist; needs one OPM aquifer deck |
| H8. **Welge construction, shown** | S1 (interactive) | Render the fractional-flow curve with the tangent construction itself (shock, front saturation, average Sw behind front) as a live panel next to the sim profile — teach the *construction*, not just its output | Chart-layer work; math exists in `fractionalFlow.ts` |
| H9. **Poor-man's aquifer** | S1 + S2 + S4 | Van Everdingen-Hurst / Fetkovich aquifer analytic vs the big-pore-volume-boundary-cell trick in live IMPES vs OPM AQUCT — three ways to model influx, two of them approximations with visible costs | VEH/Fetkovich aquifer analytic is standard TS; trick needs per-cell φ or a high-φ column (per-layer φ not yet exposed — check) |

## 6. Pattern I — Waterflood surveillance & assumption-testing

| Exhibit | Sources | Teaching point | Feasibility |
|---|---|---|---|
| I1. **Chan plot: coning vs channeling** | S6(diagnostic) + S2×2 | Run a coning case (xz section, producer near OWC-like water zone) and a thief-layer channeling case; compute WOR and WOR-derivative — show the Chan diagnostic actually separates the two mechanisms | Both cases in-envelope; diagnostic is TS post-processing |
| I2. **Ershaghi X-plot on known truth** | S6 + S2 | Recovery linearization fitted to sim watercut — does the extrapolated ultimate match the model's true movable oil? | TS-only |
| I3. **What crossflow does to Dykstra-Parsons** | S1 + S2 | DP assumes non-communicating layers: kv=0 matches the analytic; raising kv makes the sim diverge — the assumption quantified live (sweep_vertical extension) | Fully in-envelope today |
| I4. **Pattern family** | S1 + S2 | Direct line drive vs staggered vs five-spot at fixed M (Craig correlations per pattern) — pattern selection as sweep-efficiency choice | Craig data exists for five-spot; add pattern correlations |
| I5. **Wettability as relperm family** | S1 + S2 | Water-wet / mixed / oil-wet Corey sets, analytical BL per set — Craig's rules of thumb, live | Trivial variants of wf_bl1d |
| I6. **VRR ladder** | S1(MB) + S2 | VRR <1 / =1 / >1: pressure maintenance vs depletion hybrid, MB overlay explains the pressure path | VRR diagnostic exists |

## 7. Pattern J — EOR within an honest envelope

Live engine has immiscible gas + 3-phase; everything else is S1-analytic + S4-pre-run + proxy,
labeled as such (the D-pattern honesty rule).

| Exhibit | Sources | Teaching point | Feasibility |
|---|---|---|---|
| J1. **Polymer fractional flow** | S1(Pope) + S2(waterflood baseline) + S4(polymer deck) | Viscosified water shifts f_w → later breakthrough, better sweep; analytic vs OPM polymer, live waterflood as the "do nothing" baseline | Pope f_w is easy TS; `opm-tests/polymer*` decks (Tier 6.7) |
| J2. **Surfactant as capillary-desaturation proxy** | S1 + S2(proxy) | Lower s_or + suppressed Pc = "surfactant effect": what breaking capillarity buys in RF; explicitly labeled a parameter proxy, not chemistry | In-envelope today |
| J3. **Gravity-stable gas injection (GAGD)** | S1(Dietz stability) + S2 | Critical rate for a stable gas front in a vertical flood — below: piston; above: fingering/override. The stability criterion is one formula | 3-phase vertical flood in-envelope |
| J4. **Miscible proxy: Koval / Todd-Longstaff** | S1(Koval) + S4(solvent deck) | Fingering-averaged miscible displacement theory vs OPM's TL solvent model | Koval is simple TS; solvent decks in opm-tests |
| J5. **Incremental-recovery framing** | any EOR pair | Every EOR exhibit should show the *delta* vs its waterflood baseline — needs a small chart affordance (gap G6) | — |

## 8. Pattern K — Uncertainty machinery

| Exhibit | Sources | Teaching point | Feasibility |
|---|---|---|---|
| K1. **Bootstrap decline forecast** | S6(fit×N) + S2(truth) | Add measurement noise to "observed" early rates, refit Arps N times → forecast fan vs the sim's true future; E5 divider marks the history window. Data quality → forecast confidence, all in TS | Cheap; high value; no engine work |
| K2. **Analytic vs simulated Monte Carlo** | S1×1000 + S2×~20 | Sample V_DP / M / Corey exponents: the analytic fan is free, the sim fan is expensive — do they agree? (When they don't, the proxy's fan was never trustworthy.) | Needs G5 sampled run-sets + G4 fan bands |
| K3. **"Uncertainty mode" (product idea)** | S2×N | Generalize sensitivity sweeps to sampled sweeps: N random draws over declared parameter ranges on small grids (~20–50 browser runs), P10/50/90 bands | G4 + G5; the run-set machinery is the hard part already done |
| K4. **OGIP range from aquifer ambiguity** | S1 + S4 | H7's misread quantified as an uncertainty band on OGIP from early data | Rides on H7 |

## 9. Pattern L — Exploration toolbox

Pre-production framing: sparse data, discovery-well tests, volumetrics, analogs.

| Exhibit | Sources | Teaching point | Feasibility |
|---|---|---|---|
| L1. **Volumetrics → deliverability bridge** | S1(MC) + S2×3 | Classic GRV·φ·So/Bo Monte Carlo (instant, TS) → pick P10/50/90 realizations → run depletion on each: resource range becomes production range | G4 fans; small |
| L2. **DST on heterogeneous truth** | S1(interpretation) + S2(E1 field) | Interpret a short drawdown assuming homogeneity (Ei fit → kh, skin) on a known heterogeneous per-cell field — how biased is a 12-hour test? E1's first strong consumer | Needs H1 machinery + E1 store wiring |
| L3. **Interference / compartmentalization test** | S1(superposition) + S2×2 | Pulse one well, watch pressure at an observation point; connected vs low-perm-baffle realizations (per-cell perm barrier) — does the test detect the compartment? Appraisal-decision framing | Needs observation-point pressure traces (G8); barrier via E1 |
| L4. **Sparse-data type-curve EUR** | S6(match) + S2 | Fetkovich type-curve matching on the first N months only → EUR range vs truth; the E5 divider is the star | dep_decline groundwork; TS-only |
| L5. **Discovery p/z ambiguity** | = H7/K4 | Same exhibit, exploration framing | — |

## 10. Additional engineering gaps (extends §3)

- **G4 — fan/band rendering** (P10/50/90 shaded bands): now needed by K1–K3, L1, and SPE11 —
  promote from "SPE11-era" to a shared prerequisite.
- **G5 — sampled run-sets**: extend the sweep runner from enumerated variants to N sampled
  parameter draws (seeded), with per-draw provenance.
- **G6 — delta/incremental curves**: render `case − baseline` for EOR increments (J5).
- **G7 — shut-in via well schedule**: H2 (buildup) and any pulse test are blocked on **E2**
  (declarative time-based schedule), which now has three consumers (WAG 5.5, H2, L3).
- **G8 — observation-point pressure traces**: rateHistory carries field aggregates only;
  interference tests need p(t) at a chosen cell. History snapshots already hold full grids at
  intervals — a TS extraction may suffice without engine changes.
- **Per-layer/per-cell porosity**: H9's aquifer trick and richer heterogeneity cases want φ
  fields like E1's perm fields — check current exposure before assuming an engine gap.

## 11. Amended order (addendum items only)

Cheap, high-value, FIM-independent, data-independent first:
**K1 bootstrap decline** and **I3 crossflow-vs-DP** (both nearly free) → **H1 drawdown ladder** +
**H4 zero-flow equilibrium** (fundamentals shelf opens; H4 also becomes the cheapest FIM parity
check later) → **I1 Chan plot** + **I5 wettability family** → **H7/K4 p/z misread** (first
aquifer deck) → **J1 polymer** (first EOR) → **L2 DST** + **L3 interference** once E1 store
wiring and G8 land → **K2/K3 uncertainty machinery** behind G4/G5.
