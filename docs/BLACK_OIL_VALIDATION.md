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

### FIM baseline (explicit replay, ~3.5 min release)

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
  a grid-convergence one, and is tracked in `TODO.md` rather than in these tests.

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
default moves; the regression guard for that is still open (`ROADMAP.md` 1.3). Scenarios that
set their own value (SPE1 uses 2.06e-4 /bar) override it on both sides.

**Material-balance diagnostics are not a full per-phase closure.** Water and gas cumulative
errors are reported explicitly; oil is reported against stock-tank inventory change but the
three-phase closure is still residual-based (`docs/UNIT_SYSTEM.md`, note 5). The SPE1 acceptance
criteria above therefore grade oil and gas drift separately and normalize each against its own
inventory.

**Gas redissolution is off in SPE1.** `gasRedissolutionEnabled: false` matches the reference
deck's behavior for this case: liberated free gas does not re-enter solution when pressure
recovers. Scenarios that need redissolution must opt in.

**SCAL is tabular where the deck is tabular.** SPE1 supplies exact SWOF/SGOF tables; the Corey
endpoints in the scenario remain only as fallback metadata for the two-phase path. Scenarios
without tables use the Corey model, which is an approximation of, not a substitute for, deck
tables.

## 4. Known gaps

- Three-phase status is still `experimental`: the exit criteria and the gas-drive breakthrough /
  Sg-evolution acceptance tests are open (`ROADMAP.md` 1.2).
- No SPE-style black-oil case beyond SPE1 (SPE9, volatile-oil style cases) is covered.
- The IMPES/FIM answer gap on the depletion column (section 2) is unexplained.
- Scenario-wiring regressions for SPE1 (published-reference panel placement, `cellDzPerLayer`,
  per-layer completion payloads) remain frontend-side TODO items.
