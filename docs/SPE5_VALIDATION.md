# SPE5 Case 1 on the compositional engine (#52)

This document owns the live SPE5 Case 1 scenario. The fluid, water phase, structured grid, rate
control and WAG schedule are the enablers recorded in `COMPOSITIONAL_VALIDATION.md` §8 and
`SPE3_VALIDATION.md` §“Gas cycling decision”; this page records what the reservoir case is, how it
differs from the OPM deck implementation, and what still blocks offering it.

**Status: authored, not shipped.** `src/lib/catalog/scenarios/comp_spe5_wag.ts` builds the complete
payload and is withheld from the picker. The independent JutulDarcy 0.3.7 compositional reference
for the same payload is committed at `opm/compositional/spe5/reference.json` (240 monthly reports,
7305 d) and recorded below. No UI overlay or scorecard row uses it yet; the scenario stays withheld
until the comparator and published-curve work in the reference plan lands.

[#93](https://github.com/sergeyfarin/ressim/issues/93) — the **6114.808 d** stall — was a
near-critical stability-tolerance defect in the flash, fixed below.
[#99](https://github.com/sergeyfarin/ressim/issues/99) — the **6375 d** region — was a
successive-substitution and iteration-cap limit, also fixed below. The replay clears that region
without a timestep collapse.
[#102](https://github.com/sergeyfarin/ressim/issues/102) — the **6646.0625 d** stability-trial
stall — applied the same acceleration one level lower, to the Michelsen trial. The month-192 →
month-240 replay now completes.

## Case definition

`opm/compositional/spe5/case.json` is the exact payload the scenario sends. It is exported from the
TS definition so the browser and the native replay run one source.

| Item | Value | Source |
|---|---|---|
| Grid | 7×7×3, 500 ft × 500 ft, layers 20/30/50 ft | `SPE5.BASE` `DIMENS`, `DX`, `DY`, `DZ` |
| Depth / pressure | top 8325 ft; 4000 psia at the 8400 ft datum | `TOPS`, `EQUIL` |
| Initial gradient | 0.054611196 bar/m, the pinned EOS density at the datum | `flash(spe5, 4000 psia, 160 °F, z₀)` |
| Rock | φ 0.3; kx/ky 500/50/200 mD; kz 50/50/25 mD | `PORO`, `PERMX/Y/Z` |
| Fluid | OPM `Spe5FluidSystem`, six hydrocarbons, PRCORR | `docs/COMPOSITIONAL_VALIDATION.md` §1 |
| Temperature | 160 °F | Killough & Kossack (1987) |
| Initial composition | C1 0.50, C3 0.03, C6 0.07, C10 0.20, C15 0.15, C20 0.05 | same paper |
| Injection gas | C1 0.77, C3 0.20, C6 0.03 | same paper |
| Water | connate 0.20; PVTW 14.7 psia / Bw 1.0 / cw 3.3e-6 psi⁻¹ / 0.7 cP | `PVTW` |
| Relperm | deck `SWFN`/`SGFN`/`SOF3` on Stone II `SWOF`/`SGOF`, union of both tables' knots; zero Pc | `SWFN`, `SGFN`, `SOF3` |
| Producer | 12000 STB/d oil, 1000 psia BHP floor | `WCONPROD` |
| Water injector | 12000 STB/d, 10000 psia BHP ceiling | `WCONINJE` |
| Gas injector | 12000 Mscf/d, 10000 psia BHP ceiling | `WCONINJE` |
| Schedule | 2 years production, then nine one-year water/gas pairs to year 20 | `SPE5CASE1.DATA` |

Well indices are Peaceman from the deck’s 0.5 ft diameter and each layer’s `k·dz`. `COMPDAT` item 10
sets `Kh = 10000 mD·ft` on every completion: layers 1 and 3 already have that (`500×20` and
`200×50`); the middle layer’s own `50×30 = 1500` is overridden. These wells sit in layers 1 and 3,
so the override does not change their index.

## What this case is and is not

**The published project compared four-component and fully compositional models as well as
black-oil + solvent representations.** ResSim runs a six-component Peng–Robinson model, which is
one of the published model classes. The OPM deck available locally implements the case as
**black-oil + solvent**; that deck is the grid, rock, relperm and schedule source, not the benchmark
itself. A published comparison should use the compositional participants' curves, not the OPM
deck's black-oil output.

The deliberate differences from the OPM deck implementation are:

- **Capillary pressure is zero.** The compositional model has no hydrocarbon capillary pressure;
  the deck’s water–oil column is small but not zero.
- **Surface separation is one equilibrium stage** at 1 atm / 288.71 K (C5’s compositional default),
  not the deck’s black-oil PVT table. The deck’s stock-tank rate targets are converted through it.
- **`SOF3` is interpreted by Stone II.** The scenario samples the union of the deck’s own knots and
  the transformed `SOF3` knots, so no stated breakpoint is dropped, and sets `Pc = 0`.
- **The producer is oil-rate controlled, not liquid-rate.** `SurfacePhase::Liquid` in the
  compositional engine is the stock-tank hydrocarbon liquid, which is the deck’s `ORAT`.

### Shut-in criteria: declared and evaluated (#98)

The paper specifies stopping at **GOR 10 Mscf/STB** or **WOR 5 STB/STB**. The scenario declares both
in `terminationPolicy`:

- `gor-threshold` with `relation: 'gte'` at **1781.0760667903526 m³/m³**, the conversion of
  10 Mscf/STB through `1000 × 0.028316846592 / 0.158987294928`;
- `watercut-threshold` at **5/6**, because WOR 5 STB/STB is the same surface condition as water cut
  `5 / (5 + 1)`.

The compositional session evaluates the policy after every report step from the snapshot's per-well
surface ledger. A matching report is always emitted, even when it falls between the requested
snapshot interval, and the batch ends through the normal `completed` path with the matched condition
in its summary rather than as a solver failure. The run record keeps the completed step count and
the termination summary, so the stopping report is part of the result, not only a UI warning. The
stop ends the whole batch rather than shutting in only the producer; SPE5 has one producer, so its
producer curves are unchanged, but injection and pressure history stop at that report.

**ResSim has not reached the stop on a monthly report; the independent reference has.** The
from-scratch ResSim replay reaches month 217 at **6604.938 d** and fails month 218 at
**6612.703 d** (`scaled residual 1.36e-7`, #116). The last peaks before the stall are month-207
GOR **924.0 m³/m³** (5.19 Mscf/STB) and month-208 water cut **0.830951** (WOR 4.91), both below
the thresholds. The frozen month-192 checkpoint with the scenario's monthly steps follows the same
path and fails at month 218 (`1.86e-8`). The existing 5-day-step workaround crosses the water cut
at **6304.25 d** (0.866442, WOR 6.49), but that is not a report step: the monthly reports around it
are 0.827546 at 6300.5625 d and 0.830951 at 6331 d, both below `5/6`.

The JutulDarcy reference crosses the same stop at month 207 (**6300.5625 d**) by water cut:
**0.854462** (WOR 5.13), GOR **1060.2 m³/m³**; its month-208 report is **0.872972**. ResSim
reports 0.827546 and 0.830951 at those two reports. The two simulators are therefore on opposite sides of
`5/6` at the same report, separated by 0.027 and 0.042 water cut — inside the late-window spread
recorded below (worst 0.0616). #98's threshold is physically reachable on this case; whether a
given monthly ResSim path fires it is a late-time trajectory question, not a case-definition
defect. The policy is still not demonstrated end-to-end on a ResSim monthly run, so #98 remains
open under #116.

The stop is scenario metadata, not part of `case.json`, so the native replays and the committed
JutulDarcy reference run without it. A comparison must either apply the same stop or truncate both
sides at the report where it fires.

Regression coverage:

- `src/lib/catalog/scenarios/comp_spe5_wag.test.ts` pins both authored thresholds;
- `src/lib/workers/terminationPolicy.test.ts` covers a matching and a non-matching GOR, the WOR/water-cut
  equivalence, a producer rate condition, zero-liquid vapour production, unreported surface rates
  and `mode: 'all'`;
- `src/lib/workers/compositionalSession.test.ts` covers a policy that stops mid-batch and one that
  does not, a match on the final requested step, and a restored session carrying the policy.

The full published-trajectory comparison — the published compositional participants' curves and
the shared scorecard — remains the work in the reference plan below; the JutulDarcy comparison
above is a cross-simulator trajectory measurement, not the published benchmark.

The reported "oil" and "gas" are the separator **liquid** and **vapour** volumes, not black-oil
`FOPT`/`FGPT` metered through a PVT table. At 1 bar the flash's single-phase label is pressure-blind,
so a CO2-rich surface stream can be labelled liquid; the chart and record keep the separator names
visible rather than relabelling it.

### Month-215 flash trial (#112): fixed

The from-scratch replay used to fail at **6528.84375 d** in the VapourLike stability trial at cell
32. That trial was sliding slowly toward the trivial solution rather than sitting at a fixed point:
`k_norm` fell from `1.6e-2` to `5.5e-3`, the trial sum drifted from `1 − 2.18e-9` to `1 − 2.01e-9`
while the stopped flag was true, and `sum (R_i − 1)^2` hovered between about `5e-14` and `2.2e-13`
without reaching the `1e-15` fixed-point tolerance. It hit the 100,000-iteration cap and returned
`TrialNotConverged`, even though the sum had long since decided the state was stable.

A **stable-side residual floor** now ends that path: once the sum has stopped more than
`TRIAL_SUM_TOLERANCE` below one, a squared residual below `1e-12` is enough for the verdict. Such a
trial needs no flash seed. Unstable-side trials keep the `1e-15` fixed-point tolerance because
their `K` seeds the flash, and any trial still inside the decision margin keeps iterating unchanged.
For the cell-32 feed the floor fires at iteration 15,511 with residual `9.999e-13`.

The default retry budget is also 10 attempts rather than 6: the same report needs an eighth attempt
to reach its feasible `0.238 d` step, and ten leaves a bounded margin. This is a ladder increase,
not an acceptance relaxation. The underlying inefficiency is that every `step()` call starts again
from the requested `dt`; remembering the last accepted timestep is a worthwhile follow-up.

The regression is `comp_stability_finishes_a_near_trivial_stable_trial` (the cell-32 feed returns
`Stable` instead of hitting the cap). The ignored native replay
`comp_spe5_from_scratch_clears_the_month_215_flash_stall` reaches month 217, proving the 215 region
is behind it:

```bash
cargo test --release --manifest-path src/lib/ressim/Cargo.toml --lib \
  comp_spe5_from_scratch_clears_the_month_215_flash_stall -- --ignored --nocapture
# commit a380a22d: SPE5 from scratch reached month 217 at t=6604.938 d in 489.7 s
# commit e1e07ed5 (#119): month 217 at t=6604.938 d in 527.6 s
# The 869.9 s figure recorded earlier was measured while the JutulDarcy reference and a second
# replay were running; it is dropped as a contaminated baseline.
```

The next stall is in month 218 and remains open under
[#116](https://github.com/sergeyfarin/ressim/issues/116). After #119 the same from-scratch
replay fails at **6612.703 d** with `Nonlinear: Newton did not converge in 20 iterations
(scaled residual 1.36e-7)`, so the fix moved the month-215 flash but not the near-critical
phase-labelling plateau.

### Month-215 cold flash (#119): fixed

The #112 fix removed the stability-trial cap, but the next cold flash at cell 32 still exhausted
the 5,000-substitution budget. The trace showed it was not still converging: the equilibrium
residual fell from `3.11e-9` at substitution 250 to `2.42e-9` at 1,000, then rose to `1.10e-8` at
5,000, while `sum (ln K)^2` moved only from `4.6029e-3` to `4.5984e-3`. Raising the cap would have
made it worse.

The Newton acceleration was not at fault. Its Jacobian matched central finite differences to a
worst relative error of `1.9e-6`, and the full step from `beta = 0.0013` moved to `0.156` — the
right direction, toward the true `beta = 0.1333` — but the full-system residual rose from `3.3e-9`
to `6.3e-7`. The monotone line search accepted only `alpha ~ 1/128`, reducing the residual by about
1% per 50-step attempt.

`NEWTON_NONMONOTONE_CEILING` now admits that one full step when the candidate `beta` is physical
and its residual is below `1e-5`. The following Newton steps converge quadratically; the final
admission target is unchanged at `1e-11`, and a failed attempt still falls back to substitution.
The cell-32 feed converges in 15 iterations to `TwoPhase`, `beta = 0.1333429`, residual `8.2e-14`;
the warm-started continuation from 228.52 bar reaches the same root. The committed seed now accepts
the `2.0409934222698212e-6 d` step at `6540.217532701092 d` in one attempt and 6 Newton iterations.

Regressions: `comp_flash_near_critical_full_step_converges` (the exact cold feed, under 100
iterations, in the thermodynamic gate) and `comp_spe5_month215_flash_seed_steps_through` (the
step, flipped from expecting the cap to expecting success; release-only).

### Late-life stall (#93): fixed

The replay used to stop at **6114.808 d** with `Budget` over `Nonlinear: Newton did not converge in
20 iterations (scaled residual 1.0414404786341731e-3)`. The trigger is a near-critical state in the
top-layer cells next to the water injector (cell 17 and its mirror cell 23). At 160 °F and
`z = (0.676258, 0.224125, 0.056110, 0.016736, 0.019748, 0.007023)` the two-phase split has
`beta = 0.0649` at 224.6557 bar, and the true bubble point is about 225.0 bar.

The Michelsen stability test inherited OPM's `sum W >= 1 + 1e-5` acceptance margin. At 224.6557 bar
the two-phase branch has lower Gibbs energy and the trial sum is only `1 + 9.2e-6`, inside that
margin, so the test called the feed stable. The flash then jumped from a two-phase split with
`beta = 0.0649` to a single liquid; the mixture molar volume dropped by 5.2e-4 relative and the
scaled residual changed sign across the jump. A monotone line search cannot cross a residual
discontinuity, and the full Newton iteration two-cycled between the branches. Cutting the timestep
only shrinks the jump until the whole transition is avoided; it does not make the step affordable.

`TRIAL_SUM_TOLERANCE` is now `1e-12`, and the trial iteration must itself stop changing the trial
sum to `1e-14` relative before the verdict is used. The squared K-update residual alone does not
bound the trial sum — the sum is stationary at the fixed point, so its error is second order in K
and judging it from the residual needs an unmeasured curvature factor — which is why the sum-step
check is explicit. The split now continues from `beta = 0.0649` to zero; the true switch is at
**225.0379687 bar**, where `beta = 1.1e-8` and the relative molar-volume jump is **8.9e-11**, below
the `1e-8` nonlinear acceptance tolerance. A warm seed from below the switch and a cold start agree
at 225.0376 bar, where the first version of this fix had cold single-phase and warm two-phase.

The switch regression is `comp_flash_keeps_a_near_critical_split_continuous` (it scans through the
true switch and bounds the volume jump), and the one-cell Newton regression is
`comp_newton_one_cell_near_critical_depletion_crosses_the_switch` (single → two-phase in three
iterations). OPM's scalar flash carries the same `1e-5` margin and switches earlier; the committed
OPM fixtures still agree after the change: 84 stability verdicts, 47 flash phase states and 140
trajectory states.

The month-192 → month-201 segment now clears in 56 calls and 117 s.

### Late-life flash acceleration (#99): fixed

The next region was a genuine successive-substitution stall, not the stability verdict. Around
6366–6375 d the flash's convergence ratio approached one: captured states needed 78, 173, 4,520
and more than 5,000 substitutions, and at 6366.59 d the flash failed at residual 4.3e-8. Even with
a 100,000 cap the 5-day step was then cut 32× to 0.156 d.

The flash now takes a damped Newton step on the full equilibrium system `[ln K, beta]` — the same
fugacity-equality plus Rachford–Rice rows that `fluid/derivatives.rs` differentiates — when a
substitution step retains at least 70% of its residual after ten iterations. The line search only
accepts a decrease in the full-system residual, and a singular Jacobian, an EOS-domain failure or a
failed line search returns to substitution, so acceleration cannot turn a convergent flash into a
failure. The recorded late-life states now converge in 14–20 iterations, and the fixture and C6
sweep answers are unchanged (`docs/COMPOSITIONAL_VALIDATION.md` §5).

The flash acceleration alone did not unblock the replay: the next step failed in the stability
test's `LiquidLike` trial, which needed **28,946** substitutions and was rejected by the inherited
OPM cap of 20,000. `MAX_TRIAL_ITERATIONS` is now 100,000, a correctness backstop on the same
argument. That trial is the same linearly convergent near-critical stall as the flash, one level
down. It now takes a damped Newton step on the tangent-plane stationarity system for the active
components — `ln K_i + ln phi_i^trial(W) - ln phi_i^feed(z) = 0` for the vapour-like trial, with
the trial and feed terms reversed for the liquid-like one — when a substitution step retains at
least 95% of its residual after ten iterations. The line search accepts only a decrease in the
full-system residual, and a singular Jacobian, an EOS-domain failure, a failed line search or an
iteration cap returns to substitution, so acceleration cannot turn a convergent trial into a
failure. The trial-sum and trivial-solution verdict criteria are unchanged.

The 6366.59 d state's `LiquidLike` trial now reaches its verdict in **60 substitutions** (65 for
`VapourLike`), down from 28,946, and the same acceleration clears the later **6646.0625 d** trial
at cell 33 that sat just above the trivial-solution threshold and needed more than 100,000
substitutions. No verdict changes: the 47-state stability fixture, the flash fixture and the C6
sweep are unchanged.

The 6375 d segment now advances without a timestep collapse: the month-192 → 6375 d replay takes
**108 calls, no retried step below 2.5 d (event-capped single-attempt steps are unaffected),
at most 10 Newton iterations, 193 s**. The full month-192 → month-240 replay completes in
**302 calls, worst 11 Newton iterations, 970 s**.

Both results start from the committed month-192 checkpoint, which was written in `849f03ad`,
before the #93/#99/#102 flash fixes. That file is intentionally frozen as the reproduction seed for
those regressions; it is not a checkpoint the current engine produces. A current from-scratch run
passes month 192 but diverges from this state and fails in month 218
([#116](https://github.com/sergeyfarin/ressim/issues/116)). The "completes" result therefore covers
the resume path from the frozen seed, not the full from-scratch horizon.

Evidence:

```bash
# The exact failing call, from the committed pre-fix state: one attempt, 5 Newton iterations.
cargo test --release --manifest-path src/lib/ressim/Cargo.toml \
  comp_spe5_resume_192_first_stalled_call_converges -- --ignored --nocapture

# The month-192 to month-201 segment that used to stall: 56 calls, 117 s.
cargo test --release --manifest-path src/lib/ressim/Cargo.toml \
  comp_spe5_resume_192_clears_the_201_report -- --ignored --nocapture

# The month-192 to 6375 d near-critical region: 108 calls, no retried step below 2.5 d,
# worst 10 Newton iterations, 193 s.
cargo test --release --manifest-path src/lib/ressim/Cargo.toml \
  comp_spe5_resume_192_clears_the_near_critical_region -- --ignored --nocapture

# The rest of the 20-year replay across the 6646.0625 d trial stall: 302 calls, 970 s.
cargo test --release --manifest-path src/lib/ressim/Cargo.toml \
  comp_spe5_resume_192_completes_the_20_year_replay -- --ignored --nocapture
```

The 16-year from-scratch replay (`comp_spe5_case_reaches_the_late_life_checkpoint`) also passes
with the #93 fix, in about 220 s. The unit regressions are part of the thermodynamic gate:
`comp_stability_detects_a_near_critical_tangent_plane` pins the verdict on both sides of the true
bubble point, `comp_flash_keeps_a_near_critical_split_continuous` pins the continuous split and the
no-jump molar volume across the old switch,
`comp_flash_accelerates_spe5_late_life_near_critical_states` pins the #99 states below 100
iterations, and `comp_stability_accelerates_a_near_critical_trial` pins the #102 state at fewer
than 100 substitutions for both trial phases.

## Independent JutulDarcy reference

`opm/compositional/spe5/reference.json` is a JutulDarcy 0.3.7 / Julia 1.12.7 compositional run of
the same `case.json` payload: the 7×7×3 grid, rock compressibility, six-component PRCORR EOS, OPM
Stone II relative permeability, rate targets and WAG schedule. It has **240 monthly reports**
(30.4375 d) through **7305 d**, with `completed: true`. The fixture carries the per-report producer
rates, cell-33 pressure and saturations, producer control/BHP, and Jutul's `ministeps`,
`newton_iterations`, `failed_ministeps` and final reservoir CNV/MB. `manifest.json` records the
generator, verify command and SHA-256 hashes; `worktree_clean_at_generation` is false because the
fixture was produced before the generator and this documentation were committed, from base commit
`e10bddc6`. Regenerate or verify with:

```bash
bash tools/opm_compositional/run-spe5.sh           # regenerate, about 80 min
bash tools/opm_compositional/run-spe5.sh --check   # replay and compare parsed numbers
```

Four changes were needed before the run was comparable to ResSim:

- **OPM Stone II.** JutulDarcy 0.3.7's `StoneIIMethod` evaluates the water-oil and gas-oil oil
  curves at the three-phase oil saturation. OPM's `EclStone2Material`, and ResSim's
  `k_ro_stone2_generic`, use `krow(Sw)` and `krog(1 - Swc - Sg)`. `OPMStoneII` in the generator
  implements the OPM dispatch; the upper clamp follows ResSim's `StoneTwoRelPerm`.
- **Rock compressibility.** The first run omitted the case's `5e-6 psi⁻¹` rock compressibility.
  `JutulDarcy.set_rock_compressibility!` has to be applied to the `MultiModel` and the parameter
  container rebuilt with `setup_parameters`, because Jutul moves the static pore volume into a
  secondary `FluidVolume`.
- **Producer surface rates.** JutulDarcy 0.3.7 reports `wrat`/`wcut` from the aqueous reservoir
  volume fraction; gas expansion inflates the water volume by up to ~80× at the SPE5 producer. The
  fixture rebuilds the surface rates from the conserved component mass rates: water from
  `Water_mass_rate / rhoWS`, hydrocarbons from the remaining mass split by the
  `SurfaceWellConditions` flash. `raw_jutul_*` keeps the uncorrected values for audit.
- **Declared stop.** The run records the first report that fires the scenario's GOR/water-cut
  policy but continues to 7305 d, because #116 needs both sides of month 218. Only the first firing
  report is stored in `stopped`; a comparison truncates there.

Jutul's convergence tolerances are its defaults (CNV `1e-3`, MB `1e-7`), not ResSim's per-cell
mole-balance target; the fixture records them as such.

### Comparison with ResSim

The from-scratch ResSim probe has 217 monthly reports before the month-218 stall, so the comparison
covers months 1–217. At **730.5 d** (month 24) the two runs agree on pressure and the rate split:

| Quantity | ResSim | JutulDarcy |
|---|---:|---:|
| Mean pressure, bar | 112.68 | 112.58 |
| Cell-33 pressure, bar | 109.84 | 109.75 |
| Cell-33 Sw | 0.2032 | 0.2032 |
| Cell-33 Sg | 0.4091 | 0.4102 |
| Producer oil, m³/d | 959.2 | 954.3 |
| Producer gas, m³/d | 212,697 | 212,390 |
| Producer water, m³/d | 0.13698 | 0.13679 |
| GOR, m³/m³ | 221.7 | 222.6 |
| Water cut | 1.428e-4 | 1.433e-4 |

Both producers switch from oil-rate to BHP control at month 16 (**487.0 d**), the same report.
Across all 217 months the pressure agreement stays tight: the worst mean-pressure difference is
**1.76 bar** (month 175) and the worst cell-33 difference is **2.16 bar** (month 177). The rate and
saturation spread is wider in the late water/gas years: worst producer-oil difference **28.4%**
(month 208), worst GOR difference **22.6%** (month 164) and worst water-cut difference **0.0616**
absolute (month 169). Those are late-time trajectory spreads, not a normalization or unit error —
the early and mid-life rates agree within a few percent.

| month | day | mean p R / J (bar) | cell 33 p R / J (bar) | Sw 33 R / J | Sg 33 R / J | oil R / J (m³/d) | GOR R / J (m³/m³) | WC R / J | Jutul ministeps / Newton |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 12 | 365.25 | 139.04 / 138.94 | 136.23 / 136.11 | 0.2030 / 0.2030 | 0.1093 / 0.1101 | 1907.8 / 1916.8 | 87.5 / 87.5 | 0.00006 / 0.00006 | 1 / 5 |
| 16 | 487.00 | 129.07 / 128.94 | 125.37 / 125.23 | 0.2032 / 0.2032 | 0.2069 / 0.2083 | 1864.1 / 1854.8 | 112.2 / 112.3 | 0.00009 / 0.00009 | 1 / 5 |
| 24 | 730.50 | 112.68 / 112.58 | 109.84 / 109.75 | 0.2032 / 0.2032 | 0.4091 / 0.4102 | 959.2 / 954.3 | 221.7 / 222.6 | 0.00014 / 0.00014 | 1 / 4 |
| 48 | 1461.00 | 110.05 / 110.25 | 106.32 / 106.66 | 0.2025 / 0.2025 | 0.4706 / 0.4788 | 847.2 / 882.3 | 252.4 / 240.1 | 0.00016 / 0.00016 | 300 / 3279 |
| 96 | 2922.00 | 112.58 / 112.64 | 106.36 / 106.82 | 0.2015 / 0.2015 | 0.5094 / 0.5244 | 872.8 / 887.3 | 276.8 / 279.3 | 0.00016 / 0.00016 | 299 / 3263 |
| 144 | 4383.00 | 119.55 / 119.96 | 109.10 / 110.29 | 0.3836 / 0.3819 | 0.3007 / 0.3037 | 860.1 / 894.5 | 291.8 / 286.4 | 0.00016 / 0.00015 | 280 / 3050 |
| 192 | 5844.00 | 189.75 / 190.26 | 181.17 / 181.69 | 0.5353 / 0.5601 | 0.0732 / 0.0244 | 348.8 / 333.2 | 283.4 / 287.6 | 0.58980 / 0.60889 | 190 / 2065 |
| 204 | 6209.25 | 231.81 / 232.06 | 215.99 / 216.31 | 0.4869 / 0.4835 | 0.1680 / 0.1740 | 290.7 / 287.2 | 649.4 / 649.1 | 0.68825 / 0.69197 | 1 / 6 |
| 205 | 6239.69 | 229.93 / 229.46 | 215.43 / 215.25 | 0.5032 / 0.4970 | 0.1686 / 0.1845 | 235.4 / 241.3 | 745.1 / 774.4 | 0.73048 / 0.72410 | 136 / 1427 |
| 206 | 6270.12 | 229.62 / 228.78 | 215.46 / 214.31 | 0.5205 / 0.5174 | 0.1557 / 0.1725 | 178.7 / 155.8 | 847.6 / 1008.1 | 0.78231 / 0.79946 | 151 / 1618 |
| 207 | 6300.56 | 229.99 / 229.19 | 216.49 / 215.55 | 0.5346 / 0.5315 | 0.1336 / 0.1447 | 136.8 / 110.8 | 924.0 / 1060.2 | 0.82755 / 0.85446 | 158 / 1699 |
| 208 | 6331.00 | 230.58 / 230.38 | 218.70 / 217.67 | 0.5350 / 0.5424 | 0.1185 / 0.1135 | 138.4 / 99.0 | 854.3 / 1015.2 | 0.83095 / 0.87297 | 160 / 1721 |
| 209 | 6361.44 | 231.23 / 231.37 | 220.42 / 220.35 | 0.5389 / 0.5437 | 0.1064 / 0.0938 | 142.4 / 134.5 | 829.3 / 797.7 | 0.83061 / 0.84179 | 162 / 1754 |
| 210 | 6391.88 | 231.77 / 232.38 | 222.02 / 221.94 | 0.5461 / 0.5589 | 0.0861 / 0.0766 | 149.6 / 131.6 | 809.7 / 816.3 | 0.82640 / 0.84582 | 166 / 1801 |
| 211 | 6422.31 | 232.02 / 233.08 | 223.06 / 223.62 | 0.5542 / 0.5823 | 0.0668 / 0.0350 | 159.9 / 157.1 | 801.7 / 720.4 | 0.81816 / 0.82629 | 167 / 1815 |
| 212 | 6452.75 | 232.06 / 233.37 | 223.79 / 224.93 | 0.5570 / 0.6012 | 0.0563 / 0.0000 | 166.9 / 169.2 | 818.9 / 738.2 | 0.81176 / 0.81591 | 160 / 1735 |
| 213 | 6483.19 | 231.98 / 233.52 | 224.40 / 225.98 | 0.5481 / 0.5968 | 0.0665 / 0.0000 | 167.1 / 166.1 | 861.9 / 785.3 | 0.81118 / 0.81820 | 167 / 1814 |
| 214 | 6513.62 | 231.81 / 233.43 | 224.99 / 226.55 | 0.5259 / 0.5839 | 0.1036 / 0.0000 | 164.2 / 170.8 | 913.2 / 827.9 | 0.81323 / 0.81277 | 168 / 1830 |
| 215 | 6544.06 | 231.61 / 233.09 | 225.23 / 226.69 | 0.5003 / 0.5686 | 0.1664 / 0.0000 | 158.7 / 174.9 | 984.5 / 889.4 | 0.81709 / 0.80716 | 167 / 1815 |
| 216 | 6574.50 | 231.30 / 232.57 | 225.16 / 226.51 | 0.4785 / 0.5485 | 0.2168 / 0.0696 | 152.0 / 161.7 | 1070.6 / 1024.1 | 0.82167 / 0.81540 | 204 / 2228 |
| 217 | 6604.94 | 233.87 / 234.98 | 227.43 / 228.08 | 0.4513 / 0.5234 | 0.2515 / 0.0721 | 183.8 / 212.9 | 1013.4 / 917.9 | 0.79315 / 0.77195 | 276 / 2899 |
| 218 | 6635.38 | — | 229.75 | — | 0.0000 | — | 984.4 | — | 369 / 4006 |
| 219 | 6665.81 | — | 230.56 | — | 0.4334 | — | 1135.3 | — | 975 / 10827 |

### What it says about #116

The cell-33 phase-appearance window is the discriminator. Jutul shows no free gas at months
212–215, then Sg **0.0696** and **0.0721** at months 216–217, back to zero at month 218, and
**0.4334** at month 219. Crossing it is expensive but possible: month 217 costs 276 ministeps /
2,899 Newton iterations, month 218 costs 369 / 4,006, and month 219 costs 975 / 10,827 with 610
failed ministeps. The schedule then returns to two ministeps / ten Newton iterations at month 220
and one / five at month 221, before the next gas cycle builds again. Jutul's gas periods are
intrinsically slow as well — month 48 is 300 ministeps / 3,279 Newton iterations and month 96 is
299 / 3,263 — so the month-218 storm is an amplification of the gas-cycle cost, not a one-off
numerical failure.

That supports the #116 reading: the ResSim plateau is a solver-robustness problem at a crossing the
same discrete case can traverse, not an uncrossable physical discontinuity. It does not supply a
ResSim target trajectory. Jutul's late saturation state differs materially from ResSim's: at month
217 Jutul has cell-33 Sw/Sg 0.5234/0.0721 where ResSim has 0.4513/0.2515, and Jutul returns to
single liquid at month 218 while ResSim's stalled state keeps free gas. The two runs agree on
pressure and the broad water-cut ramp, not on the exact phase split in the last window.

Two caveats for later use of the fixture. The run emitted 234 `Flash did not converge in 25000
iterations` warnings from MultiComponentFlash; the worst final ε was **5.54e-8** against the
`1e-8` tolerance, and the accepted reservoir CNV stayed below Jutul's `1e-3` (maximum **2.33e-4**,
at month 217). The reference is therefore a default-tolerance JutulDarcy result, not a
thermodynamic oracle; the per-report CNV/MB in the fixture lets a tighter comparison reject the
affected reports instead of treating every number as exact.

## Reference plan

1. **Done.** The JutulDarcy reference is committed at `opm/compositional/spe5/reference.json` with
  `manifest.json` and `run-spe5.sh`; see “Independent JutulDarcy reference” above. The earlier
  156-report run is superseded: it ran before the OPM Stone II, rock-compressibility and
  surface-rate corrections, its producer was BHP-controlled from 730.5 d at an oil rate of
  2.9 m³/day, and its water cut at 4017.75 d was 0.49 against ResSim's 0.0002. The committed run
  completes 240 reports through 7305 d and records per-report Newton/timestep diagnostics.
2. Put the run through `compare_small_direct.py --comp-reference-dir` and the shared scorecard,
  including water saturation (the existing compositional comparator compares `p`, `sg` and `z`; it
  needs the `sw` column the SPE5 fixture carries).
3. State the oil-rate, GOR and average-pressure bands against JutulDarcy and the published
  **compositional** participants' curves. Digitize those curves under the SPE1 precedent
  (`publishedReferenceSeries`); do not hand-copy a number from prose.
4. Only then move `comp_spe5_wag` out of `WITHHELD_SCENARIOS`.

## Reproducer and gates

```bash
# Payload, EOS gradient and closed-well equilibrium; part of the compositional gate.
cargo test --manifest-path src/lib/ressim/Cargo.toml comp_spe5_ -- --nocapture

# First WAG cycle (production, water year, gas year); release and --ignored.
cargo test --release --manifest-path src/lib/ressim/Cargo.toml \
  comp_spe5_case_accepts_its_first_water_and_gas_slugs -- --ignored --nocapture

# Everything up to the month-192 checkpoint; release and --ignored because it is minutes of work.
cargo test --release --manifest-path src/lib/ressim/Cargo.toml \
  comp_spe5_case_reaches_the_late_life_checkpoint -- --ignored --nocapture

# #93: the exact failing call from the committed pre-fix state; seconds.
cargo test --release --manifest-path src/lib/ressim/Cargo.toml \
  comp_spe5_resume_192_first_stalled_call_converges -- --ignored --nocapture

# #93: the month-192 to month-201 segment from the pre-fix seed; release and --ignored.
cargo test --release --manifest-path src/lib/ressim/Cargo.toml \
  comp_spe5_resume_192_clears_the_201_report -- --ignored --nocapture
```

The failing call now completes in one timestep attempt and 5 Newton iterations. The pre-fix failure
was not a linear-solve or timestep-size defect: smaller timesteps and sparse LU reproduced it,
replacing the producer control with its BHP limit reproduced it, and a monotone line search stalled
at the residual jump. The 50-iteration experiment recorded in #93 was inconclusive and remains so;
the fix does not rely on it.

No black-oil or compositional tolerance is changed by this case. The stability margin change is a
thermodynamic correction, not a case-specific tolerance: it is covered by the two unit regressions
above and by the full `comp_stability_`/`comp_flash_` gate.
