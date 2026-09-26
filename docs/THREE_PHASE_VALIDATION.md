# Three-Phase Validation

Authoritative record for how the three-phase (oil/water/gas) path is graded: the exit criteria
for leaving `experimental` status, the comparative-solution acceptance criteria against OPM Flow
and SPE1, the gas-front behavior criteria, and an honest statement of what the material-balance
diagnostics do and do not report.

Companion documents: `docs/THREE_PHASE_IMPLEMENTATION_NOTES.md` (architecture and parameter
reference), `docs/BLACK_OIL_VALIDATION.md` (SPE1 acceptance and black-oil safeguards),
`docs/UNIT_SYSTEM.md` (units and equations).

## 1. The bar for leaving `experimental`

Three-phase mode was labelled `experimental` because validation lagged implementation. The
following five conditions define what "validated" means for it. All five are met as of the
baseline recorded in section 5; the label was removed on 2026-07-25.

1. **A comparative-solution anchor on each drive mechanism.** Gas *injection* is graded against
   SPE1 Case 1 (`docs/BLACK_OIL_VALIDATION.md` §1). Solution gas *drive* is graded against an
   OPM Flow reference solution (section 2 below). One anchor covering only injection is not
   sufficient, because liberation of dissolved gas is not exercised by an injection case that
   stays saturated.
2. **The reference must read the same fluid.** The acceptance case's PVT table, SCAL curves,
   grid, wells and initial state are shared between the engine and the reference deck by
   construction, not approximated. A mismatch in the input makes a mismatch in the output
   uninterpretable.
3. **The named mechanism must be asserted, not just the curves.** A case called "solution gas
   drive" must be shown to actually drop below the bubble point, strip Rs out of the oil, build
   free gas, and raise the producing GOR. Matching pressure and rate curves alone can be
   achieved without any of that happening.
4. **Gas-front behavior must be graded quantitatively.** Breakthrough timing inside an
   acceptance band and stable under timestep refinement; the gas-saturation profile monotone in
   space and advancing monotonically in time. Bounds-and-finiteness checks are not enough.
5. **All three phases must close, explicitly.** Water, gas *and* oil material-balance drift each
   inside a stated tolerance, with oil graded as its own quantity rather than inferred from the
   saturation constraint (section 4).

Tolerances throughout are acceptance criteria with deliberate headroom over the measured error,
not benchmark tolerances tuned to the current build. They are not to be widened to make a change
pass; a regression that breaks one is a physics or solver finding.

## 2. Solution gas drive vs OPM Flow

**Case.** The `gas_drive` catalog scenario: 20 × 1 × 1 slab of 50 m × 50 m × 10 m cells, φ = 0.2,
k = 100 mD, a single BHP-controlled producer at 100 bar in the last cell, no injector, gravity
off, 60 × 10-day steps. The reservoir starts *saturated* — initial pressure 200 bar equals the
PVT table's bubble point, with 8 % initial free gas — so drawdown liberates dissolved gas from
the first step rather than first traversing an undersaturated leg.

**Fluid.** `generateBlackOilTable(35 API, 0.75 gas gravity, 80 °C, Pb = 200 bar, Pmax = 300 bar,
20 points, c_o = 1e-5/bar)` from `src/lib/physics/pvt.ts`, the scenario's own table, embedded in
the Rust test as `gas_drive_pvt_rows`. SCAL is the scenario's Corey curves; the engine and the deck
both use Stone II (`relperm.rs::k_ro_stone2`, `STONE2`).

