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
**0.854810** (WOR 5.89), GOR **1066.1 m³/m³**; its month-208 report is **0.873085**. ResSim
reports 0.827546 and 0.830951 at those two reports. The two simulators are therefore on opposite sides of
`5/6` at the same report, separated by 0.027 and 0.042 water cut — inside the late-window spread
recorded below (worst 0.0615). #98's threshold is physically reachable on this case; whether a
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

### Non-monotone Newton answers (#122): guarded

The #119 full step relaxes the path, not the physics: its first candidate at `beta = 0.157` is
`+4.8e-10` above the single-phase feed in dimensionless Gibbs energy `g/(R T)`, even though the
following Newton iterations reach the lower answer. A converged non-monotone Newton replacement is
now admitted only when its two-phase Gibbs energy does not exceed the feed's; otherwise the attempt
returns to substitution. The feed is the lower of the liquid and vapour single-phase branches, and
a `1e-10` roundoff margin separates equality from a real metastable split. Both sides are evaluated
on normalized compositions: the EOS accepts a feed sum within `1e-9`, and using its raw entries
would move the comparison by more than the margin. The check applies only to the Newton
replacement — the substitution fixed point is still admitted by its residual alone.

Regressions: `comp_flash_gibbs_guard_rejects_the_near_critical_nonmonotone_split` pins the #119
candidate as uphill and the converged answer as downhill;
`comp_flash_gibbs_guard_normalizes_a_slightly_non_unit_feed` scales that feed by `1 + 9e-10` and
requires the same convergent two-phase answer. The #119 seed and the C0 flash fixture are
unchanged.

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
generator, verify command and SHA-256 hashes. The current fixture was regenerated on 2026-11-08 with
JutulDarcy's default well-node volume (#116 step 0, "Well-node volume" below). It was generated
from base commit `2ef5027a` with that generator change still uncommitted, so
`worktree_clean_at_generation` is false. The well-node volume is a model choice, not only a
performance setting (see "Well-node volume" under step 0): ResSim's wells have no wellbore storage,
and JutulDarcy's default nodes add 0.192 m³ per injector and 0.479 m³ for the producer. The
default-volume fixture is therefore **provisional**. The bounded volume/timestep sensitivity
("Volume/timestep sensitivity" under step 0) does not justify promoting it. At a 0.5 d timestep cap
the two node volumes agree through month 217 to 0.07 bar and 0.9% in oil. But the report-step
reference is not converged in time: it is 1.5 bar and 24% in oil from the 0.5 d run. The earlier
fixture (`b14f0976`, 1e-4 m³ well nodes) is kept only in that comparison.

`manifest.json` records the resolved model configuration (`configuration`: per-well node volumes,
timestep cap, versions, tolerances). `--check` fails if the configuration differs from it. On a run
that starts and ends on the same clean commit, `--check` also records that commit and the source
hashes in `verification`. On a dirty tree it verifies but records nothing. Regenerate or verify
with:

```bash
bash tools/opm_compositional/run-spe5.sh           # regenerate, about 30 min
bash tools/opm_compositional/run-spe5.sh --check   # replay; compare numbers and configuration
```

Five changes were needed before the run was comparable to ResSim:

- **Surface conditions.** JutulDarcy's default surface temperature is 288.15 K; ResSim pins
  101325 Pa / 288.71 K. Every well now carries that condition explicitly, so the producer surface
  flash, the oil-rate control and the injection densities use the same convention. The fixture
  records it as `surface_conditions`.
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
mole-balance target; the fixture records them as such. The generator runs with
`error_on_incomplete = true`, and the wrapper validates `completed`, the 240 reports and the
7305 d horizon before it replaces either `reference.json` or `manifest.json`.

### Comparison with ResSim

The from-scratch ResSim probe has 217 monthly reports before the month-218 stall, so the comparison
covers months 1–217. Every number in this section is printed by
`python3 -I tools/jutul/spe5_compare.py RESSIM_MONTHS` (its docstring has the command that dumps
the ResSim months); none is typed by hand. At **730.5 d** (month 24) the two runs agree on
pressure and the rate split:

| Quantity | ResSim | JutulDarcy |
|---|---:|---:|
| Mean pressure, bar | 112.68 | 112.58 |
| Cell-33 pressure, bar | 109.84 | 109.75 |
| Cell-33 Sw | 0.2032 | 0.2032 |
| Cell-33 Sg | 0.4091 | 0.4102 |
| Producer oil, m³/d | 959.2 | 954.2 |
| Producer gas, m³/d | 212,697 | 212,885 |
| Producer water, m³/d | 0.13698 | 0.13679 |
| GOR, m³/m³ | 221.7 | 223.1 |
| Water cut | 1.428e-04 | 1.433e-04 |

Both producers switch from oil-rate to BHP control at month 16 (**487.0 d**), the same report.
Across all 217 months the worst mean-pressure difference is **0.86 bar** and the worst cell-33
difference **0.76 bar**, both at the edge of the crossing (months 217 and 216). The worst
producer-oil difference is **11.7%** (month 208), the worst GOR difference **10.5%** (month 217) and
the worst water-cut difference **0.0177** absolute (month 208), all relative to the larger of the
two. The worst cell-33 Sg difference is **0.096** (month 217).

The 1e-4 m³ reference had shown roughly twice these spreads: 1.75 bar, 28.5%, 22.3% and 0.0615.
So the late-life ResSim/Jutul gap depends on JutulDarcy's well-node volume. Closer agreement does
not show that the default volume is the more accurate model: ResSim has no wellbore storage, and
the 1e-4 m³ nodes were chosen elsewhere precisely to approximate that (see "Well-node volume").
The sensitivity later showed the gap depends even more on the timestep: coarse-step ResSim
differs by 33% in oil from the 0.5 d-capped Jutul runs. ResSim run at the same caps (see
"ResSim's timestep convergence") agrees with Jutul to 1.0% in oil at 5 d and at 0.5 d, and its own
report-step run is 33% from its 0.5 d run, as Jutul's is 24% from its own. So that 33% is
time-discretization error both simulators share, not a ResSim-specific error. The agreement above
is between two simulators taking report-sized steps, not a measure of either one's accuracy.

| month | day | mean p R / J (bar) | cell 33 p R / J (bar) | Sw 33 R / J | Sg 33 R / J | oil R / J (m³/d) | GOR R / J (m³/m³) | WC R / J | Jutul ministeps / Newton |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 12 | 365.25 | 139.04 / 138.94 | 136.23 / 136.12 | 0.2030 / 0.2030 | 0.1093 / 0.1101 | 1907.8 / 1916.9 | 87.5 / 87.7 | 0.00006 / 0.00006 | 1 / 4 |
| 16 | 487.00 | 129.07 / 128.94 | 125.37 / 125.23 | 0.2032 / 0.2032 | 0.2069 / 0.2083 | 1864.1 / 1854.9 | 112.2 / 112.5 | 0.00009 / 0.00009 | 1 / 4 |
| 24 | 730.50 | 112.68 / 112.58 | 109.84 / 109.75 | 0.2032 / 0.2032 | 0.4091 / 0.4102 | 959.2 / 954.2 | 221.7 / 223.1 | 0.00014 / 0.00014 | 1 / 4 |
| 48 | 1461.00 | 110.05 / 110.01 | 106.32 / 106.30 | 0.2025 / 0.2025 | 0.4706 / 0.4702 | 847.2 / 849.2 | 252.4 / 250.6 | 0.00016 / 0.00016 | 1 / 4 |
| 96 | 2922.00 | 112.58 / 112.59 | 106.36 / 106.46 | 0.2015 / 0.2015 | 0.5094 / 0.5107 | 872.8 / 882.0 | 276.8 / 273.0 | 0.00016 / 0.00016 | 1 / 4 |
| 144 | 4383.00 | 119.55 / 119.65 | 109.10 / 109.43 | 0.3836 / 0.3841 | 0.3007 / 0.3011 | 860.1 / 873.6 | 291.8 / 287.5 | 0.00016 / 0.00015 | 1 / 4 |
| 192 | 5844.00 | 189.75 / 189.90 | 181.17 / 181.20 | 0.5353 / 0.5450 | 0.0732 / 0.0580 | 348.8 / 344.8 | 283.4 / 283.6 | 0.58980 / 0.59440 | 1 / 5 |
| 204 | 6209.25 | 231.81 / 232.03 | 215.99 / 216.27 | 0.4869 / 0.4869 | 0.1680 / 0.1665 | 290.7 / 293.3 | 649.4 / 637.9 | 0.68825 / 0.68751 | 1 / 5 |
| 205 | 6239.69 | 229.93 / 229.64 | 215.43 / 215.46 | 0.5032 / 0.5018 | 0.1686 / 0.1739 | 235.4 / 240.2 | 745.1 / 751.1 | 0.73048 / 0.72604 | 6 / 29 |
| 206 | 6270.12 | 229.62 / 229.23 | 215.46 / 215.06 | 0.5205 / 0.5200 | 0.1557 / 0.1620 | 178.7 / 169.9 | 847.6 / 896.5 | 0.78231 / 0.78902 | 2 / 9 |
| 207 | 6300.56 | 229.99 / 229.71 | 216.49 / 216.10 | 0.5346 / 0.5349 | 0.1336 / 0.1375 | 136.8 / 125.5 | 924.0 / 973.1 | 0.82755 / 0.83903 | 17 / 171 |
| 208 | 6331.00 | 230.58 / 230.65 | 218.70 / 218.49 | 0.5350 / 0.5399 | 0.1185 / 0.1128 | 138.4 / 122.1 | 854.3 / 890.6 | 0.83095 / 0.84867 | 5 / 31 |
| 209 | 6361.44 | 231.23 / 231.45 | 220.42 / 220.49 | 0.5389 / 0.5421 | 0.1064 / 0.1010 | 142.4 / 138.1 | 829.3 / 815.2 | 0.83061 / 0.83665 | 14 / 130 |
| 210 | 6391.88 | 231.77 / 232.30 | 222.02 / 222.28 | 0.5461 / 0.5542 | 0.0861 / 0.0775 | 149.6 / 142.9 | 809.7 / 792.5 | 0.82640 / 0.83483 | 30 / 301 |
| 211 | 6422.31 | 232.02 / 232.69 | 223.06 / 223.53 | 0.5542 / 0.5715 | 0.0668 / 0.0474 | 159.9 / 160.9 | 801.7 / 755.4 | 0.81816 / 0.82048 | 2 / 8 |
| 212 | 6452.75 | 232.06 / 232.74 | 223.79 / 224.30 | 0.5570 / 0.5800 | 0.0563 / 0.0194 | 166.9 / 168.9 | 818.9 / 778.9 | 0.81176 / 0.81298 | 1 / 4 |
| 213 | 6483.19 | 231.98 / 232.59 | 224.40 / 224.90 | 0.5481 / 0.5780 | 0.0665 / 0.0117 | 167.1 / 172.2 | 861.9 / 820.7 | 0.81118 / 0.80875 | 1 / 4 |
| 214 | 6513.62 | 231.81 / 232.37 | 224.99 / 225.43 | 0.5259 / 0.5600 | 0.1036 / 0.0456 | 164.2 / 168.5 | 913.2 / 883.5 | 0.81323 / 0.81091 | 1 / 4 |
| 215 | 6544.06 | 231.61 / 232.14 | 225.23 / 225.93 | 0.5003 / 0.5299 | 0.1664 / 0.1022 | 158.7 / 162.0 | 984.5 / 953.3 | 0.81709 / 0.81562 | 9 / 81 |
| 216 | 6574.50 | 231.30 / 231.95 | 225.16 / 225.92 | 0.4785 / 0.5004 | 0.2168 / 0.1711 | 152.0 / 143.3 | 1070.6 / 1088.3 | 0.82167 / 0.83102 | 123 / 1353 |
| 217 | 6604.94 | 233.87 / 234.73 | 227.43 / 228.01 | 0.4513 / 0.4731 | 0.2515 / 0.1554 | 183.8 / 198.5 | 1013.4 / 906.6 | 0.79315 / 0.78439 | 388 / 4245 |
| 218 | 6635.38 | — / 236.13 | — / 229.41 | — / 0.4280 | — / 0.0000 | — / 200.1 | — / 1050.6 | — / 0.77838 | 1899 / 20828 |
| 219 | 6665.81 | — / 237.37 | — / 229.97 | — / 0.3869 | — / 0.4827 | — / 232.5 | — / 1049.4 | — / 0.75180 | 1010 / 11038 |

