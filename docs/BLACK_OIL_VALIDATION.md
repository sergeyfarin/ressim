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

**Classified 2026-09-24 (#12): accepted characterization, oracle OPM Flow at matched resolution.**
The published reference is itself a 10×10×3 result, so the refined grid's departure from it says
nothing about which one is right. `tools/opm_flow/spe1_refinement_oracle.py` derives a 20×20×3 deck
from the committed SPE1 case (same domain, half-size cells, producer in the far corner) and runs
Flow on both grids. Flow shows the same refinement effect: an earlier, sharper GOR rise.

| producing GOR [m³/m³] | 630 d | 720 d | 810 d | 1080 d | 1800 d | 3600 d |
|---|---|---|---|---|---|---|
| Flow 10×10 | 220.9 | 347.4 | 649.6 | 1305.6 | 1904.7 | 4148.2 |
| ResSim 10×10 | 221.4 | 368.3 | 695.9 | 1321.9 | 1871.2 | 3894.8 |
| Flow 20×20 | 236.1 | 443.7 | 733.9 | 1328.9 | 1904.1 | 3997.1 |
| ResSim 20×20 | 241.7 | 475.5 | 781.9 | 1340.0 | 1881.5 | 3729.0 |

ResSim tracks Flow equally well on both grids. The worst difference over 90–3600 d at 90-day
checkpoints is pressure 3.06 % / 3.03 %, oil rate 2.73 % / 2.81 % and GOR 7.13 % / 7.17 %
(10×10 / 20×20). Refinement therefore does not open a gap to an independent simulator. The 31 %
GOR "error" at 730 d is the sharper front measured against a coarser grid, and stays a
characterization, not a criterion.

```bash
python3 tools/opm_flow/spe1_refinement_oracle.py --out /tmp/spe1-refinement
cargo test --release --manifest-path src/lib/ressim/Cargo.toml spe1_areal_refinement_reference_error_replay -- --ignored --nocapture
```

```bash
cargo test --release --manifest-path src/lib/ressim/Cargo.toml spe1_areal_refinement_reference_error_replay -- --ignored --nocapture
```

## 2. Grid convergence for black-oil depletion

**Case.** 1D column, 1000 m × 200 m × 20 m, φ = 0.2, k = 100 mD, initial pressure 175 bar
(undersaturated, Rs = 15 Sm³/Sm³, bubble point 150 bar), producer at the far end on a 120 bar
BHP, no gas redissolution, 20 × 5-day steps. The same physical domain is discretized at 5, 10,
20 and 40 cells; pore-volume-weighted field averages of pressure, Rs, Bo and free-gas saturation
must form a converging sequence. The PVT table is three rows: (100 bar, Rs 5, Bo **1.08**,
Bg 0.01), (150, 15, 1.12, 0.006) and an undersaturated row (200, 15, 1.119). Both solvers run at
the default step controls (IMPES: 0.05 saturation, 75 bar).

**Where.** `src/lib/ressim/src/tests/physics/depletion_grid_convergence.rs`. The OPM Flow twin
of the column is [`small-direct`](../opm/reference-decks/small-direct/README.md) `bo-1d-10` and
`bo-1d-40`, generated from the same fixture.

Criteria per quantity: each successive refinement difference must be at most 0.8× the previous
one (first-order upstream transport gives ~0.5–0.6 here). The two finest grids must agree to
within 1 %, or 1.5 % for Sg (the reason is in the test). The case also asserts it is genuinely
below the bubble point with liberated free gas, so a degenerate state cannot pass silently. A
guard asserts the table is thermodynamically stable (`dBo/dp < Bg·dRs/dp` below the bubble
point).

### Baselines at `5fa21a2` (both default gates; IMPES dissolved-gas limit, #44)

| nx | IMPES p / Rs / Bo / Sg | FIM p / Rs / Bo / Sg | OPM Flow p / Rs / Sg |
|---|---|---|---|
| 5 | 129.0697 / 10.81394 / 1.102907 / 0.024349 | 129.1342 / 10.82685 / 1.102959 / 0.024281 | |
| 10 | 129.7699 / 10.95397 / 1.103470 / 0.023405 | 129.8390 / 10.96780 / 1.103526 / 0.023330 | 129.8389 / 10.96777 / 0.023330 |
| 20 | 130.1497 / 11.02994 / 1.103776 / 0.022896 | 130.2210 / 11.04420 / 1.103834 / 0.022816 | |
| 40 | 130.3533 / 11.07067 / 1.103940 / 0.022623 | 130.4234 / 11.08469 / 1.103997 / 0.022545 | 130.4151 / 11.08302 / 0.022556 |

- Both solvers contract monotonically in every quantity.
- FIM matches Flow to 1e-4 bar at nx = 10 and 0.008 bar / 1.1e-5 Sg at nx = 40. IMPES sits
  0.062 bar and 0.3 % Sg from Flow at nx = 40, and its oil, gas and water balance errors are at
  roundoff (~1e-12 relative).
- **Agreement with Flow at 5-day steps is not accuracy here.** FIM and Flow share an implicit
  scheme and its time error. Flow re-run at 0.1-day reports (`validate-cross-solver.sh --refine 0.1`)
  reads 129.6121 / 130.2013 bar and Sg 0.023628 / 0.022836 at nx = 10 / 40. Against that, at nx = 40,
  FIM is +0.222 bar / −1.3 % Sg and IMPES +0.152 bar / −0.9 % Sg. Before #44, IMPES was
  +0.205 bar / −1.3 %, so the dissolved-gas limit moved IMPES away from Flow at the same step and
  closer to the time-converged answer. Details: the small-direct README's #44 section.
- Flow is re-run on regenerated `small-direct` `bo-1d-*` decks. Their undersaturated PVTO rows are
  written by ResSim's own `interpolate_oil`, so before #38 they had encoded its `c_o` extrapolation
  above the bubble point. This column's 200-bar row (Bo 1.119) does not follow `c_o`, which is why
  this case moved while SPE1 and the three-phase acceptance cases did not.
- Superseded: the IMPES columns at `1b7853e` (before #44, which cut IMPES substeps where Rs
  changes fast) read 129.1222 / 129.8236 / 130.2046 / 130.4062 bar and Sg 0.024266 / 0.023319 /
  0.022807 / 0.022537. The FIM columns and the Flow values did not change.
- Superseded: the table at `30bde0d` (conservative IMPES, #37, before #38) read FIM 129.0867 /
  129.7853 / 130.1644 / 130.3631 bar, IMPES 129.0519 / 129.7480 / 130.1254 / 130.3250 bar, and
  Flow 129.7852 / 130.3652 bar at nx = 10 / 40. It is superseded because the table itself changed.
  The solver-to-Flow agreement is the same order on both. The IMPES columns at `c225102` (2 bar
  cap, before #37) read 128.8256 / 129.5157 / 129.8887 / 130.0862 bar and Sg 0.024714 / 0.023777 /
  0.023273 / 0.023008, with a +3.6 % oil balance error at nx = 10.
- Flow values are pore-volume-weighted averages of `PRESSURE`, `RS` and `SGAS` at report
  step 20, from the same decks. The FIM/Flow cell and convergence comparison is in the
  small-direct README.

```bash
cargo test --release --manifest-path src/lib/ressim/Cargo.toml physics_depletion_ -- --nocapture --test-threads=1
```

### #11: the old IMPES/FIM/Flow gap was an unstable PVT table (resolved 2026-09-23)

Until `c225102` the 100 bar row had Bo = 1.05. That gives `dBo/dp = 1.4e-3 > Bg·dRs/dp ≈ 1.2e-3`
below the bubble point: the two-phase volume factor `Bt = Bo + (Rs_i − Rs)·Bg` **grew** with
pressure, so the total compressibility of a saturated cell was negative until Sg reached about
1–2 %. FIM and Flow solve mass equations directly, and they still agreed with each other on that
table (the superseded tables below). IMPES cannot represent that table:

- IMPES did not transport oil (fixed by #37, next subsection). `So = 1 − Sw − Sg` was whatever
  was left over, so any mismatch
  between its pressure equation and the transport closure becomes oil that was produced but
  never left the reservoir. The step report's `material_balance_error_oil_m3` measures this
  directly. It was **+144 %** of produced oil (1980 Sm³ of 3359) at nx = 40, with no dependence
  on grid or dt. FIM was at 0.004 %.
- Its storage term cannot go negative. `saturated_c_o_eff` replaced the negative saturated value
  with the scalar `c_o` (section 3), and no clamp can fix it, because a negative `c_t` leaves the
  explicit pressure equation ill-posed. Per-cell attribution closed to 0.1 Sm³. 95 % of the
  error arose in saturated cells holding little free gas, at about −17 Sm³ per bar of pressure
  drop, and it vanished by Sg ≈ 2 %.
- That is why shrinking IMPES's steps moved it *away* from Flow (the 0.84 bar / 4.4 % Sg limit
  recorded on #11).
- FIM breaks on the same table as well, just below the bubble point. At BHP 145 it took 10,853
  substeps, ended with average pressure 128 bar (below the BHP) and 300 % balance errors. With
  Bo = 1.08 the same run takes 22 substeps and conserves exactly. **Explained in #39**
  (re-measured at `884035b`: 3,729 substeps, every cell at 129.4 bar):
  - *Below the BHP.* The whole drop happens after the producer shuts in, with zero oil-balance
    change (step 1: 151 → 129 bar, no production). The unstable table makes the fluid volume
    non-monotone in pressure, so the closed column has a second pressure for the same masses and
    relaxes to it.
  - *The balance "errors".* All of them arise in the first 5-day step (3,708 substeps through the
    132–150 bar window, where the storage term's pressure slope changes sign). Each substep is
    inside FIM's material-balance tolerance (about 5e-8 of oil in place against a 1e-5 bound), and
    together they reach 2e-4 of oil in place. That is large only next to the small production.

  The input is ill-posed, so this is acceptable for it. The remedy is detection:
  `PvtTable::thermodynamically_unstable_ranges` in the engine, and a `pvt-thermodynamically-unstable`
  pre-run warning (`validateInputs.ts`) that names the range. No shipped scenario or variant
  triggers it. The one system fixture that depleted through such a table
  (`make_below_bubble_point_flash_sim`) moved to Bo = 1.08. Four unit fixtures keep it on
  purpose, and each is commented.

### #37: conservative three-phase IMPES (2026-09-23, `30bde0d`)

With a stable table, the residual-oil IMPES still lost oil to time discretization. A substep that
crossed the bubble point took its storage term from the undersaturated state and booked the
liberation as oil. At `c225102`, sweeping IMPES's pressure cap on nx = 10/40 gave this:

| cap (bar) | 75 | 20 | 10 | 5 | 2 | 1 (sat 0.01) |
|---|---|---|---|---|---|---|
| oil MB error before #37, nx=10 / 40 | +36 / +39 % | +20 / +18 % | +8.5 / +12 % | +7.7 / +8.8 % | +3.6 / +3.8 % | +2.3 / +2.5 % |
| p nx=40 before #37 (FIM 130.36) | 129.01 | 129.73 | 129.78 | 129.89 | 130.09 | 130.12 |
| p nx=40 after #37 | 130.33 | 130.31 | | | 130.29 | |

Three-phase IMPES now transports all four black-oil masses (water, stock-tank oil, free gas,
dissolved gas) as surface volumes. It recovers the cell state by flashing them at the new
pressure on `Vp(p)`, using FIM's rock law (`impes/closure.rs`). After the linear pressure solve, a
Newton loop drives `V(p, N(p)) = Vp(p)`. Its matrix is the assembled pressure operator with the
storage diagonal replaced by the closure's own slope, and its right-hand side is the volume
residual over dt. It converges when every cell's residual is worth less than 1e-4 bar. The linear
solve's `c_t` (section 3) only supplies the first guess, so a poor storage estimate costs
iterations rather than mass. After #37:

- Oil, gas and water balance errors are at roundoff at every cap (1e-12 relative), with
  `|ΣS − 1| ≤ 2e-8`. The regression bounds the oil error at 1e-8 relative, the pressure gap to
  FIM at 0.1 bar and the Sg gap at 0.5 %.
- Rock and water compressibility close too. With `c_r = 1e-4`, `c_w = 4.5e-5` on nx = 10, the
  old IMPES reported 3183 Sm³ of oil error against 9536 Sm³ produced. The error is now at
  roundoff (`physics_depletion_impes_closes_balances_with_rock_and_water_compressibility`, in
  the `impes` bucket).
- Well withdrawals use the conversions the step report applies, so reported production is the
  mass removed. In three-phase mode the IMPES report balances water in surface volume, like FIM.
- A volume-only tolerance was tried first and rejected. On the single-cell liberation contract,
  where the 80 bar BHP sits below the table's first row and oil and gas are clamped
  incompressible, storage is only `c_w·Sw` ≈ 3e-7 /bar. A 1e-6·Vp volume leftover is then 3 bar
  of pressure, which the next substep drove below the BHP.
- Two-phase IMPES is unchanged: it still moves water by volume and keeps oil as `1 − Sw`. The
  Buckley–Leverett benchmarks and the native/wasm binding matrix are identical.

Smaller IMPES effects, measured and real but not the cause of #11:

- `PvtTable::interpolate_oil` is discontinuous above the highest bubble point. Rs equal to the
  saturated value takes the `c_o` extrapolation, while an Rs just below it takes the
  undersaturated branch rows, a relative jump of up to 2e-4 in Bo ([#38](https://github.com/sergeyfarin/ressim/issues/38)).
- IMPES counted water and rock expansion in its pressure `c_t` but transported water by volume
  on a fixed pore volume. Fixed in three-phase mode by #37.
- The true-IMPES flux weighting (`Bo_i − Bg_i(Rs_i − Rs_up)` over `Bo_up`, and `Bg_i/Bg_up`) was
  tested and ruled out: it changes pressure by 5e-4 bar.

The probe behind these numbers is a diagnostic patch. It was not merged, and its commands and
the per-cell attribution are on the issue.

### Superseded baselines (unstable Bo = 1.05 table, before `c225102`)

Kept for the #11 record. These numbers are not comparable with the table above.

| nx | IMPES (75 bar cap) p / Rs / Bo / Sg | FIM after FIM-BUBBLE-001 p / Rs / Bo / Sg | OPM Flow p / Rs / Sg |
|---|---|---|---|
| 5 | 122.5011 / 9.50022 / 1.080389 / 0.033903 | 122.0752 / 9.41504 / 1.079797 / 0.034681 | |
| 10 | 122.7612 / 9.55224 / 1.080751 / 0.033496 | 122.2670 / 9.45340 / 1.080063 / 0.034384 | 122.2669 / 9.45338 / 0.034385 |
| 20 | 122.8931 / 9.57862 / 1.080934 / 0.033293 | 122.3742 / 9.47484 / 1.080212 / 0.034214 | |
| 40 | 122.9711 / 9.59422 / 1.081042 / 0.033171 | 122.4318 / 9.48635 / 1.080292 / 0.034124 | 122.4301 / 9.48602 / 0.034127 |

Before FIM-BUBBLE-001 the FIM replay took ~20,000 substeps to cross the bubble point and its Sg
was non-monotone (0.029940 / 0.030042 / 0.030277 / 0.030044). That is why the FIM test used to be
`#[ignore]`d with a 5 % Sg spread bound instead of a contraction check. Both were removed in
`c225102`.

## 3. Black-oil solver safeguards (read this before interpreting results)

These are deliberate, documented deviations from a textbook black-oil formulation. They keep the
pressure solve well-posed; they also mean a few reported quantities are approximations.

**Effective oil compressibility below the bubble point.** In three-phase mode the IMPES pressure
accumulation term uses `get_c_o_effective(p, Rs_cell)` (`src/lib/ressim/src/pvt.rs`,
`src/lib/ressim/src/impes/pressure.rs`) rather than a raw `-1/Bo · dBo/dp`. Since #37 this only
seeds the first linear pressure solve. The volume-balance iteration that follows takes its
storage from the mass closure itself, so none of the approximations below reach the conserved
masses (section 2):

- Saturated cells use `c_eff = -(dBo/dp)/Bo + (Bg/Bo)·dRs/dp`, i.e. rock-fluid storage plus the
  dissolved-gas contribution, evaluated by central difference on the PVT table.
- If that combination is non-finite or non-positive — which a saturated `Bo(p)` slope alone can
  produce, since Bo *increases* with pressure below the bubble point — the code falls back to
  the scenario's positive scalar `c_o`. A negative accumulation coefficient would make the
  pressure matrix indefinite and destabilize the IMPES solve.
  **A non-positive `c_eff` means the table is thermodynamically unstable** (`Bt` grows with
  pressure). A physical table never takes this fallback, and when an unstable table does, IMPES
  misstates storage and books the error as oil (#11, section 2).
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
stock-tank inventory depletion. In two-phase IMPES, oil *saturation* is still residual by
construction (`S_o = 1 - S_w`), so there the oil diagnostic checks reporting/FVF closure rather
than an independently transported oil equation. Three-phase IMPES has transported oil mass since
#37, and FIM solves an oil mass equation, so on both of those the oil diagnostic is a genuine
conservation check. The SPE1 acceptance criteria therefore grade
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
- Two-phase IMPES still keeps oil as the residual `1 − Sw` and moves water by volume on a fixed
  pore volume, so rock and water expansion there are booked as oil. Three-phase IMPES was made
  conservative by #37 (section 2).
- Saturated PVT tables with `dBo/dp > Bg·dRs/dp` are accepted, with a pre-run warning naming the
  unstable range since #39. They break IMPES and fragment FIM through that range (section 2).
- SPE1 scenario wiring is covered (#12). Reference-panel placement is pinned in
  `referenceComparisonModel.test.ts`. The layer thicknesses (20/30/50 ft) and each well's
  completion layer, physical-well id, surface-rate target and BHP limit are checked in
  `src/lib/workers/configureSimulator.test.ts`, through the worker's own setup function rather
  than a test replica of it.

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
| Multi-completion gravity | [#10](https://github.com/sergeyfarin/ressim/issues/10) | **no** (reconstruction) | no warning on either solver; IMPES/FIM agree to 1 % on pressure, 5e-3 on peak `Sw` | shared well geometry, Peaceman PI, `refresh_well_head_offsets` | no independent geometry oracle | **not reproduced; cause/applicability inconclusive.** Superseded 2026-09-24: the exact replay reproduced it; four IMPES multi-completion defects, fixed ([#10](https://github.com/sergeyfarin/ressim/issues/10), `docs/CASE_LIBRARY_ROADMAP.md`) |
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

The #11 replay above is historical. Since `c225102` the FIM test is no longer `#[ignore]`d, and it
runs on the stable table from section 2.

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
  between two solvers that reuse the same geometry cannot exclude an error in that shared code. *(The
  exact replay, run 2026-09-24, did reproduce it. The cause was IMPES-only, not shared geometry.)*
- #21 was not attempted by F6; it was executed separately on 2026-09-23 (section below).
- #11's 9 % gap is measured, not explained. Nothing here identifies a mechanism. *(Explained
  2026-09-23: an unstable PVT table in the test fixture. See section 2.)*

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

