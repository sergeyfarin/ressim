# Black-Oil Validation

Authoritative record for how the black-oil path is graded: the SPE1 comparative-solution
acceptance criteria, the depletion grid-convergence checks, and the solver safeguards a user
should know about when reading black-oil results.

Companion documents: `docs/UNIT_SYSTEM.md` (units, equations, PVT notes),
`docs/THREE_PHASE_IMPLEMENTATION_NOTES.md` (three-phase implementation state),
`docs/P4_TWO_PHASE_BENCHMARKS.md` (two-phase Buckley-Leverett policy).

## 1. SPE1 acceptance criteria

**Case.** SPE Comparative Solution Project #1 (Odeh 1981, SPE 9723), Case 1: 10×10×3 grid,
1000 ft × 1000 ft cells, layer thicknesses 20/30/50 ft, layer permeabilities 500/50/200 mD,
gas injection at (1,1) in the top layer, producer at (10,10) in the bottom layer, producer
20,000 STB/day oil with a 1000 psia BHP floor, injector 100 MMscf/day with a 9014 psia ceiling.

**Reference.** `flow 2026.04` on `OPM/opm-common/tests/SPE1CASE1.DATA` — the same series the
frontend overlays on the `spe1_gas_injection` scenario (`TODO.md`, "SPE1 reference data
(2026-07-24)"). The Rust test embeds the yearly field-pressure and producing-GOR samples plus
the producer oil-rate report schedule so the engine can be graded without the frontend.

**Where.** `src/lib/ressim/src/tests/spe1_acceptance.rs`.

| Criterion | Tolerance | Worst measured error |
|---|---|---|
| Field average reservoir pressure, yearly to 3650 d | 3 % | 1.73 % (at 1095 d) |
| Producer surface oil rate, yearly to 3650 d | 8 % | 3.33 % (at 2190 d) |
| Producing GOR, yearly to 3650 d | 12 % | 4.39 % (at 3285 d) |
| Producer holds the 3179.74 Sm³/d surface target while the reference is on plateau (≤ 730 d) | 0.5 % | met |
| Oil material-balance drift vs STOIIP | 1 % | within band at every checkpoint |
| Gas material-balance drift vs gas handled (initial free + dissolved in place, plus cumulative injection) | 1 % | within band at every checkpoint |
| Solver warnings during the run | none | none |

Tolerances are acceptance criteria with deliberate headroom (roughly 1.7×–2.7× the measured
error), not tuned-to-the-build benchmark tolerances. They are not to be widened to make a change
pass; a regression that breaks one is a physics or solver finding.

The gas material-balance denominator is total surface gas handled by the case, not cumulative
injection alone. Early in the run, injection is small compared with the gas already dissolved in
the oil, so an injection-only denominator makes an unchanged absolute drift look like a
time-dependent error (2.9 % at 365 d against injection, 0.27 % against gas handled).

### Replay

Fast gate (first reference year, ~12 s debug) — runs by default and in
`bash scripts/validate-solver-coverage.sh fim`:

```bash
cargo test --manifest-path src/lib/ressim/Cargo.toml spe1_first_year_matches_published_reference -- --nocapture
```

Full 10-year acceptance replay (~3 s release, ~90 s debug) — `#[ignore]`d, so it must be asked
for explicitly:

```bash
cargo test --release --manifest-path src/lib/ressim/Cargo.toml spe1_full_horizon_matches_published_reference -- --ignored --nocapture
```

### Recorded baseline

Engine revision `0cfead9` ("fixing OPM decks"), solver FIM (the catalog default for
`spe1_gas_injection`), 30-day report steps, measured 2026-07-24 with the acceptance tests added
on top of `0cfead9` and no engine files modified. Verbatim summary from the full replay:

```
t=  365.0 pressure_err= 0.282% oil_rate_err= 0.001% gor_err= 1.079%
t=  730.0 pressure_err= 0.939% oil_rate_err= 0.001% gor_err= 3.736%
t= 1095.0 pressure_err= 1.728% oil_rate_err= 0.121% gor_err= 1.450%
t= 1460.0 pressure_err= 1.383% oil_rate_err= 0.652% gor_err= 0.198%
t= 1825.0 pressure_err= 1.075% oil_rate_err= 1.730% gor_err= 1.221%
t= 2190.0 pressure_err= 1.098% oil_rate_err= 3.325% gor_err= 3.659%
t= 2555.0 pressure_err= 1.011% oil_rate_err= 3.230% gor_err= 3.719%
t= 2920.0 pressure_err= 0.822% oil_rate_err= 3.112% gor_err= 3.940%
t= 3285.0 pressure_err= 0.856% oil_rate_err= 3.213% gor_err= 4.390%
t= 3650.0 pressure_err= 0.923% oil_rate_err= 3.153% gor_err= 3.617%
SPE1 worst-case errors: pressure=1.728% oil_rate=3.325% gor=4.390%
```

Provisional until rerun on the committed revision that contains these tests; the engine under
test is `0cfead9` either way, since the change adds tests only.

### SPE1 under areal refinement (characterization, not a criterion)

The catalog's `grid` sensitivity offers 20×20×3 over the same domain. Refining does **not**
uniformly improve reference agreement, so it is recorded rather than asserted:

```
nx= 10 t=  730.0 pressure_err=  0.939% oil_rate_err=  0.001% gor_err=  3.736%
nx= 10 t= 1095.0 pressure_err=  1.728% oil_rate_err=  0.121% gor_err=  1.450%
nx= 10 t= 3650.0 pressure_err=  0.923% oil_rate_err=  3.153% gor_err=  3.617%
nx= 20 t=  730.0 pressure_err=  1.878% oil_rate_err=  0.001% gor_err= 32.834%
nx= 20 t= 1095.0 pressure_err=  2.845% oil_rate_err=  6.559% gor_err=  2.356%
nx= 20 t= 3650.0 pressure_err=  0.121% oil_rate_err=  0.979% gor_err=  1.047%
```

Read together: the refined grid is *better* late (pressure 0.12 % vs 0.92 %, GOR 1.0 % vs 3.6 %
at 3650 d) and *worse* through breakthrough (GOR 32.8 % at 730 d, oil rate 6.6 % at 1095 d). The
refined case breaks gas through earlier and more sharply than the reference; once the field is
well past breakthrough it tracks the reference more closely than the coarse grid does. This
narrows the older "finer grid moves away from reference" note in `TODO.md`: the divergence is a
breakthrough-timing/front-sharpness effect, not a whole-run degradation.

Material balance holds on both grids at every checkpoint, so this is a transport/well-model
question rather than a conservation defect.

```bash
cargo test --release --manifest-path src/lib/ressim/Cargo.toml spe1_areal_refinement_reference_error_replay -- --ignored --nocapture
```

## 2. Grid convergence for black-oil depletion

**Case.** 1D column, 1000 m × 200 m × 20 m, φ = 0.2, k = 100 mD, initial pressure 175 bar
(undersaturated, Rs = 15 Sm³/Sm³, bubble point 150 bar), producer at the far end on a 120 bar
BHP, no gas redissolution, 20 × 5-day steps. The same physical domain is discretized at 5, 10,
20 and 40 cells; pore-volume-weighted field averages of pressure, Rs, Bo and free-gas saturation
must form a converging sequence.

**Where.** `src/lib/ressim/src/tests/physics/depletion_grid_convergence.rs`.

Criteria per quantity: each successive refinement difference must be at most 0.8× the previous
one (first-order upstream transport gives ~0.5–0.6 here), and the two finest grids must agree
to within 1 %. The case also asserts it is genuinely below the bubble point with liberated free
gas, so a degenerate state cannot pass silently.

### IMPES baseline (default gate, ~0.1 s debug)

| nx | pressure [bar] | Rs [Sm³/Sm³] | Bo [m³/Sm³] | Sg |
|---|---|---|---|---|
| 5 | 122.5011 | 9.50022 | 1.080389 | 0.033903 |
| 10 | 122.7612 | 9.55224 | 1.080751 | 0.033496 |
| 20 | 122.8931 | 9.57862 | 1.080934 | 0.033293 |
| 40 | 122.9711 | 9.59422 | 1.081042 | 0.033171 |

Successive-difference ratios are ≈ 0.51 then ≈ 0.59 for all four quantities — first-order,
monotone, consistent across the pressure, PVT and liberated-gas variables.

```bash
cargo test --manifest-path src/lib/ressim/Cargo.toml physics_depletion_grid_convergence_impes -- --nocapture
```

### FIM baseline — superseded 2026-09-22 by FIM-BUBBLE-001 (see below)

> The table below and the "non-monotone Sg" observation were measured on a run that took ~20,000
> substeps to cross the bubble point: gas-free cells were carried on an Sg primary pinned to
> the Sg = 0 relperm kink. After the fix the replay takes 20 s rather than ~3.5 min, contracts
> monotonically in every quantity, and matches OPM Flow on the same column
> ([`small-direct`](../opm/reference-decks/small-direct/README.md) `bo-1d-10`/`bo-1d-40`).
>
> | nx | pressure [bar] | Rs [Sm³/Sm³] | Bo [m³/Sm³] | Sg | OPM Flow p / Rs / Sg |
> |---|---|---|---|---|---|
> | 5 | 122.0752 | 9.41504 | 1.079797 | 0.034681 | |
> | 10 | 122.2670 | 9.45340 | 1.080063 | 0.034384 | 122.2669 / 9.45338 / 0.034385 |
> | 20 | 122.3742 | 9.47484 | 1.080212 | 0.034214 | |
> | 40 | 122.4318 | 9.48635 | 1.080292 | 0.034124 | 122.4301 / 9.48602 / 0.034127 |
>
> Successive pressure differences 0.192 / 0.107 / 0.058 bar: contraction 0.56, 0.54. Measured on
> branch `fim/bubble-point-lifecycle`, same replay command. The FIM half of #11 is resolved: FIM
> agrees with Flow to 0.002 bar and 3e-6 Sg. What remains is IMPES against Flow, about 0.5 bar
> and 3% Sg.

#### Historical FIM baseline (pre-FIM-BUBBLE-001)

| nx | pressure [bar] | Rs [Sm³/Sm³] | Bo [m³/Sm³] | Sg |
|---|---|---|---|---|
| 5 | 121.7097 | 9.34195 | 1.079290 | 0.029940 |
| 10 | 121.9734 | 9.39468 | 1.079655 | 0.030042 |
| 20 | 122.1341 | 9.42682 | 1.079879 | 0.030277 |
| 40 | 122.2031 | 9.44061 | 1.079974 | 0.030044 |

```bash
cargo test --release --manifest-path src/lib/ressim/Cargo.toml physics_depletion_grid_convergence_fim -- --ignored --nocapture
```

Two observations recorded rather than asserted:

- Pressure, Rs and Bo contract as on IMPES, but the FIM free-gas average is non-monotone at the
  1e-4 level (0.029940 / 0.030042 / 0.030277 / 0.030044). The spread is ~0.8 % of the value and
  is a substep-ladder artefact, not a refinement trend, so the FIM test bounds the spread (5 %)
  instead of demanding contraction.
- FIM and IMPES converge to slightly different answers on the same case (Sg ≈ 0.0300 vs 0.0332,
  ~10 %). Both are self-consistent under refinement; the gap is a solver/timestep question, not
  a grid-convergence one, and is tracked in
  [GitHub issue #11](https://github.com/sergeyfarin/ressim/issues/11) rather than in these tests.

## 3. Black-oil solver safeguards (read this before interpreting results)

These are deliberate, documented deviations from a textbook black-oil formulation. They keep the
pressure solve well-posed; they also mean a few reported quantities are approximations.

**Effective oil compressibility below the bubble point.** In three-phase mode the IMPES pressure
accumulation term uses `get_c_o_effective(p, Rs_cell)` (`src/lib/ressim/src/pvt.rs`,
`src/lib/ressim/src/impes/pressure.rs`) rather than a raw `-1/Bo · dBo/dp`:

- Saturated cells use `c_eff = -(dBo/dp)/Bo + (Bg/Bo)·dRs/dp`, i.e. rock-fluid storage plus the
  dissolved-gas contribution, evaluated by central difference on the PVT table.
- If that combination is non-finite or non-positive — which a saturated `Bo(p)` slope alone can
  produce, since Bo *increases* with pressure below the bubble point — the code falls back to
  the scenario's positive scalar `c_o`. A negative accumulation coefficient would make the
  pressure matrix indefinite and destabilize the IMPES solve.
- Undersaturated cells use the scalar `c_o` directly, blended quadratically into the saturated
  value over the last 5 bar above the cell's bubble point so the coefficient does not jump at
  the phase boundary.
- In two-phase mode `get_c_o` always returns the scalar `c_o`: reading `dBo/dp` off the
  saturated curve there would conflate oil compressibility with changing Rs along the
  bubble-point locus and overestimate the undersaturated value.

Practical consequence: near and below the bubble point, oil storage is a stabilized
approximation. It is accurate where the PVT table is well-behaved and conservative where it is
not; it is not a substitute for a fully implicit compositional treatment.

**The scalar undersaturated `c_o` default is asserted in two places.** The Rust core defaults to
`c_o = 1e-5 /bar` (`src/lib/ressim/src/lib.rs`); the frontend asserts the same number as
`DEFAULT_UNDERSATURATED_OIL_COMPRESSIBILITY_PER_BAR` in `src/lib/physics/pvt.ts`, which
`src/lib/analytical/materialBalance.ts` imports rather than redeclaring. Nothing enforces that
the two sides stay equal, so an analytical overlay can silently disagree with the engine if one
  default moves. A cross-language regression guard now asserts the shared value. Scenarios that set
  their own value (SPE1 uses 2.06e-4 /bar) override it on both sides.

**Material-balance diagnostics report each phase explicitly, with one structural limitation.**
Water and gas cumulative errors are direct inventory comparisons, and oil is reported against
stock-tank inventory depletion. Oil *saturation* is still residual by construction
(`S_o = 1 - S_w - S_g`), so the oil diagnostic checks reporting/FVF closure rather than an
independently transported oil-saturation equation. The SPE1 acceptance criteria therefore grade
oil and gas drift separately and normalize each against its own inventory. See
`docs/THREE_PHASE_VALIDATION.md` section 4.

**Gas redissolution is off in SPE1.** `gasRedissolutionEnabled: false` matches the reference
deck's behavior for this case: liberated free gas does not re-enter solution when pressure
recovers. Scenarios that need redissolution must opt in.

**SCAL is tabular where the deck is tabular.** SPE1 supplies exact SWOF/SGOF tables; the Corey
endpoints in the scenario remain only as fallback metadata for the two-phase path. Scenarios
without tables use the Corey model, which is an approximation of, not a substitute for, deck
tables.

## 4. Known gaps

- Three-phase status is no longer `experimental` (2026-07-25): exit criteria, the gas-drive OPM
  Flow comparative solution, and the breakthrough / Sg-evolution acceptance tests are recorded in
  `docs/THREE_PHASE_VALIDATION.md`.
- No SPE-style black-oil case beyond SPE1 (SPE9, volatile-oil style cases) is covered.
- The IMPES/FIM answer gap on the depletion column (section 2) is unexplained. Re-measured at
  `5ebdc78`: 9.0 % on `Sg` at `nx=40` (section 5).
- Scenario-wiring regressions for SPE1 (published-reference panel placement, `cellDzPerLayer`,
  per-layer completion payloads) remain frontend-side TODO items.

## 5. FIM repair F6 applicability table (2026-09-15)

Evidence base for the **FIM-REPAIR-READY** decision in
[`FIM_REPAIR_EXECUTION_PLAN_2026-09-14.md`](FIM_REPAIR_EXECUTION_PLAN_2026-09-14.md). Every row
was reproduced on commit `5ebdc78` with a clean tree; the replay command is next to each result.
"Blocker" means the failure affects a path F7's extraction reuses and must be resolved before it;
"independent" means real but outside that foundation.

| Case | Issue | Reproduced at `5ebdc78`? | Measured result | Reused FIM path | External oracle | Verdict |
|---|---|---|---|---|---|---|
| Closed depletion | — | yes | water/oil inventory drift `+8.9e-10` / `-1.2e-9` relative over 8 steps | accumulation, accepted-source integration | self-consistency (no sources) | **pass** |
| Gas reporting / `dep_gas_pz` | [#25](https://github.com/sergeyfarin/ressim/issues/25) | **no** | closure `+0.0109` (issue claimed `+0.0885`); recovery `0.9280` vs documented `0.928` | `record_fim_step_report` gas conversion | scenario's own volumetric GIIP | **not reproduced — close** |
| Black-oil depletion FIM vs IMPES | [#11](https://github.com/sergeyfarin/ressim/issues/11) | yes | `Sg(nx=40)` FIM `0.030179` vs IMPES `0.033171` → **9.0 %** | shared PVT/flash, FIM timestep ladder | neither backend is truth; both self-consistent under refinement | **independent** |
| Multi-completion gravity | [#10](https://github.com/sergeyfarin/ressim/issues/10) | **no** (reconstruction) | no warning on either solver; IMPES/FIM agree to 1 % on pressure, 5e-3 on peak `Sw` | shared well geometry, Peaceman PI, `refresh_well_head_offsets` | no independent geometry oracle | **not reproduced; cause/applicability inconclusive** |
| SPE1 / gas injection & appearance | [#12](https://github.com/sergeyfarin/ressim/issues/12) | n/a | `spe1_full_horizon_matches_published_reference` and `spe1_areal_refinement_reference_error_replay` both pass (release) | FIM Newton/assembly | published SPE1 reference | **scenario/frontend scope — independent** |
| Waterflood report-step sensitivity + Flow oil bias | [#21](https://github.com/sergeyfarin/ressim/issues/21) | yes (2026-09-23, below) | the 8–10% gap does **not** reproduce: matched-step FOPT within 0.05–0.36% on all three controls, **0.002%** once the decks' SWOF and Bo reference match ResSim | temporal discretization + two deck-mapping differences | source-pinned Flow on the tracked decks | **resolved — no solver defect** |

### Reproduction commands

```bash
# closed depletion, multi-completion gravity, lifecycle contracts
cargo test --manifest-path src/lib/ressim/Cargo.toml fim_repair_ -- --nocapture
# gas reporting (#25) — the scenario's own harness, including its inventory-closure check
npx vitest run src/lib/catalog/scenarios/dep_gas_pz.test.ts
# FIM vs IMPES depletion (#11)
cargo test --release --manifest-path src/lib/ressim/Cargo.toml physics_depletion_grid_convergence_fim -- --ignored --nocapture
cargo test --manifest-path src/lib/ressim/Cargo.toml physics_depletion_grid_convergence_impes -- --nocapture
```

### Section 2 FIM baseline re-confirmed at `5ebdc78`

The section 2 FIM table still reproduces; differences are at the 1e-3..1e-4 level, inside the
substep-ladder spread that section already records as an artefact rather than a trend.

| nx | pressure [bar] | Rs [Sm³/Sm³] | Bo [m³/Sm³] | Sg |
|---|---|---|---|---|
| 5 | 121.7150 | 9.34301 | 1.079297 | 0.029994 |
| 10 | 121.9720 | 9.39439 | 1.079653 | 0.030056 |
| 20 | 122.1341 | 9.42682 | 1.079879 | 0.030314 |
| 40 | 122.2055 | 9.44110 | 1.079978 | 0.030179 |

This supersedes nothing: it confirms the existing section 2 FIM baseline rather than replacing it.

### Verdict

**Scoped FIM regression repair is ready.** No reproduced failure blocks the layout extraction F7
actually delivered. #11 is real and reproduces at 9 %; preserving behavior preserves this
unexplained accuracy limitation rather than validating it. #10 did not reproduce in the
reconstruction, but its cause and applicability remain inconclusive. #25 did not reproduce.
#12 is scenario/frontend scope. #21 was executed on 2026-09-23 and resolved (section below).

### What this record does **not** establish

- #10 was tested through a **reconstruction** of the `wf_gravity` geometry, not a replay of its
  shipped deck. A negative result here does not prove the shipped scenario is clean. Agreement
  between two solvers that reuse the same geometry cannot exclude an error in that shared code.
- #21 was not attempted by F6; it was executed separately on 2026-09-23 (section below).
- #11's 9 % gap is measured, not explained. Nothing here identifies a mechanism.

## #21 — waterflood report-step sensitivity and the Flow oil bias (2026-09-23)

Master `654618f`, flow 2026.04. Both halves reproduced; neither is a solver defect.

**Oil bias.** The historical "8–10%" compared ResSim's end-of-step rate × report step against
Flow's cumulative `FOPT`. Integrating ResSim's rate history instead, on the three tracked
quarter-day controls (`opm/reference-decks/water-pressure-*`), gives this:

| Cumulative oil at t = 0.25 d | Flow | ResSim | ResSim vs Flow |
|---|---|---|---|
| 23×23×1, 1 × 0.25 | 336.24 (1 substep) | 344.08 (3) | +2.33% |
| 23×23×1, 25 × 0.01 | 356.46 | 356.20 | −0.07% |
| 22×22×1, 1 × 0.25 | 340.14 (1) | 348.31 (4) | +2.40% |
| 22×22×1, 25 × 0.01 | 361.01 | 360.71 | −0.08% |
| 20×20×3, 1 × 0.25 | 762.52 (1) | 760.61 (1) | −0.25% |
| 20×20×3, 25 × 0.01 | 806.14 | 803.43 | −0.34% |

At 2 days with matched 0.05-day steps the differences are −0.28%, −0.28% and −0.17%.

- **The single-step gap is temporal.** ResSim substeps through the injection transient and Flow
  does not, so ResSim is the closer of the two to the converged value there: −3.4% against
  Flow's −5.6% on 23×23×1. This confirms and refines WATER-028.
- **The matched-step remainder is deck mapping**, in two parts.
  - The decks carry a 9-knot SWOF while the preset uses analytic Corey. With
    `--corey-table-points 9`, injection agrees to ≤ 0.004%.
  - The decks' `PVCDO 300 1.0 1e-5` sets Bo = 1 **at 300 bar**, while ResSim's no-table oil FVF is
    `b_o·exp(−c_o·p)`, i.e. Bo = 1 **at 0 bar**, which is 0.30% more surface oil at 300 bar.
    Setting the deck's reference Bo to `exp(−0.003)` makes the 23×23×1, 5 × 0.05 rung agree to
    **+0.002%** (348.791 against 348.783). **Fixed engine-side by #36** (next section): with
    the unmodified decks, every matched rung now agrees to ≤ 0.005%.

**Report-step sensitivity** (`wf_bl1d` geometry, `small-direct/ow-1d-96`, 30 days):

| report dt | Flow FOPT | ResSim vs Flow FOPT | Flow / ResSim substeps |
|---|---|---|---|
| 2.0 | 1235.77 | +0.98% | 23 / 17 |
| 1.0 | 1243.68 | +0.09% | 32 / 33 |
| 0.5 | 1255.98 | +0.10% | 61 / 62 |
| 0.25 | 1263.69 | +0.05% | 121 / 120 |
| 0.1 | 1268.84 | +0.07% | 301 / 300 |
| 0.05 | 1270.71 | +0.07% | 601 / 600 |

Both engines move by the same 2.8% across the ladder, so the sensitivity is temporal
discretization shared with Flow. ResSim no longer fragments here (one substep per report step,
after FIM-DIRECT-001). At 2-day steps Flow's own controller substeps more (23 vs 17), hence the
+1% rung.

Replay:

```bash
python3 tools/opm_flow/water_control_ladder.py '[["23x23x1",0.25,[0.25,0.05,0.01]],["22x22x1",0.25,[0.25,0.05,0.01]],["20x20x3",0.25,[0.25,0.05,0.01]]]'
RS_EXTRA="--corey-table-points 9" python3 tools/opm_flow/water_control_ladder.py '[["23x23x1",0.25,[0.05,0.01]]]'
# wf_bl1d ladder: per dt, write decks and run ResSim with OPM_SMALL_REPORT_DT=<dt>, then
python3 tools/opm_flow/compare_small_direct.py --deck-dir <decks> --ressim-dir <out> --flow-dir <flow> --case ow-1d-96
```

The oil-FVF reference-pressure convention (0 bar for oil, 300 bar for water and rock) is a
separate modelling decision and is tracked in [#36](https://github.com/sergeyfarin/ressim/issues/36). It moves reported surface oil by
0.3% at 300 bar in every dead-oil scenario.

## #36 — oil FVF is referenced to the initial pressure, like water and rock (2026-09-23)

Without a PVT table, oil FVF was `b_o·exp(−c_o·p)`, so `b_o` held at 0 bar, while water's `b_w`
and the rock reference held at the initial pressure (`set_initial_pressure`). It is now
`b_o·exp(−c_o·(p − p_ref))` with `p_ref` set together with the other two. That is the Eclipse
`PVCDO` convention, and it matches the UI's label for `volume_expansion_o`, "Oil formation
volume factor". Every dead-oil scenario, FIM and IMPES, reports about `c_o·p_init` less surface
oil than before: 0.30% at 300 bar. Nothing with a PVT table changes.

Against Flow, the tracked water-pressure decks, unmodified, with `--corey-table-points 9`:

| rung | ResSim vs Flow FOPT | injection |
|---|---|---|
| 23×23×1, 5 × 0.05 to 0.25 d | +0.002% (was +0.30%) | +0.003% |
| 23×23×1, 25 × 0.01 to 0.25 d | +0.002% | +0.004% |
| 22×22×1, 5 × 0.05 to 0.25 d | +0.002% | +0.003% |
| 20×20×3, 5 × 0.05 to 0.25 d | −0.005% | −0.003% |
| 23×23×1, 40 × 0.05 to 2 d | +0.002% | +0.002% |
| 20×20×3, 40 × 0.05 to 2 d | −0.004% | −0.004% |

Three scenario tests moved. Each change is justified in the test itself.

- **`dep_arps`: late-time error against the layer-superposition decline fell about 3×.** It went
  from 1.9/1.5/2.0% to 0.5/0.7/0.8%, and the final-rate ratio from 1.019/1.008/1.007 to
  1.004/0.993/0.992. The minimum early-transient error it demands was lowered from 0.08 to
  0.07, because the smallest early error fell to 0.079 once the offset was gone. Its real
  claim, early error at least 4× the late error, is unchanged and holds.
- **`dep_pss` now infers the Dietz shape factor from the reservoir-volume rate.** The surface rate
  had assumed Bo = 1, and C_A amplifies a productivity error about 16×. With reservoir volumes,
  master and this change infer identical C_A to 1e-12, all within 0.02–2.4% of Dietz. The 3%
  tolerance is unchanged.
- **`wf_numerics`: recovery is surface oil over oil in place at `b_o`.** It is now unbiased, where
  before it read 0.3% high. The steep and fine runs sit 0.0053 and 0.0057 from their Buckley–
  Leverett values. The bound was 0.005, which passed only on the bias, and is now 0.0075, a
  factor of three inside the ≥ 0.02 separation the test exists to show.