### What it says about #116

The cell-33 phase-appearance window is where the two runs separate. With JutulDarcy's default well
node,
Jutul keeps free gas in cell 33 through months 212–215, but less of it than ResSim: Sg is
0.019 / 0.012 / 0.046 / 0.102 against ResSim's 0.056 / 0.067 / 0.104 / 0.166. Then it has 0.171 and
0.155 at months 216–217, **0.000** at month 218, and 0.483 at month 219.

The 1e-4 m³ reference had shown no free gas at all in months 212–215. The earlier reading that
"Jutul shows no free gas at months 212–215" depended on that well-node volume, and it is withdrawn
as a discriminator.

Jutul's cell-33 label at the month-218 report is single-phase, between two-phase reports on either
side (Sg is zero again at month 222). That alone does not show a numerically unstable label:
pressure and composition change between reports (cell-33 pressure rises 1.4 bar from month 217 to
218 and Sw falls by 0.045), so a single-phase state at one report can be a physical state. It shows
only that the reference's monthly label in this window is not monotone.

Crossing the window is expensive but possible:

| Month | Ministeps | Newton iterations | Failed ministeps |
|---|---|---|---|
| 216 | 123 | 1,353 | 76 |
| 217 | 388 | 4,245 | 237 |
| 218 | 1,899 | 20,828 | 1,164 |
| 219 | 1,010 | 11,038 | 616 |

Quiet months take 1–2 ministeps (month 220: 2 / 8), and the gas years are cheap (month 48: 1 / 4).
The month-218 storm is specific to the crossing. The 1e-4 m³ reference's slow gas periods came
from the gas injector's well-node convergence with that node volume, not from the reservoir.

That supports the #116 reading: the ResSim plateau is a solver-robustness problem at a crossing the
same discrete case can traverse at JutulDarcy's default tolerances, not an uncrossable physical
discontinuity. It does not supply a
ResSim target trajectory. The two runs agree on pressure and the water-cut ramp, not on the exact
phase split in this window, and the reference's split in this window moved with a well-node
setting.

Two caveats for later use of the fixture:
- The run emitted 189 `Flash did not converge in 25000 iterations` warnings from
  MultiComponentFlash. The worst final ε was **6.19e-8** against the `1e-8` tolerance.
- The accepted final reservoir CNV stayed below Jutul's `1e-3`, but only just: the maximum is
  **9.34e-4**, at month 215.

The reference is therefore a default-tolerance JutulDarcy result, not a thermodynamic oracle. The
per-report CNV/MB in the fixture lets a tighter comparison reject the affected reports instead of
treating every number as exact.

### Step 0: measurements before any acceptance change (#116)

**Provisional.** Every number in this section was measured on a dirty tree on top of `2ef5027a`
(the instruments below, uncommitted at the time). No solver behaviour changed: the Newton observer
hook is behaviour-neutral and every new test entry point is `#[cfg(test)]`. The numbers become a
baseline only after a rerun on the committed tree.

The proposed acceptance rule (registry `COMP-ACCEPT-001`) is **not** approved and not implemented.
Step 0 measured the actual iterates it would act on, in both ResSim's and JutulDarcy's
normalizations, with a signed and an absolute conservation account per iterate.

#### Instruments and commands

| What | Where | Command |
|---|---|---|
| Per-Newton-iterate record (residual per row, Jutul CNV/MB, signed global imbalance, absolute local defect, well rows, flipped cells, flash detail of every flipped or above-tolerance cell) | `compositional/spe5_plateau_tests.rs` via `solve_newton_observed` | see below |
| Seed plateau trace (`stall_218.json`, dt 0.02 d then 1.95e-5 → 2.44e-6 d) | `comp_spe5_month218_plateau_trace` | `SPE5_TRACE_OUT=DIR cargo test --release --lib comp_spe5_month218_plateau_trace -- --ignored` (seconds) |
| From-scratch replay to the month-218 failure, then every halving attempt traced; writes the failing call's starting state | `comp_spe5_from_scratch_month218_failure_trace` | `SPE5_TRACE_OUT=DIR cargo test --release --lib comp_spe5_from_scratch_month218_failure_trace -- --ignored` (~16 min) |
| Failure B from its committed seed (`stall_218_from_scratch.json`): the failing call and its last attempt traced, signature asserted | `comp_spe5_from_scratch_218_seed_reproduces_signature_b` | `SPE5_TRACE_OUT=DIR cargo test --release --lib comp_spe5_from_scratch_218_seed_reproduces_signature_b -- --ignored --nocapture` (~1 min) |
| Jutul ministep diagnostics, months 205–220: dt, success, per-Newton reservoir CNV/MB per component, every well and facility criterion, and the resolved configuration | `tools/jutul/spe5_ministep_diagnostics.jl` | `julia +1.12 --project=tools/jutul tools/jutul/spe5_ministep_diagnostics.jl OUT.json 205 220` (~30 min with the default well volume); table: `python3 -I tools/jutul/spe5_compare.py --ministeps OUT.json` |
| Cross-flash fixture, months 205–216, cells 32/33/39/40 | `opm/compositional/spe5/cross_flash_205_216.json` | `bash tools/jutul/run-spe5-cross-flash.sh [--check] [WORKDIR]` (~30 min). The merge refuses to write unless all 192 points have valid results from both flashes; `spe5_cross_flash_fixture.py check FIXTURE` re-validates a written one. `comp_spe5_cross_flash_fixture_reproduces_the_ressim_column` (in the thermo gate) fails if the ResSim flash moves it |
| JutulDarcy well-volume/timestep sensitivity | `tools/jutul/run-spe5-sensitivity.sh` | `bash tools/jutul/run-spe5-sensitivity.sh OUTDIR [JOBS]` (~80 min at 4 jobs) |
| Near-critical flash states of failure B's best iterate (cells 33/39/40, 64 pressure ulps, full splits and the fluid) | `comp_spe5_failure_b_flash_conditioning` | `SPE5_TRACE_OUT=DIR cargo test --release --lib comp_spe5_failure_b_flash_conditioning -- --ignored` (seconds) |
| The same states solved in 50 digits, and by MultiComponentFlash | `tools/jutul/spe5_flash_mp.py`, `tools/jutul/spe5_flash_precision.jl` | `uv run --no-project --with mpmath python -I tools/jutul/spe5_flash_mp.py COND.json MP.json [STRIDE]`; `julia +1.12 --project=tools/jutul tools/jutul/spe5_flash_precision.jl COND.json OUT.json MP.json` |
| Flash refinement on the captured states (cells 33/39/40; pressure-ulp, 0.01 Pa and composition sweeps; both modes) | `comp_spe5_flash_refinement_states` | `SPE5_TRACE_OUT=DIR cargo test --release --lib comp_spe5_flash_refinement_states -- --ignored` (seconds); 50 digits: `spe5_flash_mp.py STATES.json MP.json 1` (~2.5 min); table: `spe5_flash_refinement_compare.py STATES.json MP.json` |
| Failure B with the refinement: last attempt, Taylor test, dt ladder, the failing call | `comp_spe5_failure_b_with_refinement` | `SPE5_TRACE_OUT=DIR cargo test --release --lib comp_spe5_failure_b_with_refinement -- --ignored --nocapture` (~3 min) |
| Replays with a halving budget, a dt cap, timestep memory or the flash refinement, per-month work and balance; optionally the failing call's starting state | `comp_spe5_replay_with_timestep_options` | `SPE5_REPLAY_OUT=OUT.json [SPE5_REPLAY_SEED=stall_218_from_scratch.json] [SPE5_MAX_ATTEMPTS=N] [SPE5_MAX_DT_DAYS=D] [SPE5_DT_MEMORY_GROWTH=G] [SPE5_FLASH_REFINE=1] [SPE5_REPLAY_CHECKPOINT_OUT=SEED.json] cargo test --release --lib comp_spe5_replay_with_timestep_options -- --ignored --nocapture` (11 min to 2.5 h); table: `spe5_compare.py --timestep-convergence` |
| One Newton solve per halving from any captured state, with per-iterate flips and residuals | `comp_spe5_trace_seed_ladder` | `SPE5_TRACE_OUT=DIR SPE5_TRACE_SEED=SEED.json [SPE5_TRACE_DT_DAYS=D] [SPE5_TRACE_HALVINGS=N] [SPE5_FLASH_REFINE=1] cargo test --release --lib comp_spe5_trace_seed_ladder -- --ignored --nocapture` (0.5–1.5 min) |

Signed global imbalance is `Σ_cells R_c` per component (dt-integrated, moles) relative to that
component's inventory; absolute local defect is `Σ_cells |R_c|`. They are recorded separately so
cancellation between cells cannot hide a local error, and neither is mixed with well injection or
production.

Jutul CNV/MB are recomputed from ResSim's dt-integrated residual `R` (moles) following JutulDarcy
0.3.7's `compositional_criterion`:
- **Hydrocarbon CNV:** `CNV_c = |R_c| M_c (1 − Sw)² / m_hc,cell`. Jutul scales a component's
  residual by `dt (1 − Sw) / (V ρ)`, where `ρ` is the hydrocarbon density at the saturations
  renormalized by `1 − Sw`, so `V ρ = m_hc,cell / (1 − Sw)`. That gives the square. (Jutul's
  `1e-10` saturation guard is omitted.)
