# SPE5 Case 1 on the compositional engine (#52)

This document owns the live SPE5 Case 1 scenario. The fluid, water phase, structured grid, rate
control and WAG schedule are the enablers recorded in `COMPOSITIONAL_VALIDATION.md` §8 and
`SPE3_VALIDATION.md` §“Gas cycling decision”; this page records what the reservoir case is, how it
differs from the published black-oil + solvent deck, and what still blocks offering it.

**Status: authored, not shipped.** `src/lib/catalog/scenarios/comp_spe5_wag.ts` builds the complete
payload and is withheld from the picker. Two things are outstanding:

1. The 20-year replay stalls in Newton at **6114.808 d (month 201, year 17)**. See
   [#93](https://github.com/sergeyfarin/ressim/issues/93) for the reproducer and what was ruled out.
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
| Rock | φ 0.3; kx/ky 500/50/200 mD; kz 50/50/25 mD | `PORO`, `PERMX/Y/Z` |
| Fluid | OPM `Spe5FluidSystem`, six hydrocarbons, PRCORR | `docs/COMPOSITIONAL_VALIDATION.md` §1 |
| Temperature | 160 °F | Killough & Kossack (1987) |
| Initial composition | C1 0.50, C3 0.03, C6 0.07, C10 0.20, C15 0.15, C20 0.05 | same paper |
| Injection gas | C1 0.77, C3 0.20, C6 0.03 | same paper |
| Water | connate 0.20; PVTW 14.7 psia / Bw 1.0 / cw 3.3e-6 psi⁻¹ / 0.7 cP | `PVTW` |
| Relperm | deck `SWFN`/`SGFN`/`SOF3` sampled onto Stone II `SWOF`/`SGOF`; zero Pc | `SWFN`, `SGFN`, `SOF3` |
| Producer | 12000 STB/d oil, 1000 psia BHP floor | `WCONPROD` |
| Water injector | 12000 STB/d, 10000 psia BHP ceiling | `WCONINJE` |
| Gas injector | 12000 Mscf/d, 10000 psia BHP ceiling | `WCONINJE` |
| Schedule | 2 years production, then nine one-year water/gas pairs to year 20 | `SPE5CASE1.DATA` |

Well indices are Peaceman from the deck’s 0.5 ft diameter and each layer’s `k·dz`; the deck’s
`Kh = 10000 mD·ft` is exactly `500×20 = 50×30 = 200×50`, so the layers share one geometric index.

## What this case is not

The published SPE5 comparison is a **black-oil + solvent** run. ResSim has no solvent option, so
the scenario runs the same six components through the Peng–Robinson compositional engine. That is
the case the #52 plan calls for, but it is not a reproduction of the 1987 black-oil curves:

- **Capillary pressure is zero.** The compositional model has no hydrocarbon capillary pressure;
  the deck’s water–oil column is small but not zero.
- **Surface separation is one equilibrium stage** at 1 atm / 288.71 K (C5’s compositional default),
  not the deck’s black-oil PVT table. The deck’s stock-tank rate targets are converted through it.
- **`SOF3` is interpreted by Stone II.** The deck’s table is keyed on oil saturation; the scenario
  samples it at the deck’s own saturation nodes and sets `Pc = 0`.
- **The producer is oil-rate controlled, not liquid-rate.** `SurfacePhase::Liquid` in the
  compositional engine is the stock-tank hydrocarbon liquid, which is the deck’s `ORAT`.

These are recorded so a future comparison does not attribute a difference to the solver when it is
a model-definition difference.

## Reference plan

1. Generate the JutulDarcy compositional reference from the same `case.json` (grid, EOS, BIC,
  Stone II curves, rate targets, WAG schedule) and commit it under
  `opm/compositional/spe5/reference.json` with a manifest, as the SPE3 reference does.
2. Put the run through `compare_small_direct.py --comp-reference-dir` and the shared scorecard,
  including water saturation (the existing compositional comparator compares `p`, `sg` and `z`; it
  needs the `sw` column the SPE5 fixture carries).
3. State the oil-rate, GOR and average-pressure bands against both JutulDarcy and the published
   Killough–Kossack curves. Digitize the published curves under the SPE1 precedent
   (`publishedReferenceSeries`); do not hand-copy a number from prose.
4. Only then move `comp_spe5_wag` out of `WITHHELD_SCENARIOS`.

## Reproducer and gates

```bash
# Payload admissibility; part of the compositional gate.
cargo test --manifest-path src/lib/ressim/Cargo.toml \
  comp_spe5_case_payload_is_admissible -- --nocapture

# First WAG cycle (production, water year, gas year); release and --ignored.
cargo test --release --manifest-path src/lib/ressim/Cargo.toml \
  comp_spe5_case_accepts_its_first_water_and_gas_slugs -- --ignored --nocapture

# Everything up to the late-life stall; release and --ignored because it is minutes of work.
cargo test --release --manifest-path src/lib/ressim/Cargo.toml \
  comp_spe5_case_reaches_the_late_life_checkpoint -- --ignored --nocapture

# The stall itself (#93). The seed is the accepted state at month 192.
# opm/compositional/spe5/stall_192.json
```

No black-oil or compositional tolerance is changed by this case.
