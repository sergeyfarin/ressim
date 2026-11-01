# SPE5 Case 1 on the compositional engine (#52)

This document owns the live SPE5 Case 1 scenario. The fluid, water phase, structured grid, rate
control and WAG schedule are the enablers recorded in `COMPOSITIONAL_VALIDATION.md` §8 and
`SPE3_VALIDATION.md` §“Gas cycling decision”; this page records what the reservoir case is, how it
differs from the OPM deck implementation, and what still blocks offering it.

**Status: authored, not shipped.** `src/lib/catalog/scenarios/comp_spe5_wag.ts` builds the complete
payload and is withheld from the picker. Two things are outstanding:

1. The 20-year replay stalls in Newton at **6114.808 d (month 201, year 17)**. See
   [#93](https://github.com/sergeyfarin/ressim/issues/93) for the reproducer.
2. The independent JutulDarcy compositional reference is not committed yet. No UI overlay or
   scorecard row exists for this case.

The scenario is not presented as validated, and no result from it should be quoted until both land.

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

# Everything up to the late-life stall; release and --ignored because it is minutes of work.
cargo test --release --manifest-path src/lib/ressim/Cargo.toml \
  comp_spe5_case_reaches_the_late_life_checkpoint -- --ignored --nocapture

# The stall itself (#93). The committed seed is the accepted state at month 192.
cargo test --release --manifest-path src/lib/ressim/Cargo.toml \
  comp_spe5_resume_192_reproduces_the_stall -- --ignored --nocapture
```

The resumed reproducer fails at the same 6114.808 d with `Budget` over `Nonlinear: Newton did not
converge in 20 iterations (scaled residual 1.04e-3)`. Smaller timesteps and sparse LU reproduce it.
Replacing the producer control with its BHP limit reproduces it. Raising the Newton budget to 50
was started, did not finish the segment within ten minutes, and was stopped: that experiment is
**inconclusive**, not evidence that the iteration budget is irrelevant.

No black-oil or compositional tolerance is changed by this case.
