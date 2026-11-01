# SPE5 Case 1 on the compositional engine (#52)

This document owns the live SPE5 Case 1 scenario. The fluid, water phase, structured grid, rate
control and WAG schedule are the enablers recorded in `COMPOSITIONAL_VALIDATION.md` §8 and
`SPE3_VALIDATION.md` §“Gas cycling decision”; this page records what the reservoir case is, how it
differs from the OPM deck implementation, and what still blocks offering it.

**Status: authored, not shipped.** `src/lib/catalog/scenarios/comp_spe5_wag.ts` builds the complete
payload and is withheld from the picker. Two things are outstanding:

1. The full 20-year replay still stops in a later near-critical region around **6375 d (month
   209)**, where successive substitution needs more than 20,000 iterations and the Newton solve then
   cuts the timestep. This is separate from #93 and is tracked as
   [#99](https://github.com/sergeyfarin/ressim/issues/99).
2. The independent JutulDarcy compositional reference is not committed yet. No UI overlay or
   scorecard row exists for this case.

[#93](https://github.com/sergeyfarin/ressim/issues/93) itself — the **6114.808 d** stall — was a
near-critical stability-tolerance defect in the flash, fixed below. The month-201 segment now
clears, but the scenario is still not presented as validated, and no result from it should be
quoted until the later region and the reference land.

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

### Recorded deviation: shut-in criteria

The paper specifies stopping at **GOR 10 Mscf/STB** or **WOR 5 STB/STB**. The payload always
requests the full 240 reports. The compositional run loop has no surface-rate history (#95) and does
not evaluate termination policies (#98), so neither criterion is enforced yet. This must be fixed
before a published-trajectory comparison or an explicit stopping rule is claimed.

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

The month-192 → month-201 segment now clears in 56 calls and 117 s. A later near-critical region
around **6375 d (month 209)** is a separate gap: the flash's successive substitution needs about
20,700 iterations even with a 100,000 cap, and the Newton solve then needs six timestep cuts to
advance 0.156 d. That is a flash-acceleration and nonlinearity problem, not the stability verdict
fixed here; it is tracked as [#99](https://github.com/sergeyfarin/ressim/issues/99). The 20-year
replay therefore does not complete yet and the scenario stays withheld.

Evidence:

```bash
# The exact failing call, from the committed pre-fix state: one attempt, 5 Newton iterations.
cargo test --release --manifest-path src/lib/ressim/Cargo.toml \
  comp_spe5_resume_192_first_stalled_call_converges -- --ignored --nocapture

# The month-192 to month-201 segment that used to stall: 56 calls, 117 s.
cargo test --release --manifest-path src/lib/ressim/Cargo.toml \
  comp_spe5_resume_192_clears_the_201_report -- --ignored --nocapture
```

The 16-year from-scratch replay (`comp_spe5_case_reaches_the_late_life_checkpoint`) also passes
with the fix, in about 220 s. The unit regressions are part of the thermodynamic gate:
`comp_stability_detects_a_near_critical_tangent_plane` pins the verdict on both sides of the true
bubble point, and `comp_flash_keeps_a_near_critical_split_continuous` pins the continuous split and
the no-jump molar volume across the old switch.

## Reference plan

1. Generate the JutulDarcy compositional reference from the same `case.json` (grid, EOS, BIC,
  Stone II curves, rate targets, WAG schedule) and commit it under
  `opm/compositional/spe5/reference.json` with a manifest, as the SPE3 reference does.
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
