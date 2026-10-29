# 2-D quarter-five-spot compositional reference case

The **structured-grid trajectory oracle** for #47: a five-by-five areal CO2 flood, run through OPM's
`flowexp_comp`. It is a derivative of [`../1d_comp/1D_COMP.DATA`](../1d_comp/1D_COMP.DATA) — the same
CO2 / methane / decane fluid, the same `SGOF` table and the same initial mixture — with the geometry
and well placement changed so that an areal transmissibility and a two-direction Peaceman radius are
part of the comparison.

| | |
| --- | --- |
| Deck | `QUARTER_FIVE_SPOT.DATA`, derived from OPM's `compositional/1D_COMP.DATA`, © 2024 SINTEF Digital / TNO, Open Database License (notice retained in the file) |
| Simulator | `flowexp_comp`, the same build as the other compositional cases ([`build-flowexp-comp.sh`](../../../tools/opm_compositional/build-flowexp-comp.sh)) |
| Build environment | Ubuntu 26.04 / g++ 15.2 / OPM `2026.04-1~resolute` since the 2026-10-29 re-baseline; see [`../README.md`](../README.md#build-environment) |
| Regenerate | `bash tools/opm_compositional/run-quarter-five-spot.sh` |
| Verify unchanged | the same script with `--check` |
| Consumed by | `comp_reference_quarter_five_spot_matches_opm` in `src/lib/ressim/src/compositional/reference_tests.rs` |

## The case

`DIMENS 5 5 1`, 20 m × 20 m × 10 m cells, 100 mD isotropic, 10% porosity. Initially 75 bar at
150 °C with `z = [0.1, 0.3, 0.6]`. The injector is in `(1,1,1)`, the producer in `(5,5,1)`, and
`COMPDAT` uses the deck's 0.0151 m wellbore **diameter** — item 9, not a radius — on both.

The injector runs at 100 000 sm³/day against a 150 bar limit and never reaches the limit; the
producer holds 50 bar. Ten report intervals of 0.1 d follow one pre-well interval of 0.001 d, so the
fixture ends at 1.001 d.

## Three places this deck differs from `1D_COMP.DATA`

1. **The grid is areal**, and the producer moved from cell 5 to cell 25. That is the point of the
   case: the 1D reference cannot see a y-face, a diagonal displacement or a well index that depends
   on both lateral cell widths.
2. **The injector is rate-controlled at the wellhead and the EOS is ResSim's pinned ternary.**
   The deck's `ACF`, `PCRIT`, `TCRIT`, `MW` and `VCRIT` are written as the pinned values, which are
   OPM's `ThreeComponentFluidSystem` at the precision `src/lib/ressim/src/fluid/pinned.rs`
   records. `flowexp_comp` reads EOS data from the deck, so this makes the reference and ResSim
   solve the same fluid rather than two roundings of it.
3. **A 0.001 d pre-well `TSTEP` is retained.** It is not physics. In this build of
   `flowexp_comp`, a schedule whose first report step already has the wells open writes the
   *initial* restart state for every report step while its summary advances, so the per-cell fixture
   would never move. One report interval before `WELSPECS` fixes the output alignment.

## Surface-rate control and the comparison

`WCONINJE` is a surface **gas** rate. ResSim's public payload has no surface-rate control for a
pinned fluid — the pinned specifications carry no `STCOND` — so the test drives the injector with
the molar rate that 100 000 sm³/day represents at the deck's `STCOND 15.0 1.0`, computed from
ResSim's own flash of pure CO2 at 1 bar and 15 °C. Both sides therefore prescribe the same moles.
The test checks the cumulative against the reference's own `FGIT` rather than assuming the surface
conversion: it currently agrees to `5e-12` relative.

## `reference.json`

Per cell per report step: pressure, `SGAS`, `SOIL`, and the liquid, vapour and overall mole
fractions (`XMF`, `YMF`, `ZMF`), plus the field injection and production summaries. As in the other
`flowexp_comp` fixtures, `OIL_DEN`, `GAS_DEN`, `OIL_VISC`, `GAS_VISC`, `WBHP` and all block summary
vectors are written as zeros and are not usable as reference values; the deck does not request the
zero well vectors and the extractor's note records the rest.

## What the comparison measures

At matched 0.1 d steps over the ten flowing intervals, the worst differences recorded by the test
are **0.007 bar** on pressure, **2.8e-5** on gas saturation and **4.2e-5** on the CO2 overall mole
fraction, with the cumulative injection at the reference's own value. Those are measured numbers
from this deck, not a widened target; the test's bands are set just above them.