- **Hydrocarbon MB:** `MB_c = |Σ_cells R_c M_c| / m_domain`, the same as Jutul's MB, with
  `m_domain` the domain's total mass, water included.
- **Water CNV:** recorded as `|R_w| M_w / (V ρ_w)`, which is Jutul's water CNV except for the
  sign. Jutul compares the signed value, so a negative water residual never raises its water CNV.
  ResSim's column is an upper bound on Jutul's.
- **Water MB:** recorded as `|Σ_cells R_w M_w| / m_domain`, a water conservation metric. It is
  **not** Jutul's water criterion: Jutul sums the per-cell, volume-scaled values `dt r_w / (ρ_w V)`
  and multiplies by `dt` again before dividing by the domain mass. That quantity is not a mass
  balance.

Every Jutul CNV and MB maximum quoted below comes from a hydrocarbon row, so none depends on the
water definitions. The checks in the "0.0886 d" paragraph are against Jutul's
reservoir criteria only.

#### Cross-flash: 191/192, one point undetermined

The four flashes (ResSim and MultiComponentFlash, each at both simulators' monthly `(p, z)` and at
the swapped pairs) agree on **191 of 192** points, every one of them a valid result (no flash error
or warning, a stability report present and consistent with the label): 95/96 originals and 96/96 swaps. The earlier
"95/96" counted the originals only. MultiComponentFlash's tight solve logged no warnings on any
point.

The single disagreement is month 215, cell 32, ResSim's state at 228.673 bar: ResSim splits it
(beta 0.158, Σ ln² K 1.6e-3), and Jutul's stability test calls it stable. Jutul's own forced split
from three starts converges to the same beta (fugacity residual ≤ 1.3e-14) with
`g_split − g_feed = −8.37e-12 RT`. That is inside ResSim's `1e-10` Gibbs margin and below what
either stability test resolves, so the label is **not resolved by either flash's stability test**
(a Gibbs difference that small is not shown to be beyond double precision in general). The earlier
description of this point as a Jutul stability miss is withdrawn.

The fixture was regenerated against the default-volume reference. That changes the Jutul states but
not the counts: still 191/192, with the same single point. At every month from 205 to 216, both runs
now have cell 33 two-phase, and both flashes say so at both states. Against the 1e-4 m³ reference,
Jutul's cell 33 had been single-liquid in months 212–215, about 1.5 bar above ResSim, and the swaps
had tied that label difference to pressure. No ResSim flash defect was found at either set of
states. The fixture's provenance records the generating commit and the dirty-tree flag.

The cross-flash covers monthly report states only. The plateau iterates themselves were checked
separately: at the seed plateau's two-phase iterate (228.12342 bar, beta 0.487, Σ ln² K 5.8e-4)
ResSim gives `ΔG = −4.0e-12 RT`, and Jutul's forced split stalls with ΔG straddling ±2e-12. At the
single-liquid iterate (0.001 bar higher) Jutul's forced split converges (fugacity residual ≤ 2e-13)
to V 0.4765 with `ΔG = −4.6e-13 RT`. Both stability tests call both iterates stable. The phase label
at the crossing is not determined by either flash.

#### Two failure signatures, not one

**A — seed plateau (`stall_218.json`): a label flip with a fixed conservation jump.**

| dt (d) | Converged | Last iterate: ResSim scaled | Jutul CNV | Jutul MB | max signed / inventory | Flip set |
|---|---|---|---|---|---|---|
| 0.02 | no (20) | 1.62e-3 | 1.60e-4 | 1.18e-8 | 5.4e-7 | {33, 39} |
| 1.95e-5 | no (20) | 1.79e-6 | 1.93e-7 | 1.92e-10 | 8.8e-9 | {33} |
| 9.77e-6 | no (20) | 1.19e-6 | 1.28e-7 | 1.93e-10 | 8.8e-9 | {33} |
| 4.88e-6 | no (20) | 8.91e-7 | 9.60e-8 | 1.93e-10 | 8.8e-9 | {33} |
| 2.44e-6 | **yes (17)** | 5.18e-10 | 5.6e-11 | 3.1e-15 | 1.4e-13 | — |

Below 0.02 d the iteration is an exact two-cycle on cell 33 alone. Every row above `1e-8` is cell
33 or a face neighbour (34, 40, 82), and the well rows are at ~1e-16. The cell-33 residual is
uniform across components (a total-moles defect). The signed imbalance of the two-phase side is
**+106 mol C1, 8.8e-9 of inventory, constant as dt shrinks**: switching the label changes the
cell's inventory (molar volume) by a fixed amount. It is the same from 1.95e-5 d to 4.88e-6 d, so
shrinking dt over that range does not reduce it. Accepting the
plateau would book that amount as numerical mass on every such step.

At 2.44e-6 d the second iterate already had every cell at 3.3e-9. The water injector's row was
still at 2.6e-2 because Newton starts from a BHP guess (initial residual 3.38). Its update pushed
cell 33 across the switch, and the step converged only at iteration 17. Starting from the last
accepted BHP is a candidate lever; it has not been tried.

**B — the actual from-scratch failure: a residual floor with no flip.** The replay fails at month 218
(start 6612.703 d). Ten attempts halve from 22.67 d:

| dt (d) | Last iterate: ResSim scaled | Jutul CNV | Jutul MB | max signed / inventory | Flip set |
|---|---|---|---|---|---|
| 22.67 | 2.07e+1 | 8.40e-1 | 1.91e-5 | 5.3e-4 | {32, 33, 39} |
| 2.83 | 1.22 | 5.33e-2 | 6.14e-7 | 1.1e-5 | {33, 39} |
| 0.354 | 9.46e-2 | 4.80e-3 | 1.32e-7 | 6.1e-6 | {33, 39} |
| 0.0886 | 1.40e-2 | **9.08e-4** | **4.85e-8** | 2.2e-6 | {33, 39} |
| 0.0443 | 1.36e-7 (best 3.64e-8) | 1.45e-8 | 2.2e-15 | 1.0e-13 | **none** |

The last attempt has **no phase flips**. Cells 33 and 39 stay two-phase at 228.121 bar (0.003 bar
below the switch, Σ ln² K 5.9e-4, beta 0.477, Sg 0.2655), and the residual wanders between 3.6e-8
and 1.4e-7 over cells {33, 34, 39, 40, 46, 82, 88}. Its best iterate is clean by every conservation
measure: signed imbalance 4e-14 of inventory and absolute local defect 20.5 mol C1 (1.7e-9 of inventory). The proposed
rule (a) requires flips in step with the alternation, so it **would not fire on the actual
failure**. It would fire only on the seed (A), where accepting would book a fixed imbalance.

Calling B's floor "noise" is a **hypothesis**, not a finding. Nothing yet measures what sets it:
the flash tolerance, the linear-solve accuracy, or the Jacobian's consistency near the switch. The
state before the failing call is committed as `opm/compositional/spe5/stall_218_from_scratch.json`.
`comp_spe5_from_scratch_218_seed_reproduces_signature_b` replays the call from it in about a
minute: it asserts the ten failed halvings and, for the last attempt, no flips, a best iterate
between `1e-8` and `1e-6`, and signed imbalance below `1e-12` from iteration 4 on. It reproduces
the from-scratch numbers above bit for bit.

At 0.0886 d the 20th iterate meets **both** of Jutul's reservoir tolerances (CNV 9.1e-4 < 1e-3, MB 4.9e-8 <
1e-7) while it carries a signed imbalance of 2.2e-6 of inventory and flips two cells. Jutul's
criteria are not a safe acceptance test to import into ResSim.

#### Failure B: what sets the floor

Measured from the committed seed `stall_218_from_scratch.json` on the last attempt (0.0443 d), on
a dirty tree on `8904b364` (provisional). The commands are in `compositional/spe5_failure_b_tests.rs`:

```bash
SPE5_TRACE_OUT=DIR cargo test --release --lib comp_spe5_failure_b_floor_probe -- --ignored --nocapture        # ~70 s
SPE5_TRACE_OUT=DIR cargo test --release --lib comp_spe5_failure_b_flash_conditioning -- --ignored --nocapture  # ~7 s
SPE5_TRACE_OUT=DIR cargo test --release --lib comp_spe5_failure_b_floor_scales_with_dt -- --ignored --nocapture # ~25 s
```

**Not the linear solve.** SPE5's 1,029-row system takes the FGMRES-CPR route by default. Each
iteration's report (route, Krylov iterations, true residual on the full row-scaled system) is
recorded. The same attempt on sparse LU and dense LU stalls the same way: from iteration 4 on, its
residual stays between 1.2e-8 and 1.5e-7 (sparse LU) or 2.3e-8 and 1.4e-7 (dense LU), against 3.6e-8
to 1.4e-7 on FGMRES-CPR. None reaches `1e-8`.

**Not the flash tolerance.** The same attempt with the flash's equilibrium tolerance replaced
(test-only override) by 1e-9, 1e-10, 1e-12 and 1e-13 gives best iterates 1.7e-8, 1.7e-8, 1.2e-8 and
1.2e-8, and peaks of 1.2e-7 to 1.7e-7. The floor does not follow the tolerance.

**The residual is not smooth there.** A Taylor test at iterates 2, 4, 6 and 8 runs along the step
Newton took, with the BHPs held. A repeated evaluation is bit-identical, so the residual is
deterministic. But `R(x + h d) − R(x) − h J d` stays between 2e-8 and 3e-7 for every h from 1 down
to 1e-7. It vanishes only once `h d` falls below a pressure ulp. A smooth residual would give `h²`. So
any representable change of the state moves the scaled residual by about the floor's size, and
Newton cannot get below it.

**Where the jump is.** At h = 1e-6 the largest changes are in cells 33 and 39 (every hydrocarbon
row, about equally) and their neighbours 40, 82 and 88. They are entirely in the **face and well
terms**: each cell's own inventory change explains at most 5e-12 of a 2e-8 to 1.2e-7 change. No
phase label changes.

**Why: ResSim's near-critical split is ill-determined.** Cells 33 and 39 sit at Σ ln² K ≈ 6e-4. Re-flashing
them at the two states of a 5e-13 to 8e-11 bar perturbation (composition unchanged to 2e-16)
moves β by 9e-7 to 3e-5 and x, y by 9e-10 to 3e-8. Both flashes are converged (equilibrium residual about 1.5e-12). At the
best iterate:
- Tightening the tolerance moves cell 33's β by 2.8e-5 (residual 1.6e-12 → 1.4e-14), and a further
  tightening moves it back by 2.9e-5 (residual 8.9e-15).
- Over 64 consecutive pressure ulps (2.4e-12 bar), β spans 4.2e-5 in cell 33 and 5.6e-5 in cell 39.
- In cell 40, further from the critical point, the spread is 3e-12.

