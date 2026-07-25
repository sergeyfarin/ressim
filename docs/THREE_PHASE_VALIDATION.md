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
20 points, c_o = 1e-5/bar)` from `src/lib/physics/pvt.ts` — the scenario's own table. The same 20
rows are emitted verbatim into the deck's `PVTO`/`PVDG` and embedded in the Rust test, so all
three read identical fluid properties. SCAL is the scenario's own Corey curves, evaluated with
the same effective-saturation definitions as `relperm.rs` and emitted as tabular `SWOF`/`SGOF`;
the deck selects `STONE2` explicitly, because Flow's default three-phase oil model is not
Stone II and the engine uses Stone II (`relperm.rs::k_ro_stone2`).

**Reference.** `flow 2026.04` on `tools/opm_flow/opm_flow_tool/cases.py::GAS_DRIVE`. The parsed
series are committed as `src/lib/catalog/opm-flow-results/gas_drive.json` (status `parsed`) and
overlaid on the scenario's charts; the Rust test embeds the same samples so the engine can be
graded without the frontend.

**Where.** `src/lib/ressim/src/tests/three_phase_acceptance.rs`.

| Criterion | Tolerance | Worst measured error |
|---|---|---|
| Field average reservoir pressure, 11 checkpoints to 600 d | 3 % | 1.588 % (at 50 d) |
| Producing GOR | 12 % | 6.076 % (at 10 d) |
| Cumulative surface oil | 8 % | 4.315 % (at 600 d) |
| Producer surface oil rate, while the reference rate ≥ 10 Sm³/d | 10 % | 4.610 % (at 20 d) |
| Oil material-balance drift vs STOIIP | 1 % | 0.0097 % |
| Gas material-balance drift vs gas in place (free + dissolved) | 1 % | 0.0080 % |
| Solver warnings during the run | none | none |

**Why cumulative oil carries the late-time oil comparison.** The producer's oil rate decays from
40 Sm³/day to 0.03 Sm³/day over the run. Past ~100 days the absolute difference from the
reference is a fraction of a cubic metre per day while the relative difference grows to ~11 %,
which measures nothing useful. The instantaneous rate is therefore graded only while the
reference rate is still meaningful (≥ 10 Sm³/day, i.e. the first ~50 days), and the well-
conditioned integral is graded over the whole horizon.

**Known bias.** The engine produces systematically *more* oil than the reference — cumulative oil
runs +1.1 % at 10 d rising to +4.3 % by 600 d, monotonically and without sign change. Pressure
and GOR agreement both *improve* with time (0.008 % and 0.117 % at 600 d), so this is an
early-time displacement-efficiency difference that is then locked into the cumulative, not a
drift that keeps accumulating. It is inside the acceptance band and recorded here rather than
tuned away.

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

What *is* residual about oil is its **saturation**: transport solves water and gas explicitly and
sets S_o = 1 − S_w − S_g. So the oil diagnostic answers "does reported oil production track the
oil inventory?" — it is not an independent check that the saturation constraint itself closes.
The constraint is enforced by construction, so it cannot fail; what can fail is the reporting and
FVF path between them, and that is what is graded.

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

Engine revision `a651c02` ("Make validate-solver-coverage.sh fail on gates that never run") with
the acceptance tests and the `gas_drive` PVT upgrade applied on top, measured 2026-07-25. Solver
FIM (the scenario default). Reference: `flow 2026.04`, deck hash
`dff10045676a6f1c4a7923b81db196ebbff860900c5c36941be47ac5146d1d45`.

Verbatim summary from the characterization replay:

```
t=  10.0 pressure_err= 0.801% oil_rate_err= 0.509% cum_oil_err= 1.055% gor_err= 6.076% mb_oil= 0.0027% mb_gas= 0.0024%
t=  20.0 pressure_err= 1.426% oil_rate_err= 4.610% cum_oil_err= 0.491% gor_err= 5.854% mb_oil= 0.0038% mb_gas= 0.0034%
t=  30.0 pressure_err= 1.523% oil_rate_err= 3.305% cum_oil_err= 0.204% gor_err= 5.723% mb_oil= 0.0047% mb_gas= 0.0041%
t=  50.0 pressure_err= 1.588% oil_rate_err= 4.111% cum_oil_err= 0.983% gor_err= 5.444% mb_oil= 0.0059% mb_gas= 0.0052%
t= 100.0 pressure_err= 1.300% oil_rate_err=     -- cum_oil_err= 2.464% gor_err= 4.109% mb_oil= 0.0077% mb_gas= 0.0066%
t= 150.0 pressure_err= 0.840% oil_rate_err=     -- cum_oil_err= 3.388% gor_err= 2.634% mb_oil= 0.0086% mb_gas= 0.0073%
t= 200.0 pressure_err= 0.497% oil_rate_err=     -- cum_oil_err= 3.861% gor_err= 1.569% mb_oil= 0.0091% mb_gas= 0.0076%
t= 300.0 pressure_err= 0.175% oil_rate_err=     -- cum_oil_err= 4.181% gor_err= 0.535% mb_oil= 0.0095% mb_gas= 0.0079%
t= 400.0 pressure_err= 0.064% oil_rate_err=     -- cum_oil_err= 4.271% gor_err= 0.237% mb_oil= 0.0097% mb_gas= 0.0080%
t= 500.0 pressure_err= 0.023% oil_rate_err=     -- cum_oil_err= 4.303% gor_err= 0.147% mb_oil= 0.0097% mb_gas= 0.0080%
t= 600.0 pressure_err= 0.008% oil_rate_err=     -- cum_oil_err= 4.315% gor_err= 0.117% mb_oil= 0.0097% mb_gas= 0.0080%
gas-flood breakthrough: dt=1.0 -> Some(4.0) days, dt=0.5 -> Some(4.0) days
```

Provisional until rerun on the committed revision that contains these tests; the engine under
test is `a651c02` plus the reporting fix in section 4, which is the only engine-behavior change
in this pass.

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
- **`gas_injection` has no OPM reference of its own.** It is covered indirectly by SPE1 (same
  mechanism, graded) and by the gas-front criteria in section 3.
- **Gravity-dominated three-phase segregation** is exercised by `gas_cap.rs` as behavior, not
  against an external reference.
- **The +4 % cumulative-oil bias** in section 2 is inside the band but unexplained.
