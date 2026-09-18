# 1D compositional reference case

The **trajectory oracle** for the compositional plan's C12: OPM's `flowexp_comp` running a
five-cell 1D CO2 flood in CO2 / methane / decane — ResSim's own V1 fluid.

| | |
| --- | --- |
| Deck | `1D_COMP.DATA`, from [`OPM/opm-tests`](https://github.com/OPM/opm-tests) `compositional/`, © 2024 SINTEF Digital / TNO, Open Database License (notice retained in the file) |
| Simulator | `flowexp_comp`, built by [`tools/opm_compositional/build-flowexp-comp.sh`](../../../tools/opm_compositional/build-flowexp-comp.sh) from `OPM/opm-simulators` `release/2026.04/final` (`b82f21d`) |
| Regenerate | `bash tools/opm_compositional/run-1d-comp.sh`, and `run-refinement.sh` for `refined/` |
| Verify unchanged | the same two scripts with `--check` |
| Consumed by | `comp_reference_*` and `comp_refinement_*` in `src/lib/ressim/src/compositional/reference_tests.rs` |

## The case

Five cells, 60 m × 6 m × 6 m, 100 mD, 10% porosity, horizontal. Initially 75 bar at 150 °C with
`z = [0.1, 0.3, 0.6]`. Pure CO2 is injected into cell 1 against a 150 bar BHP limit; cell 5
produces at 50 bar. 28 report steps over about 20 days, ending with CO2 essentially everywhere.

Two details of the deck are easy to read wrong, and both cost C12 real measurement error before
they were found. The wells are declared **after** four `TSTEP`s, so nothing flows for the first
0.11 days and the first four report steps are the initial state unchanged. And `COMPDAT`'s
`0.0151` sits in item 9, which is the wellbore **diameter** — reading it as a radius makes the
well index about 12% too large, by an amount that does not divide out with cell size.

## `refined/`

C12 asks for a comparison against *a refined external solution, not merely a coarse Flow output*,
so the same deck is re-run at 10, 20 and 40 cells over the same 300 m —
`refined/n010/`, `n020/`, `n040/`. Only the discretisation changes:
[`refine_deck.py`](../../../tools/opm_compositional/refine_deck.py) rewrites the cell-count-dependent
keywords and **asserts on every one of them**, so a change to `1D_COMP.DATA` fails there rather
than silently producing a deck that is no longer the same case.

## `reference.json`

Per cell per report step: pressure, `SGAS`, `SOIL`, and the liquid, vapour and overall mole
fractions (`XMF`, `YMF`, `ZMF`). Plus well and field summary vectors.

**Three observables in it are not usable as reference values.** `OIL_DEN`, `GAS_DEN`, `OIL_VISC`
and `GAS_VISC` are requested by the deck's `RPTRST` but `flowexp_comp` writes them as zeros; so are
`FPR` and every block summary vector. The extractor prints a note for any vector that comes back
identically zero, so a column of zeros is never mistaken for agreement.

## Three places this deck is not ResSim's pinned fluid

C12 builds its specification from **the deck's** numbers, because the deck is what the reference
ran. Using the pinned ones would compare two different problems.

1. **The EOS data is more precise here** than OPM's hard-coded `ThreeComponentFluidSystem` — e.g.
   `ACF = [0.22394, 0.01142, 0.4884]` against `[0.224, 0.011, 0.488]`. Critical volumes and the
   all-zero interaction matrix do match.
2. **The deck supplies its own relative permeability** — `SGOF`, a Corey-squared table keyed on gas
   saturation.
3. **The deck supplies its own surface conditions** — `STCOND 15.0 1.0`, not the 288.71 K / 1 atm
   pinned in `COMPOSITIONAL_VALIDATION.md` §6.