So **in ResSim's current flash**, these two cells' phase split moves at the 1e-5 level, even at
round-off-level fugacity residuals. The overall inventory barely depends on it, but the phase
saturations, compositions and mobilities in the fluxes do. That is the residual's jump.

This is not shown to be a limit of double precision. The independent check below separates the
possibilities.

**Independent check of the near-critical states.** The exact inputs of cells 33, 39 and 40 (cell 40
is the control) at failure B's best iterate, 64 consecutive pressure ulps each, were written by the
conditioning test. They were solved two more ways:
- `tools/jutul/spe5_flash_mp.py`: an independent PR/PRCORR flash written from the textbook
  equations with ResSim's own parameters. It solves ln K and β by Newton to `|F| < 1e-40` in
  50-digit arithmetic, on every 4th point.
- `tools/jutul/spe5_flash_precision.jl`: MultiComponentFlash in Float64, JutulDarcy's flash, which
  shares no code with ResSim. It cannot run in BigFloat because its work storage is allocated as
  Float64.

| Cell 33 (39 alike) | β minus the 50-digit β | Float64 fugacity residual |
|---|---|---|
| 50-digit split, evaluated by MultiComponentFlash in Float64 | 0 | 2.7e-15 to 1.3e-14 |
| ResSim, default tolerance 1e-11 | −9.7e-6 to +1.2e-5 | 1.3e-12 to 1.8e-12 |
| ResSim at the best iterate, tolerance 1e-12 / 1e-14 | −1.9e-5 / +9.6e-6 | 1.4e-14 / 8.9e-15 |
| MultiComponentFlash, forced from Wilson's K, V0 = 0.3 or 0.5 | −1.9e-5 to +2.6e-5 | not converged at 1e-13 in 200,000 iterations |
| 50-digit β, spread over the sampled ulps | 1.9e-11 | — |

Cell 40 converges in 13–27 MultiComponentFlash iterations, within 1e-12 of the exact β, as ResSim
does.

What this establishes:
- **The split is well determined mathematically.** The exact β moves smoothly with pressure,
  about 1e-11 over the sweep.
- **In Float64, fugacity equality cannot discriminate within a band of about ±2e-5 in β at these
  states.** Splits that far apart reach the same 1e-14 residual as the exact one. A tighter
  tolerance only selects another point in the band.
- **An independent Float64 implementation has the same difficulty.** MultiComponentFlash does not
  converge there from its own start, and its stalled answers scatter over the same band.
- ResSim's split satisfies material balance to 5e-16 with its own β. The error is in x and y
  (1.2e-8, against |y − x| ≈ 1e-3), and β inherits it as `(z − x)/(y − x)`. An earlier estimate
  here (β pinned to ~1e-10 by Rachford–Rice) was wrong: it assumed K pinned, and K is what is not.

So the floor is a property of solving fugacity equality in Float64 at this near-critical state. It
is neither a ResSim-only defect nor a demonstrated limit of double precision in general. A
better-conditioned formulation, or extended-precision evaluation of the fugacity difference, might
pin the split down. A tighter tolerance cannot.

There is also a cheaper lever. Newton needs a **smooth** split, not an exact one. The jump comes
from the flash's variable path: where it stops and how many iterations it takes changes with tiny
input changes. A deterministic seed and a fixed final polish would make the split a smooth function
of (p, z), even 1e-5 off the exact one. That is a candidate, not tried.

**It scales with dt, but the outcome at a given dt is not robust.** The jump enters the scaled
residual as flux × dt / inventory, so it should fall in proportion to dt. Halving below the
ladder's last attempt, from the committed seed:

| dt (d) | Best scaled residual | Converged at `1e-8` |
|---|---|---|
| 0.0443 (the ladder's 10th attempt) | 3.6e-8 | no (20 iterations) |
| 0.0221 | 5.2e-9 | yes (13 iterations) |
| 0.0111 | 3.0e-9 | yes (4) |
| 0.0055 | 4.3e-9 | yes (4) |
| 0.0028 | 5.9e-10 | yes (4) |
| 0.0014 | 1.7e-9 | yes (4) |

This table was first read as "failure B is the halving budget stopping one halving short". The
retry-budget runs below refute that reading. The month's remainder computed as `end − now` is
22.671554013151763 d. The from-scratch loop's accumulated remainder is 22.671554013151308 d, a
relative difference of 2e-14. That alone changes the failing call's last residual from 1.36e-7
to 6.39e-8, and makes the 11th halving (0.0221 d) fail at 4.93e-8 instead of converging. At this
state, whether an attempt converges is decided by round-off in its dt. The trend with dt holds
across the ladder; any single attempt's outcome does not. The "noise floor" has a measured path:
a near-critical phase split that Float64 fugacity equality does not pin down reaches the residual
through the face fluxes.

#### Retry budget and ResSim's timestep convergence (`1e-8` unchanged)

Both use the ignored test `comp_spe5_replay_with_timestep_options`
(`compositional/spe5_timestep_control_tests.rs`, configured through `SPE5_*` variables, see its
module docs). It records per month the steps, failed attempts, Newton work, wall time and the
signed global component balance, and stops at the first failed step. The numbers are provisional:
a release build of `7295a93c` plus the uncommitted test-only driver. Wall times are from a
single-core box running several of these at once, so they are only indicative.

**A larger retry budget does not carry B through.** From `stall_218_from_scratch.json`:
- `max_attempts = 11` fails at the 11th halving, residual 4.93e-8, after 67 s.
- `max_attempts = 14` advances 0.110 d (6612.703 → 6612.814 d) of the month's 22.67 d in 948 s,
  then fails after 14 attempts, residual 4.39e-8.

Every call restarts from the whole remainder and pays the full ladder before a step small enough
to converge, and some of those small steps still fail. A larger budget alone is not a fix. Bounded
timestep memory (start the next call from the last accepted dt, grow it gradually) is the
next timestep-control candidate. It is not tried here.

**ResSim converges in time like JutulDarcy does.** ResSim from scratch, with no cap, a 5 d cap
and a 0.5 d cap on each call, monthly reports and WAG event times kept, against the JutulDarcy
runs (default well volume) with the same caps. Printed by
`python3 -I tools/jutul/spe5_compare.py --timestep-convergence R_REPORT R_5 R_05 J_REPORT J_5 J_05`:

| ResSim run | Max dt (d) | Months completed | Accepted steps / failed attempts | Newton (accepted) | Wall (s) | Max \|signed balance\| | Stopped |
|---|---|---|---|---|---|---|---|
| ResSim report | report | 1–217 | 242 / 42 | 1,567 | 669 | 1.1e-09 | month 218, day 6612.703: step failed, 6.393474812685191e-8 |
| ResSim 5 d | 5 | 1–215 | 1,507 / 10 | 7,640 | 1,567 | 5.2e-09 | month 216, day 6555.996: step failed, 1.1115518820738136e-4 |
| ResSim 0.5 d | 0.5 | 1–211 | 12,877 / 6 | 63,281 | 8,941 | 6.1e-09 | month 212, day 6432.697: step failed, 1.5043531698317281e-6 |

Worst difference over months 1–211 (month):

| Pair | mean p (bar) | cell-33 p (bar) | producer oil (rel.) | GOR (rel.) | water cut (abs.) | cell-33 Sg (abs.) | Σ oil |
|---|---|---|---|---|---|---|---|
| ResSim report vs ResSim 0.5 d | 2.04 (176) | 3.14 (178) | 32.6% (208) | 28.0% (184) | 0.0555 (170) | 0.0782 (180) | 0.61% |
| ResSim 5 d vs ResSim 0.5 d | 0.454 (176) | 0.666 (179) | 8.2% (207) | 7.5% (164) | 0.0148 (169) | 0.0168 (180) | 0.10% |
| Jutul report vs Jutul 0.5 d | 1.48 (176) | 2.53 (178) | 24.1% (208) | 27.2% (184) | 0.0449 (170) | 0.0766 (180) | 0.61% |
| Jutul 5 d vs Jutul 0.5 d | 0.365 (176) | 0.577 (179) | 8.2% (207) | 6.8% (184) | 0.0122 (169) | 0.0165 (180) | 0.09% |
| ResSim report vs Jutul report | 0.673 (211) | 0.744 (181) | 11.7% (208) | 6.4% (139) | 0.0177 (208) | 0.0194 (211) | 0.01% |
| ResSim 5 d vs Jutul 5 d | 0.161 (4) | 0.168 (15) | 0.9% (207) | 1.0% (38) | 0.0011 (207) | 0.0017 (33) | 0.00% |
| ResSim 0.5 d vs Jutul 0.5 d | 0.172 (4) | 0.176 (4) | 1.0% (185) | 1.0% (207) | 0.00265 (185) | 0.00403 (211) | 0.01% |

Producer rate-to-BHP switch month: ResSim report 16, ResSim 5 d 16, ResSim 0.5 d 16, Jutul report 16, Jutul 5 d 16, Jutul 0.5 d 16
ResSim report vs ResSim 0.5 d, cumulative surface production at month 211: oil 0.101%, gas 0.222%
ResSim 5 d vs ResSim 0.5 d, cumulative surface production at month 211: oil 0.020%, gas 0.052%

Cell-33 Sg by month, ResSim report / ResSim 5 d / ResSim 0.5 d / Jutul report / Jutul 5 d / Jutul 0.5 d:

| Month | ResSim report | ResSim 5 d | ResSim 0.5 d | Jutul report | Jutul 5 d | Jutul 0.5 d |
|---:|---:|---:|---:|---:|---:|---:|
| 205 | 0.1686 | 0.1815 | 0.1841 | 0.1739 | 0.1820 | 0.1852 |
| 206 | 0.1557 | 0.1658 | 0.1667 | 0.1620 | 0.1660 | 0.1677 |
| 207 | 0.1336 | 0.1363 | 0.1345 | 0.1375 | 0.1361 | 0.1355 |
| 208 | 0.1185 | 0.1076 | 0.1066 | 0.1128 | 0.1073 | 0.1070 |
| 209 | 0.1064 | 0.0878 | 0.0834 | 0.1010 | 0.0875 | 0.0837 |
| 210 | 0.0861 | 0.0681 | 0.0660 | 0.0775 | 0.0677 | 0.0652 |
| 211 | 0.0668 | 0.0191 | 0.0168 | 0.0474 | 0.0178 | 0.0128 |
| 212 | 0.0563 | 0.0000 | — | 0.0194 | 0.0000 | 0.0000 |
| 213 | 0.0665 | 0.0000 | — | 0.0117 | 0.0000 | 0.0000 |
| 214 | 0.1036 | 0.0000 | — | 0.0456 | 0.0000 | 0.0000 |
| 215 | 0.1664 | 0.0000 | — | 0.1022 | 0.0000 | 0.0000 |
| 216 | 0.2168 | — | — | 0.1711 | 0.0657 | 0.0238 |
| 217 | 0.2515 | — | — | 0.1554 | 0.0000 | 0.0000 |

What it shows:
- **At matched caps, ResSim and JutulDarcy agree** through month 211: within 1.0% in oil, 0.18 bar
  and 0.004 in cell-33 Sg, at both 5 d and 0.5 d. At report steps they differ by 11.7% in oil.
- **Their time errors are alike.** Report step to 0.5 d moves ResSim 33% in oil and Jutul 24%;
  5 d to 0.5 d moves both 8.2%. These are timestep-dependent differences of the same size in
  both simulators, not errors against a converged solution: neither 0.5 d run is shown to be
  converged, since no finer cap was run.
- **Cumulatives are much less sensitive than rates.** ResSim's cumulative surface oil at month
  211 differs by 0.10% between report steps and 0.5 d; the rate peaks differ by 33%.
- **Smaller steps fail earlier, at the same physical event.** This exposes a robustness failure
  earlier; it says nothing against the smaller steps' accuracy. With the caps, cell 33's vapour
  disappears at month 211–212 in both simulators. The report-step runs keep free gas there. The
  0.5 d ResSim run fails in that window (day 6432.7, residual 1.5e-6), the 5 d run in the
  reappearance window (day 6556.0, residual 1.1e-4). Jutul's capped runs take 48–153 failed
  ministeps in the reappearance months. Every ResSim run that has reached this window has failed
  within 10 attempts at `1e-8`. **Correction (traces below):** the capped failures are not at
  cell 33. Their failing attempts are phase-label two-cycles on cell 16 (0.5 d) and cells 25/31
  (5 d). Both converge one halving past the ladder, and with a larger budget fail again a step or
  two later; see "The crossings with the refinement
  and with timestep memory".
- **Conservation holds.** The worst signed component balance relative to the initial inventory is
  6.1e-9 after 12,877 accepted 0.5 d-capped steps.

Consequences:
- **Acceptance.** A residual-based acceptance below the floor is unreachable at large dt there. An
  acceptance that knew the residual's evaluation precision would be a different, principled
  candidate. It is not proposed here.
- **Timestep control** cannot be measured on the month-218 window alone: the capped runs fail at
  other transitions. A candidate has to carry all three runs through months 211–240. A cap is not
  timestep memory (after a small accepted step the next call asks for the cap again), so the
  capped failures do not test memory; they make it a lower priority than the floor's cause.
- **Flash.** A deterministic seed or a fixed polish would not help: the flash is already
  deterministic at identical inputs, and the scatter is the residual's evaluation error amplified
  by the near-singular system. What helps is evaluating that residual more accurately (next
  section).

#### Difference-form flash refinement (experimental, off by default)

**Why the split scatters.** At the exact cell-33 split, the 50-digit Jacobian of the equilibrium
system `[ln K, beta]` has singular values from 4.0 down to 3.9e-6 and then **5.7e-11**, with a
right singular vector that is almost pure `beta` (cell 39 alike; cell 40, the control, 7e-4). An
error `e` in the residual moves `beta` by `e / 5.7e-11`: 1e-15 gives 1.8e-5. The standard Float64
residual forms `ln phi^V - ln phi^L` from terms up to 11 in size, so its error is ~1e-15, which
is the observed ±2e-5 band. The exact `beta` moves −8.5 per bar here, while Z_L − Z_V = 1.2e-4.

**The candidate.** `fluid::flash` can polish a converged two-phase split with the same equations,
with the same normalization, but every vapour-minus-liquid difference formed directly:
- `y − x` from `expm1(ln K)`;
- the differences of the mixture sums;
- Z_V − Z_L from the two cubics through their divided difference;
- each logarithm's difference through `ln_1p` of a small ratio.

The residual's error then scales with the phases' difference (|ln K| ≈ 0.017) instead of with
`ln phi`. Newton steps use the existing AD Jacobian and stop when that residual stops
decreasing (at most 8). A move larger than 1e-3 in `beta` or `ln K`, or a result outside the
existing equilibrium tolerance, is discarded and the unrefined state returned. The returned split
is what the derivative pass differentiates (it seeds `ln K` from the returned phases' `ln phi`
and takes `beta` as returned), so the implicit Jacobian describes the refined split.

It is **off in every non-test build**. A test switches it on for its thread
(`with_difference_refinement_for_test`); the replay driver does it with `SPE5_FLASH_REFINE=1`.
Two thermo-gate tests pin the contract:
- `comp_flash_difference_residual_matches_the_standard_one`: on every two-phase fixture state,
  and 1e-3 away from it, the two residuals agree to 1e-12;
- `comp_flash_refinement_leaves_ordinary_states_unchanged`: with it on, no fixture state changes
  label, and `beta`, x and y move by at most 1e-8 / 1e-9.

**Accuracy on the captured states.** Cells 33, 39 and 40 of failure B's best iterate, under 64
pressure ulps, 32 steps of 0.01 Pa, and 32 composition steps of 4e-16 (C1 up, the heaviest down),
each flashed both ways and solved in 50 digits. Printed by
`python3 -I tools/jutul/spe5_flash_refinement_compare.py STATES.json MP.json`:

| Cell | Sweep | Mode | max \|Δβ\| | mean Δβ | max \|Δx\|, \|Δy\| | max rel. Δv_L, Δv_V | max \|ΔS_V\| | mobility noise L / V (rel.) | difference residual | standard residual |
|---|---|---|---|---|---|---|---|---|---|---|
| 33 | composition | unrefined | 2.68e-05 | -1.0e-06 | 2.9e-08 | 4.5e-09 | 2.7e-05 | 8.2e-04 / 2.0e-04 | 5.9e-14 | 1.8e-12 |
| 33 | composition | refined | 6.50e-08 | +7.5e-09 | 7.1e-11 | 1.1e-11 | 6.5e-08 | 1.9e-06 / 4.7e-07 | 9.6e-15 | 1.2e-14 |
| 33 | pressure_0.01pa | unrefined | 2.99e-05 | -2.3e-06 | 3.2e-08 | 5.0e-09 | 3.0e-05 | 4.7e-04 / 1.2e-04 | 6.3e-14 | 1.9e-12 |
| 33 | pressure_0.01pa | refined | 8.66e-08 | +4.8e-09 | 9.4e-11 | 1.4e-11 | 8.7e-08 | 1.5e-06 / 3.6e-07 | 9.7e-15 | 1.4e-14 |
| 33 | pressure_ulps | unrefined | 2.14e-05 | -2.7e-06 | 2.3e-08 | 3.6e-09 | 2.1e-05 | 7.2e-04 / 1.7e-04 | 6.0e-14 | 1.9e-12 |
| 33 | pressure_ulps | refined | 7.06e-08 | -7.8e-10 | 7.7e-11 | 1.2e-11 | 7.1e-08 | 2.2e-06 / 5.4e-07 | 1.1e-14 | 1.4e-14 |
| 39 | composition | unrefined | 3.79e-05 | -2.5e-06 | 4.2e-08 | 6.3e-09 | 3.8e-05 | 1.1e-03 / 2.7e-04 | 6.6e-14 | 1.9e-12 |
| 39 | composition | refined | 8.55e-08 | +1.4e-08 | 9.3e-11 | 1.4e-11 | 8.6e-08 | 2.3e-06 / 5.7e-07 | 8.8e-15 | 1.4e-14 |
| 39 | pressure_0.01pa | unrefined | 1.76e-05 | -9.5e-07 | 1.9e-08 | 2.9e-09 | 1.8e-05 | 3.1e-04 / 7.5e-05 | 5.4e-14 | 1.8e-12 |
| 39 | pressure_0.01pa | refined | 9.33e-08 | +8.9e-10 | 1.0e-10 | 1.5e-11 | 9.3e-08 | 1.6e-06 / 3.9e-07 | 1.2e-14 | 1.4e-14 |
| 39 | pressure_ulps | unrefined | 3.06e-05 | -6.1e-07 | 3.4e-08 | 5.1e-09 | 3.1e-05 | 9.6e-04 / 2.3e-04 | 6.1e-14 | 1.8e-12 |
| 39 | pressure_ulps | refined | 1.06e-07 | +4.2e-09 | 1.2e-10 | 1.8e-11 | 1.1e-07 | 3.6e-06 / 8.9e-07 | 1.0e-14 | 1.8e-14 |
| 40 | composition | unrefined | 2.08e-12 | +3.2e-13 | 1.4e-13 | 7.5e-14 | 2.0e-12 | 4.2e-11 / 1.9e-11 | 1.3e-14 | 1.4e-14 |
| 40 | composition | refined | 1.34e-12 | +1.4e-13 | 9.0e-14 | 4.8e-14 | 1.3e-12 | 3.0e-11 / 1.4e-11 | 9.8e-15 | 1.4e-14 |
| 40 | pressure_0.01pa | unrefined | 2.40e-12 | -1.3e-13 | 1.6e-13 | 8.5e-14 | 2.4e-12 | 2.6e-11 / 1.2e-11 | 1.2e-14 | 2.1e-14 |
| 40 | pressure_0.01pa | refined | 1.52e-12 | -6.0e-14 | 1.0e-13 | 5.3e-14 | 1.5e-12 | 1.7e-11 / 7.9e-12 | 1.2e-14 | 1.4e-14 |
| 40 | pressure_ulps | unrefined | 1.79e-12 | -4.3e-14 | 1.2e-13 | 6.3e-14 | 1.8e-12 | 3.7e-11 / 1.7e-11 | 1.5e-14 | 2.5e-14 |
| 40 | pressure_ulps | refined | 9.43e-13 | +1.3e-14 | 6.2e-14 | 3.2e-14 | 9.3e-13 | 1.6e-11 / 7.6e-12 | 1.0e-14 | 2.0e-14 |

- **It improves accuracy, not just repeatability.** At cells 33/39 the `beta` error falls from
  2–4e-5 to ≤ 1.1e-7, with a mean offset ≤ 1.4e-8, so no bias is introduced. Compositions improve
  from 3e-8 to 1e-10, molar volumes from 5e-9 to 1.5e-11 (relative), and the vapour volume
  fraction from 3e-5 to 1e-7. Mobility scatter falls from 1e-3 to 2–4e-6 (relative).
- **Both residual measures fall.** Even the standard equilibrium residual falls (1.9e-12 to
  1.4e-14).
- **The control cell does not move.** Cell 40 is unchanged or slightly better.
- **The 0.01 Pa sweep is real physics.** The 3.1e-6 bar sweep moves the exact `beta` by 2.6e-5,
  which the refined flash follows.

**Failure B with the refinement**
(`cargo test --release --lib comp_spe5_failure_b_with_refinement -- --ignored --nocapture`, ~3 min):

| | Unrefined | Refined |
|---|---|---|
| Last attempt (0.0443 d) | best 3.6e-8, not converged in 20 | **converged in 4**, 4.1e-10 |
| Taylor error, iterate 2, h = 1 / 0.1 / ≤ 0.01 | 2e-8 to 3e-7 at every h | 1.4e-7 / 1.2e-9 / 2–8e-10 |
| One attempt at 0.0886 d … 22.67 d | best 4.9e-3 … 0.60 | identical |
| The failing call (whole remainder, 10 attempts) | fails | **succeeds at the 10th attempt** (0.0443 d) |

- **Taylor test.** It now falls about 100× per decade of h, as a consistent Jacobian should,
  down to a floor about 100× lower than before.
- **Larger steps.** Steps of 0.089 d and more still fail, identically with or without the
  refinement. Their residuals stay at 5e-3 to 0.6: genuine nonconvergence at large dt, not
  noise. So the crossing still needs steps of about 0.04 d, and each call that restarts from the
  whole remainder pays the halving ladder. That cost is a timestep-control question again.

#### The crossings with the refinement and with timestep memory (`1e-8` unchanged)

Provisional: release builds of `1fb3dcdb` plus the then-uncommitted test drivers
(`SPE5_DT_MEMORY_GROWTH`, `comp_spe5_trace_seed_ladder`). Every failing call's starting state was
written with `SPE5_REPLAY_CHECKPOINT_OUT` and traced with `comp_spe5_trace_seed_ladder`.

**Replays.** From scratch with the refinement on, at the same three settings as the unrefined
runs, and from the failure-B seed with bounded timestep memory (growth 2: after an accepted step
`dt`, the next request is at most `2 dt`):

| Run | Unrefined | Refined |
|---|---|---|
| From scratch, 0.5 d cap | fails day 6432.697, 1.50e-6 | fails day 6432.697, 1.48e-6 |
| From scratch, 5 d cap | fails day 6555.996, 1.11e-4 | fails day 6555.996, 1.11e-4 |
| From scratch, report steps | fails day 6612.703 (B), 6.4e-8 | 2 steps to day 6612.792, then fails (5.06e-3) |
| B seed, memory growth 2 | fails on the first call (10 attempts) | 10 steps to day 6612.821 (0.117 d, 249 s), then the request falls below the 1e-6 d minimum after 3 attempts (1.1e-6) |

**Traces of each failing call.** One Newton solve per `dt`, 20 iterations; "flips" are the
cells whose phase label changes on every iteration of the last ones:

| Starting state | Failing attempts | Mechanism | First converging `dt` |
|---|---|---|---|
| 0.5 d cap, day 6432.697 | 3.9e-3 … 9.8e-4 d (1.5e-6 at the ladder's last), refined also 4.9e-4 d | two-cycle on cell 16; residual halves with `dt` | 4.9e-4 d unrefined, 2.4e-4 d refined; 6 iterations, no flips, at every smaller `dt` |
| 5 d cap, day 6555.996 | 3.9e-2 … 9.8e-3 d (1.1e-4 at the ladder's last), both modes identical | two-cycle on cells 25/31; residual halves with `dt` | 4.9e-3 d in both modes; 6 iterations, no flips, at every smaller `dt` |
| Report steps, refined, day 6612.792 | 0.044 d (3.3e-3 ↔ 5.1e-3) | two-cycle on cells 33/39 | 0.022 d, then every `dt` to 6.7e-7 d in 4 iterations |
| Memory run, refined, day 6612.821 | every `dt` from 0.02 d to 4.9e-6 d, both modes | two-cycle on cells 33/39 (39 alone for iterations 2–11 at 4.9e-6 d); residual 8.9e-7 at 4.9e-6 d, no longer halving | none in the traced range |

What it shows:
- **The refinement is refuted as the fix for the crossings.** It removes B's precision floor (the
  previous section) and changes nothing else: the capped failures are bit for bit or nearly the
  same, and the report-step and memory runs reach the next failure a few hours later.
- **Every failure traced here is signature A**, a phase-label two-cycle in a near-critical cell,
  not B's floor. Only failure B's last attempt was a precision floor, and the refinement removes it.
- **The capped failures are not a budget problem.** At the first `dt` past the 10-attempt ladder
  they converge in 6 iterations with no flips, and at every smaller `dt` traced. But with 12
  attempts (`SPE5_MAX_ATTEMPTS=12`, from the captured states) each run takes one or two such steps
  and the next call fails all 12: the 5 d run at day 6556.001 (2.7e-5), the 0.5 d run at day
  6432.698 (3.7e-7). Traced, the same cells flip, and the largest converging `dt` has shrunk
  eightfold, 4.9e-3 → 6.1e-4 d and 4.9e-4 → 6.1e-5 d. As the cell approaches its transition, only
  steps that keep it on one side converge, and those shrink with the distance left. No step
  crosses the transition, so no budget or remembered step can carry the run over it.
- **Timestep memory is inconclusive.** With the refinement it carried the run 0.117 d past B,
  and its steps then shrank to a few 1e-6 d. The state where it stopped is a two-cycle on cells
  33/39 that does not converge down to 4.9e-6 d, like the month-218 seed of signature A, which
  converged only at 2.44e-6 d. A remembered step cannot help where no step converges.
- **What is indicated next** is candidate 1 of the revised proposal: keep a cell's phase label
  during one Newton solve unless the competing label is clearly preferred, so that a step can
  converge on one label and cross the transition. It acts on the mechanism of every failure
  traced here and does not touch acceptance.

#### BHP warm start (separate experiment, `1e-8` unchanged)

`NewtonOptions::well_bhp_start = WellBhpStart::LastAccepted`, **off by default**, starts each
rate-controlled well's BHP at its last accepted value instead of the control's guess. The default
path is unchanged. Run with `cargo test --release --lib comp_spe5_bhp_warm_start_experiment --
--ignored --nocapture` (~3 min).

| | Control guess (default) | Last accepted BHP |
|---|---|---|
| A, dt 0.02 d | no (best 1.35e-3) | no (9.8e-4) |
| A, dt 1.95e-5 / 9.77e-6 / 4.88e-6 d | no (best 1.8e-6 / 1.2e-6 / 8.9e-7) | no (best 9.6e-7 / 4.8e-7 / 2.4e-7) |
| A, dt 2.44e-6 d | yes, 17 iterations (first residual 3.38) | yes, **2 iterations** (first residual 1.2e-7) |
| B, the failing call (10 halvings) | fails, last 1.3648759701094485e-7 | fails identically |
| B, last attempt (0.0443 d) | best 3.6e-8 | best 3.6e-8 (first residual 3.38 → 2.2e-3) |

The warm start removes the BHP-driven crossing on A's smallest step (17 iterations → 2). It does
not resolve A's plateau. With the warm start, the first iterate of each failing rung is already
its best, and that floor halves with dt (9.6e-7, 4.8e-7, 2.4e-7), like B's. It has no effect on B:
the first residual drops, but the best and last residuals are identical. B's floor does not come
from the wells. Verdict: **not promoted**. It leaves both failures failing at `1e-8`.

#### What JutulDarcy accepts in the same window

Per-ministep reservoir criteria of Jutul's ministeps, from `spe5_ministep_diagnostics.jl` run on
the reference model with JutulDarcy's default well volume (see "Well-node volume" below). The table
is printed by `python3 -I tools/jutul/spe5_compare.py --ministeps OUT.json`. MB and CNV are the
final Newton iterate of each accepted ministep, over hydrocarbon components only. Jutul's water
"MB" is not a mass balance, so it gets its own column (see the definitions above).

| Month | Accepted / failed ministeps | Median accepted dt (d) | Accepted with MB > 1e-8 / > 5e-8 | Accepted with CNV > 1e-4 | Σ accepted MB, worst hydrocarbon | Σ accepted Jutul water "MB" |
|---|---|---|---|---|---|---|
| 205–215 | 52 / 36 | 2.48 | 21 / 13 | 24 | 1.4e-06 (C1) | 5.5e-08 |
| 216 | 47 / 76 | 0.319 | 44 / 28 | 45 | 2.7e-06 (C1) | 1.7e-08 |
| 217 | 151 / 237 | 0.185 | 151 / 132 | 151 | 1.1e-05 (C1) | 2.8e-08 |
| 218 | 735 / 1,164 | 0.0287 | 735 / 725 | 710 | 6.3e-05 (C1) | 1.4e-08 |
| 219 | 394 / 616 | 0.0526 | 386 / 297 | 392 | 2.6e-05 (C1) | 1.7e-08 |
| 220 | 2 / 0 | 15.2 | 0 / 0 | 0 | 1.4e-09 (C6) | 7e-10 |

All 36 failed ministeps in months 205–215, and every failed ministep from month 216 on, have the
reservoir unconverged. Through the crossing, Jutul routinely accepts ministeps whose domain mass
balance is within a factor of two of its `1e-7` limit (725 of 735 in month 218). The sum of
accepted MB over months 205–220 is at most **1.0e-4** of domain mass, for C1. The sign is not
recorded, so this is an upper bound on Jutul's own cumulative imbalance, not a measured one. In
the same normalization, ResSim's signature-A plateau carries 1.9e-10 per step and its signature-B
floor 2e-15.

So Jutul's accepted ministeps in the window sit close to its MB limit. That shows the window can be
crossed at JutulDarcy's default tolerances. It does not show that a tighter tolerance would fail to
cross, which was not tried. It also does not give ResSim a threshold, and the budgets below are
deliberately orders tighter than the reference.

#### Well-node volume: a model choice that also sets the convergence cost

The first reference (`b14f0976`) built every well with a 1e-4 m³ node volume. The SPE5 generator
did not say why, but the reason is recorded for the water-injection fixture
(`docs/COMPOSITIONAL_VALIDATION.md`, "Wellbore storage was the first mismatch"). ResSim's wells
have no wellbore storage. A JutulDarcy simple-well node stores fluid and injects it: at its default
volume it gave a 1.3e-2 bar, Δt-independent mismatch there, which a 1e-4 m³ node removed. So the
node volume changes the model (well storage), the well-node convergence scaling and the timestep
path, not only the run time.

For SPE5, recording every equation group per Newton step showed what the small node cost:
- **1,139 failed ministeps in months 205–215** (the gas year) had the reservoir converged to CNV
  ~3e-11 and failed only on the gas injector's well-node CNV (tolerance 1e-2).
- That residual stayed fixed across all 16 Newton iterations and grew exactly linearly with dt, at
  0.01556 per day, because the node-mass normalization divides by the tiny node's mass. Every
  ministep over 0.64 d failed after 16 iterations and was cut.
- 89% of the run's Newton work in months 205–220 went to failed ministeps. JutulDarcy's own
  `setup_well` documentation warns that small well volumes slow convergence.

The regenerated reference uses JutulDarcy's default volume, radius 0.1 m times the cell length
along the well: **0.479 m³ for the producer and 0.192 m³ per injector**, about 4.5e-6 of the perforated cell's
pore volume (`tools/jutul/spe5_reference.jl`; `SPE5_WELL_VOLUME_M3` overrides every well's volume, and
`manifest.json` records the resolved volumes). That makes the reference cheaper. It does not make it
more accurate as a model of ResSim's storage-free wells, and closer agreement with ResSim does not
show that it is.

Printed by `python3 -I tools/jutul/spe5_compare.py --references default=… 1e-4=…`, the 1e-4 m³
run being the superseded `b14f0976` reference:

| Run | Well volumes (m³) PROD / INJW / INJG | Max dt (d) | Completed | Ministeps / Newton / failed | Month 216 / 217 / 218 / 219 ministeps | First declared stop |
|---|---|---|---|---|---|---|
| default | not recorded | not recorded | True (240 reports) | 3,843 / 39,716 / 2,130 | 123 / 388 / 1,899 / 1,010 | 6300.5625 d, watercut |
| 1e-4 | not recorded | not recorded | True (240 reports) | 27,353 / 296,291 / 16,386 | 193 / 272 / 415 / 1,477 | 6300.5625 d, watercut |

Worst difference from `default`, months 1–217 / all shared months (month):

| Run | mean p (bar) | cell-33 p (bar) | producer oil (rel.) | GOR (rel.) | water cut (abs.) | cell-33 Sg (abs.) |
|---|---|---|---|---|---|---|
| 1e-4 | 1.28 (175) / 2.41 (234) | 1.6 (177) / 2.25 (233) | 19.0% (208) / 35.1% (234) | 19.3% (164) / 29.4% (233) | 0.05 (169) / 0.05 (169) | 0.102 (215) / 0.565 (222) |

(Neither of these two runs recorded its configuration; the default run's volumes are the ones
quoted above.) The crossing stays hard with either volume, and month 218 is worse with the default
one. The declared stop fires on the same day. Otherwise the answer **does** move with the node
volume, and not only in the crossing window. Over months 1–217 the two Jutul runs differ by up to
1.3 bar in mean pressure, 19% in producer oil and GOR, and 0.05 in water cut. That is the same size
as the ResSim/Jutul differences above (0.86 bar, 11.7%, 10.5%, 0.018). So the reference's
uncertainty from this one setting is as large as the gap it is used to judge. In the crossing
window cell 33's split moves too, by up to 0.565 in Sg at month 222. Jutul's cell-level state there
is not a target trajectory for ResSim.

The node volume changes at least three things at once: the well storage, the well rows'
convergence scaling, and the accepted timestep sequence. This comparison does not separate them,
and the bounded sensitivity below narrows the question without settling it.

#### Volume/timestep sensitivity

`bash tools/jutul/run-spe5-sensitivity.sh OUTDIR` ran on a clean worktree at `8904b364`, with the
driver of that commit. All seven runs were then validated, and the tables printed, by
`run-spe5-sensitivity.sh --validate-only` from `38ff6c0a`. Every run is complete through 7305 d and
records the requested configuration. The `default` run reproduces the committed reference
exactly, and `vol-1e-4` reproduces the superseded `b14f0976` reference's work counts and
differences.

| Run | Well volumes (m³) PROD / INJW / INJG | Max dt (d) | Completed | Ministeps / Newton / failed | Month 216 / 217 / 218 / 219 ministeps | First declared stop |
|---|---|---|---|---|---|---|
| default | 0.479 / 0.192 / 0.192 | report | True (240 reports) | 3,843 / 39,716 / 2,130 | 123 / 388 / 1,899 / 1,010 | 6300.5625 d, watercut |
| vol-1e-4 | 0.0001 / 0.0001 / 0.0001 | report | True (240 reports) | 27,353 / 296,291 / 16,386 | 193 / 272 / 415 / 1,477 | 6300.5625 d, watercut |
| vol-1e-3 | 0.001 / 0.001 / 0.001 | report | True (240 reports) | 3,974 / 40,360 / 2,125 | 85 / 360 / 751 / 673 | 6300.5625 d, watercut |
| vol-1e-2 | 0.01 / 0.01 / 0.01 | report | True (240 reports) | 3,502 / 36,414 / 1,959 | 97 / 404 / 1,580 / 1,006 | 6300.5625 d, watercut |
| default-dt5 | 0.479 / 0.192 / 0.192 | 5 | True (240 reports) | 3,618 / 27,021 / 1,235 | 235 / 143 / 140 / 1,303 | 6300.5625 d, watercut |
| default-dt0.5 | 0.479 / 0.192 / 0.192 | 0.5 | True (240 reports) | 15,537 / 43,035 / 613 | 207 / 132 / 61 / 740 | 6300.5625 d, watercut |
| vol-1e-4-dt0.5 | 0.0001 / 0.0001 / 0.0001 | 0.5 | True (240 reports) | 28,421 / 205,090 / 9,144 | 381 / 147 / 155 / 726 | 6300.5625 d, watercut |

Worst difference from `default`, months 1–217 / all shared months (month):

| Run | mean p (bar) | cell-33 p (bar) | producer oil (rel.) | GOR (rel.) | water cut (abs.) | cell-33 Sg (abs.) |
|---|---|---|---|---|---|---|
| vol-1e-4 | 1.28 (175) / 2.41 (234) | 1.6 (177) / 2.25 (233) | 19.0% (208) / 35.1% (234) | 19.3% (164) / 29.4% (233) | 0.05 (169) / 0.05 (169) | 0.102 (215) / 0.565 (222) |
| vol-1e-3 | 1.06 (175) / 1.58 (234) | 1.34 (177) / 1.43 (233) | 10.9% (208) / 21.4% (234) | 15.5% (164) / 19.2% (233) | 0.0406 (169) / 0.0406 (169) | 0.0866 (215) / 0.0866 (215) |
| vol-1e-2 | 0.187 (169) / 0.351 (228) | 0.233 (166) / 0.345 (239) | 1.5% (159) / 7.0% (237) | 1.9% (31) / 5.8% (237) | 0.00314 (169) / 0.0104 (218) | 0.00649 (217) / 0.0144 (221) |
| default-dt5 | 1.18 (159) / 4.13 (232) | 1.95 (178) / 4.07 (232) | 18.1% (208) / 36.1% (233) | 21.9% (184) / 28.4% (233) | 0.0345 (170) / 0.0345 (170) | 0.155 (217) / 0.563 (223) |
| default-dt0.5 | 1.48 (176) / 5.16 (232) | 2.53 (178) / 5.07 (232) | 24.1% (208) / 38.6% (233) | 27.2% (184) / 30.9% (239) | 0.0449 (170) / 0.0449 (170) | 0.155 (217) / 0.563 (223) |
| vol-1e-4-dt0.5 | 1.55 (176) / 5.23 (232) | 2.58 (178) / 5.18 (232) | 23.6% (208) / 42.6% (234) | 26.7% (184) / 31.0% (239) | 0.0461 (170) / 0.0461 (170) | 0.155 (217) / 0.538 (222) |

The same runs, against the 0.5 d default-volume run
(`spe5_compare.py --references default-dt0.5=… vol-1e-4-dt0.5=… default-dt5=…`):

Worst difference from `default-dt0.5`, months 1–217 / all shared months (month):

| Run | mean p (bar) | cell-33 p (bar) | producer oil (rel.) | GOR (rel.) | water cut (abs.) | cell-33 Sg (abs.) |
|---|---|---|---|---|---|---|
| vol-1e-4-dt0.5 | 0.074 (175) / 0.113 (233) | 0.11 (182) / 0.165 (219) | 0.9% (185) / 11.0% (234) | 0.9% (164) / 10.5% (234) | 0.00246 (185) / 0.00278 (234) | 0.00319 (211) / 0.583 (223) |
| default-dt5 | 0.412 (215) / 1.03 (232) | 0.577 (179) / 1.01 (232) | 8.2% (207) / 13.6% (236) | 6.9% (216) / 7.9% (239) | 0.0122 (169) / 0.0122 (169) | 0.0418 (216) / 0.392 (219) |

ResSim's months 1–217 against the references (`spe5_compare.py RESSIM_MONTHS REFERENCE`, worst
difference):

| Reference | mean p (bar) | producer oil | GOR | water cut |
|---|---|---|---|---|
| default, report step (committed) | 0.86 | 11.7% | 10.5% | 0.018 |
| default, 5 d cap | 1.6 | 27.8% | 23.5% | 0.044 |
| default, 0.5 d cap | 1.96 | 33.1% | 28.6% | 0.055 |
| 1e-4 m³, 0.5 d cap | 2.03 | 32.6% | 28.1% | 0.056 |

What this shows:
- **At a 0.5 d cap the two node volumes nearly agree through month 217:** 0.074 bar, 0.9% in oil
  and GOR, 0.0025 in water cut, against 1.28 bar and 19% at the report step. Most of the
  report-step volume dependence goes away with the smaller timestep, so the volume acts largely
  through the accepted timestep path. The remaining 0.07 bar / 0.9% is the combined effect, at that
  cap, of storage and the well rows' convergence scaling. The comparison does not separate the
  two. After the crossing (months 218–240) the two still differ: 11% in oil at month 234, and up to
  0.58 in cell-33 Sg at month 223. The volume dependence there is **unresolved**.
- **The reference is not converged in time.** At the default volume, the 5 d run is 0.41 bar and
  8.2% in oil away from the 0.5 d run, and the report-step run 1.5 bar and 24%. Whether 0.5 d is
  itself converged was not tested.
- **ResSim's agreement with the committed reference is matched-step agreement.** ResSim also takes
  report-sized steps, and it agrees best with the report-step Jutul runs. It differs by 33% in oil
  from either 0.5 d run. That difference is **not a ResSim-specific error**: ResSim at the same
  caps agrees with Jutul to 1.0% in oil, and its own report-step error is like Jutul's (see
  "Retry budget and ResSim's timestep convergence").