**Reference.** `flow 2026.04` on `tools/opm_flow/opm_flow_tool/cases.py::GAS_DRIVE`. Since #55 its
deck is `opm/reference-decks/small-direct/gas-drive-20`, written by `opm_small_direct.rs` from the
test's own simulator (`make_gas_drive_acceptance_sim`): PVT and relperm are ResSim's functions
sampled onto dense nodes (91 saturation points, plus geometric nodes just above S_gc since #55), so the deck cannot disagree with the engine about
its inputs. The hand-written deck it replaced sampled SGOF at ten nodes, and linear interpolation
between them overstated k_rg by up to 41 % around the initial S_g = 0.08. That put the reference
4–6 % off this model, and the "+4 % cumulative-oil bias" recorded here until then came from it.
The parsed series are committed as `src/lib/catalog/opm-flow-results/gas_drive.json` (status
`parsed`) and overlaid on the scenario's charts, and the Rust test embeds the same samples. The
OPM curves are shown by default (#12). The scenario tests also check the solution-gas-drive story on
the base rung: a saturated start (Rs = Rs_sat at 200 bar), liberation (each cell's Rs on the
saturated curve at its pressure, mean Rs below 80 % of the bubble-point value by 300 d), and a
producing GOR above ten times the solution GOR that rises every step (386 to 514 m³/m³).

**Where.** `src/lib/ressim/src/tests/three_phase_acceptance.rs`.

The tolerances were tightened in #55, with the justification in the test. Measured errors are
generated in [`BENCHMARKS.md`](BENCHMARKS.md) §3.

| Criterion | Tolerance |
|---|---|
| Field average reservoir pressure, 11 checkpoints to 600 d | 1 % (was 3 %) |
| Producing GOR | 1 % (was 12 %) |
| Cumulative surface oil | 3 % (was 8 %) |
| Producer surface oil rate, while the reference rate ≥ 10 Sm³/d | 8 % (was 10 %) |
| Oil material-balance drift vs STOIIP | 1 % |
| Gas material-balance drift vs gas in place (free + dissolved) | 1 % |
| Solver warnings during the run | none |

**Why cumulative oil carries the late-time oil comparison.** The producer's oil rate decays from
40 Sm³/day to 0.03 Sm³/day over the run. Past ~100 days the absolute difference from the
reference is a fraction of a cubic metre per day while the relative difference grows to ~11 %,
which measures nothing useful. The instantaneous rate is therefore graded only while the
reference rate is still meaningful (≥ 10 Sm³/day, i.e. the first ~50 days), and the well-
conditioned integral is graded over the whole horizon.

**Where the remaining error sits.** The oil-rate and cumulative-oil worsts are at 20 d, in the
steep first transient, and they are time discretization. With both simulators refined to 1-day
reports (`validate-cross-solver.sh --refine 1 --case gas-drive-20`) the 20 d gaps fall from +3.9 %
oil rate and −1.37 % cumulative oil to −0.48 % and +0.04 %, and by 50 d to 0.02–0.04 %. Neither
simulator is time-converged there at 10-day reports: refining Flow alone moves its own 20 d oil rate
from 27.6 to 26.1 Sm³/d. By 600 d cumulative oil agrees to 0.09 % at the scenario's own steps.

**JutulDarcy against Flow on the same deck** (#55) has the same explanation. At the deck's 10-day
reports they differ by −2.33 % in oil rate at 10 d, −0.68 % at 20 d, and ≤ 0.43 % from 30 d on:
the two simulators choose different internal steps inside a steep first report. With the deck
rewritten at 1-day reports (`OPM_SMALL_REPORT_DT=1`), JutulDarcy 0.3.7 and Flow agree to ≤ 0.04 %
on FOPR, FGPR, FPR and FGOR at every checkpoint from 10 to 600 d. JutulDarcy ignores `STONE2`, but
S_w stays at connate here, where Stone II reduces to k_rog, so that makes no difference.

## 3. Gas-front behavior

**Case.** The 1D gas flood fixture `make_3phase_gas_injection_sim(20, fim)` — 20 cells, gas
injector at cell 0, producer at cell 19, FIM.

**Where.** `src/lib/ressim/src/tests/three_phase_acceptance.rs`.

| Criterion | Tolerance | Measured |
|---|---|---|
| Gas breakthrough at the producer cell (Sg > 1e-3) | 2–8 days | 4.0 days |
| Breakthrough time under timestep halving (dt 1.0 → 0.5) | ≤ 1.5 days movement | 0.0 days |
| Gas saturation monotone decreasing injector → producer, every step | exact to 1e-9 | holds |
| Gas saturation monotone increasing in time in every cell | exact to 1e-9 | holds |
| Furthest invaded cell never recedes; reaches the producer within 30 d | required | holds |

This complements, rather than replaces, `physics_gas_flood_1d_timestep_refinement_keeps_breakthrough_ordering_stable`,
which asserts that refinement preserves the *ordering* of breakthrough. What was missing was the
timing itself, which is what a user reads off the GOR chart.

## 4. What the material-balance diagnostics actually report

This section exists because the previous documentation understated it in one direction and
overstated it in another.

- **Water** — `material_balance_error_m3`: cumulative (injection − production) at reservoir
  conditions versus the actual in-place water volume change. Explicit and direct.
- **Gas** — `material_balance_error_gas_m3`: cumulative (surface gas injection − surface gas
  production, free plus dissolved) versus the actual total-gas inventory change expressed at
  standard conditions. Explicit and direct. Non-zero only in three-phase mode.
- **Oil** — `material_balance_error_oil_m3`: cumulative reported surface oil production versus
  the actual stock-tank oil inventory depletion. **This is a direct diagnostic, not a residual.**
  Earlier documentation described oil as "residual" and "not reported explicitly"; that was
  wrong about the diagnostic and is corrected here.

How strong a check that is depends on the solver. FIM solves an oil mass equation, and since #37
three-phase IMPES transports all four black-oil masses and flashes them at the new pressure
(`impes/closure.rs`). On both, the oil diagnostic is a genuine conservation check. Only two-phase
IMPES still keeps oil as the residual saturation S_o = 1 − S_w. There the constraint holds by
construction, and the oil diagnostic grades the reporting and FVF path rather than an independently
transported oil equation.

Runtime scenarios do not inject oil, so no injection term appears in the oil balance.

### Producing GOR reporting

`producing_gor` used to be forced to exactly `0` whenever the producer's surface oil rate fell
below an absolute 10 Sm³/day floor. That was harmless for a 3180 Sm³/day SPE1 producer and wrong
for a depleting solution-gas-drive well, whose GOR is most interesting precisely once the oil
rate has decayed into single digits — the `gas_drive` case reported GOR = 0 for the last
~500 days of a 600-day run while producing gas the whole time.

The floor is now a denormal-scale divide-by-zero guard
(`reporting.rs::MIN_GOR_OIL_RATE_SC_DAY`). A reported `0` means "no surface oil production", i.e.
the ratio is genuinely undefined — it no longer means "the oil rate is small". Both solver paths
report total GOR including dissolved gas; the IMPES path sums free and dissolved terms
explicitly, the FIM path reads a component gas rate that already includes them.

## 5. Recorded baseline

Committed revision `5c29e0e`, clean tree, measured 2026-09-25 (release). Solver FIM (the scenario
default). Reference: `flow 2026.04`, deck hash
`dff10045676a6f1c4a7923b81db196ebbff860900c5c36941be47ac5146d1d45`.

Verbatim summary from the characterization replay:

```
t=  10.0 pressure_err= 0.801% oil_rate_err= 0.508% cum_oil_err= 1.054% gor_err= 6.076% mb_oil= 0.0000% mb_gas= 0.0000%
t=  20.0 pressure_err= 1.426% oil_rate_err= 4.609% cum_oil_err= 0.492% gor_err= 5.855% mb_oil= 0.0000% mb_gas= 0.0000%
t=  30.0 pressure_err= 1.523% oil_rate_err= 3.304% cum_oil_err= 0.203% gor_err= 5.724% mb_oil= 0.0000% mb_gas= 0.0000%
t=  50.0 pressure_err= 1.588% oil_rate_err= 4.109% cum_oil_err= 0.982% gor_err= 5.445% mb_oil= 0.0000% mb_gas= 0.0001%
t= 100.0 pressure_err= 1.299% oil_rate_err=     -- cum_oil_err= 2.462% gor_err= 4.109% mb_oil= 0.0000% mb_gas= 0.0001%
t= 150.0 pressure_err= 0.839% oil_rate_err=     -- cum_oil_err= 3.386% gor_err= 2.634% mb_oil= 0.0000% mb_gas= 0.0001%
t= 200.0 pressure_err= 0.496% oil_rate_err=     -- cum_oil_err= 3.858% gor_err= 1.569% mb_oil= 0.0000% mb_gas= 0.0001%
t= 300.0 pressure_err= 0.175% oil_rate_err=     -- cum_oil_err= 4.177% gor_err= 0.535% mb_oil= 0.0000% mb_gas= 0.0001%
t= 400.0 pressure_err= 0.063% oil_rate_err=     -- cum_oil_err= 4.266% gor_err= 0.237% mb_oil= 0.0000% mb_gas= 0.0001%
t= 500.0 pressure_err= 0.023% oil_rate_err=     -- cum_oil_err= 4.298% gor_err= 0.147% mb_oil= 0.0000% mb_gas= 0.0001%
t= 600.0 pressure_err= 0.008% oil_rate_err=     -- cum_oil_err= 4.310% gor_err= 0.117% mb_oil= 0.0000% mb_gas= 0.0001%
gas-flood breakthrough: dt=1.0 -> Some(4.0) days, dt=0.5 -> Some(4.0) days
```

Superseded: the provisional baseline on `a651c02` plus the then-uncommitted tests (2026-07-25).
Its errors agree with the above to three decimals. Its balance drift was 0.0097 % oil and
0.0080 % gas, and it is now at the 1e-6 level. The change that caused this was not isolated in
this re-measurement.

### Replay

Acceptance gates (default, ~10 s debug; also run by
`bash scripts/validate-solver-coverage.sh fim`):

```bash
cargo test --manifest-path src/lib/ressim/Cargo.toml three_phase_gas -- --nocapture
```

Characterization replay that prints the measured errors above:

```bash
cargo test --manifest-path src/lib/ressim/Cargo.toml three_phase_acceptance_error_replay -- --ignored --nocapture
```

Regenerating the OPM reference (requires a local `flow`):

```bash
uv run --directory tools/opm_flow python -m opm_flow_tool.cli run-flow gas_drive
```

## 6. What is still not covered

Removing `experimental` is a statement about the graded cases, not a claim of universal
three-phase fidelity. Known remaining gaps:

- **No three-phase analytical reference.** Buckley-Leverett and Dietz are two-phase and oil-only
  respectively; neither represents gas liberation. Three-phase grading is against numerical
  references (OPM Flow, SPE1), not closed-form solutions.
- **Vaporized oil (Rv) is not modelled.** The gas phase carries no oil, so wet-gas and
  gas-condensate behavior is out of the envelope. The OPM decks use `PVDG` (dry gas) accordingly.
- **`gas_injection` is graded against Flow on the identical model** (#12,
  `opm/reference-decks/small-direct/go-1d-50`): cumulative oil and injected gas within 0.05 % at
  every checkpoint (band 0.2 %), gas produced within 0.29 % just after breakthrough and 0.03 % at
  300 d (band 1.5 %), in both the engine test (`three_phase_gas_injection_matches_opm_flow_twin`)
  and a test of the shipped scenario. Since #20 the Flow run is also a parsed artifact, drawn on the
  scenario's charts beside the gas–oil fractional-flow solution.
- **Table-less three-phase gas is compressible since #42.** It had `Bg = 1` at every pressure, and
  `c_g` reached only IMPES's first-guess storage term. It now has `Bg = exp(−c_g·(p − p_ref))`,
  referenced to the initial pressure like the table-less oil (#36), in FIM, in IMPES and in every
  reported inventory. At `c_g = 1e-4` this moved `gas_injection`'s injected gas by +0.9 % at 100 d.
  The Flow twin was regenerated with the new PVDG and still agrees as above.
- **Gravity-dominated three-phase segregation** is exercised by `gas_cap.rs` as behavior, not
  against an external reference.
- **The +4 % cumulative-oil bias** formerly recorded in section 2 was the reference deck's, not
  the engine's (#55); section 2 now grades against a deck generated from the engine setup.