The default-volume reference stays **provisional**. These results do not justify promoting it.
Neither report-step run is a time-converged reference, and a canonical SPE5 reference should come
from a timestep-refinement ladder, at a cap where the volume no longer matters through month 217.

#### Revised proposal (experimental, not approved)

The original rule is withdrawn as written. It targeted signature A, where acceptance would book a
fixed imbalance, and it misses signature B, the actual failure. Three separate candidates
replace it, each **disabled by default** and enabled only for the SPE5 experiment through a generic
compositional solver option (no case or scenario names in the engine):

1. **Label hysteresis within the Gibbs margin (root cause of A).** During one Newton solve, keep a
   cell's previous phase label while the competing labelling is within the existing `1e-10 RT`
   Gibbs margin. This removes the inventory jump instead of accepting it. It is a labelling change,
   not an acceptance change, but it can move answers and goes through the same evaluation.
2. **Floor acceptance (B), only if B's floor proves to be noise.** Accept a Newton solve that has
   not met `1e-8` only when all of
   these hold on the accepted iterate:
   - no cell changed phase label in the last `k` iterations (candidate `k = 4`);
   - ResSim scaled residual ≤ `τ_floor` (candidate `1e-7`) and has not decreased over those
     iterations;
   - every well row meets `1e-8`;
   - per step and per component, signed global imbalance ≤ `1e-12` of inventory and absolute
     local defect ≤ `1e-8` of inventory. Over B's last 17 iterates, signed was ≤ 1.5e-13 and
     absolute 1.5e-9 to 6.6e-9 (18–79 mol C1). A's plateau has absolute 1.8e-8 to 4.5e-8, so the
     absolute budget is what separates the two, and only by a factor of about 1.5;
   - the run's **cumulative** ledgers stay within budget: Σ signed ≤ `1e-10` and Σ absolute ≤
     `1e-7` of initial inventory per component. An acceptance that would exceed either budget is
     refused, and the step falls back to the normal halving or failure.

   Every such acceptance is reported, with its ledger entry, in the step report. The ledgers are
   kept apart from well injection and production cumulatives and are never added to them.
3. **Well warm start (A, small dt).** Start Newton from the last accepted BHP instead of the
   control's guess, so a well update cannot push a near-critical cell across the switch on an
   otherwise converged step.

Order of work, as agreed in the #116 review:
1. **Investigate B first, without changing acceptance.** Done (provisional), see "Failure B: what
   sets the floor". The cause is the near-critical split's indeterminacy in Float64 reaching the
   fluxes. It is not the linear solve or the flash tolerance, and it scales with dt.
2. **BHP warm start (3) as a separate experiment**, with `1e-8` unchanged. Done: not promoted.
3. Only after that evidence, evaluate hysteresis (1) or bounded acceptance (2). Candidate 2's
   premise has changed: B's floor is the Float64 indeterminacy of the near-critical split, at
   every cell-33 transition reached so far. A larger retry budget does not carry B through. A
   smooth flash split and bounded timestep memory are now candidates alongside them.
4. **Flash refinement and timestep memory, tried (provisional).** The difference-form refinement
   removes B's floor but carries no run through a crossing; memory is inconclusive. Every failure
   traced since is signature A, so **label hysteresis (1) is next**, and floor acceptance (2) has
   lost its premise: no failure traced after the refinement is a floor.
5. **Before any hysteresis code (PR #128 review).** Freezing a label alone could hide an
   inconsistency, so the experiment must first define, in writing:
   - **branch selection:** which thermodynamic branch a held cell is on, what decides a switch
     (stability test and Gibbs margin), and that a converged step's labels still pass the
     unmodified stability test;
   - **consistent properties and derivatives:** densities, viscosities, mobilities and the implicit
     flash Jacobian all evaluated on the held branch, including a single-phase cell held
     two-phase (a negative-flash or metastable split) and the reverse;
   - **acceptance and restart:** `1e-8` unchanged; what happens to the held labels on a failed
     attempt, a halving and the next call; and what is recorded per step.

   At-matched-cap agreement with Jutul (about 1% through month 211) shows the two simulators
   solve alike; it does not establish time convergence or independent accuracy.

   The design is in [SPE5_LABEL_HYSTERESIS_DESIGN_2026-11-08.md](SPE5_LABEL_HYSTERESIS_DESIGN_2026-11-08.md)
   (not approved). Its M0 measurement (section 0 there; `93b97f79`, clean tree) refutes the
   tie-break and locates the mechanism at identical inputs:
   - At the near-critical cells (25/31, 33/39), the residual jumps where the stability test's
     trial sum crosses its `1e-12` margin, although a converged split with **lower** Gibbs energy
     (by 1.6e-13 to 4.7e-13 RT, resolved by Float64 and confirmed in 50 digits) exists on both
     sides. The jump is face flux (∝ dt) plus a 3.4e-7 to 5.9e-7 inventory difference (not ∝ dt).
   - At cell 16 the switch is a genuine dew point (β → 1, inventory continuous). The flux-only jump
     comes from Li's rule labelling the vapour-side single phase liquid.

   Next candidates (each needing its own design and evaluation): a split-based phase decision
   inside the margin band (N1), and dew-side single-phase identity (N2). Acceptance stays the
   original equations at `1e-8`.

   **M1 result (diagnostic).** The split was followed in 50 digits to its collapse and through the
   trivial cutoff; see [SPE5_SPLIT_COLLAPSE_M1_2026-11-09.md](SPE5_SPLIT_COLLAPSE_M1_2026-11-09.md).
   Cells 25/31 and 16 collapse at the dew point (`beta -> 1`); 25/31 keep a finite contrast
   (`sum ln^2 K = 3.1e-5`), so the cutoff is not reached, and 16 keeps 0.791. Cells 33/39 collapse
   at the bubble side, but the trivial cutoff fires first at `beta = 0.3929`, 46.8 Pa past the
   switch, while the split is still a real two-phase state (`S_v = 0.393`). With the existing
   identity/cutoff policies N1 lowers the scaled residual jump about 3-10x but leaves `1e-6` to
   `1e-5` and 6.5-77% component-flux changes. That is a fixed-input branch discontinuity, not a
   failed Newton solve with N1 enabled: it does not establish that N1 cannot advance a replay.
   The dew-side remainder is N2's identity question; the memory cells need a validated cutoff
   decision, not a lowered threshold. N1 remains a useful local-correction candidate; neither N1
   nor N2 is approved.

   **N1 result (experimental, test-only; not promoted).** N1 attempts the trial-pair split when a
   stable verdict is within `1e-9` of the margin and one trial is non-trivial, accepting only a
   physical, non-trivial, lower-Gibbs answer. Eligibility and ranking use only non-trivial trial
   sums; a trivial sum of one cannot qualify an out-of-band companion. It changes 21/80 M0 phase
   decisions and 10/18 M1 start/cutoff/collapse states (the memory cutoff/collapse have only trivial
   trial seeds and do not change). The **unrefined** from-scratch replay records 12 N1 flash
   overrides but is identical to N1 off (`6612.703445986848 d`, residual `6.393474812685191e-8`);
   this is the known failure-B precision floor and does not identify the combined blocker.

   With difference refinement, the captured memory-seed solve converges in 4 iterations at
   `3.770079e-13` with N1, versus 20 iterations at `1.459579e-6` without it at matched inventory;
   failure B's last rung converges in 4 at `4.087646e-10` in either N1 mode. A bounded continuation
   from failure B with refinement+N1 advances two accepted steps to `6612.836114153414 d`, then
   fails month 218 at residual `5.0410785730772645e-3` with the same cells 33/39 flipping every
   iteration (`3.328e-3`/`5.041e-3` two-cycle). N1 is a local correction; the next blocker is still
   the phase-label discontinuity. Details in
   [SPE5_SPLIT_PHASE_DECISION_N1_2026-11-09.md](SPE5_SPLIT_PHASE_DECISION_N1_2026-11-09.md).

   **Next-blocker diagnosis.** The captured failing call alternates between a genuine two-phase
   state at `227.5516 bar` and a single-liquid state at `228.1566 bar`, but the intervening N1
   switch is premature. Bisecting the actual 19→20 path under the same N1+refinement operator puts
   the switch at `228.12668313699209/1 bar`; the upper input still has a physical split
   (`beta = 0.458178303`, `sum ln^2 K = 1.645e-5`, `g_split - g_feed = -3.144e-15 RT`). At that
   upper input both Michelsen trials are trivial, so N1 loses its eligible seed; a carried seed from
   the lower side converges to `beta = 0.458176`, `sum ln^2 K = 1.64531e-5`, above the `1e-5`
   acceptance cutoff. The assembled residual jump is `3.3069e-3` (cell 33) / `3.2777e-3` (cell 39).
   The far liquid endpoint is correct but does not exonerate the premature switch. The immediate
   next change is seed eligibility/continuation through the switch; an early two-cycle
   detector/cutback should be evaluated separately for cost and sustained advancement. The Taylor
   probe now uses the same N1+refinement settings as the attempt. Details in
   [SPE5_N1_NEXT_BLOCKER_DIAGNOSIS_2026-11-09.md](SPE5_N1_NEXT_BLOCKER_DIAGNOSIS_2026-11-09.md).

   **Carried-K seed recovery (experimental, test-only).** A per-cell previous-flash `K` is
   re-solved at the current inputs as a direct split seed when the stability verdict is stable,
   retaining physicality, equilibrium, non-triviality and Gibbs checks. At the adjacent switch
   inputs it recovers `TwoPhase` (`beta 0.436209` same-z test; actual carried seed `beta 0.458176`,
   `sum ln^2 K 1.64531e-5`), while the far liquid endpoint remains `SingleLiquid` (its candidate is
   trivial, `4.3e-13`). Repeated upper flashes return the same `beta`/`K`, a cache reset starts
   cold, three whole assemblies at identical inputs give the same residual while cold re-evaluation
   differs by `4.94e-4`, and checkpoint restart reproduces the accepted inventory. Acceptance requires the cold
   re-evaluation to be independently converged and the scaled residual-vector difference to be
   `<= 1e-8`; the max-norm shortcut is not sufficient. Active well-rate equations are matched by
   well, not position; a warm/cold control change rejects the comparison (committed cell-33 probe:
   warm limit vector `[true,false,false,true]` / 1 elimination vs cold
   `[true,false,false,false]` / 2). The full failing call is unchanged because its step jumps from `227.551` to
   `228.156 bar` and never evaluates the switch; recovery plus fixed damping 0.5 fails with zero
   accepted steps. The next change should pair carried `K` with a line search or short continuation
   that evaluates intermediate inputs. Details in
   [SPE5_SEED_RECOVERY_EXPERIMENT_2026-11-09.md](SPE5_SEED_RECOVERY_EXPERIMENT_2026-11-09.md).

All thresholds above are **experimental candidates**, not validated values. They come from two
traces of one case, and the Jutul fixture's per-report numbers cover only the final ministep of
each report.

Before any of these is proposed for approval, the evaluation must include:
- the from-scratch run to month 240;
- the cumulative ledgers at the end of that run;
- sensitivity of the outcome to `τ_floor` (3e-8, 1e-7, 3e-7), to the timestep ladder, and to the
  flash tolerance (does B's floor track it?);
- unchanged controls, i.e. the compositional bucket and the SPE3 replays bit-identical with the
  option off;
- the #98 shut-in probe.

## Reference plan

1. **Done.** The JutulDarcy reference is committed at `opm/compositional/spe5/reference.json` with
  `manifest.json` and `run-spe5.sh`; see “Independent JutulDarcy reference” above. The earlier
  156-report run is superseded: it ran before the OPM Stone II, rock-compressibility, surface-rate
  and surface-condition corrections, its producer was BHP-controlled from 730.5 d at an oil rate of
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
