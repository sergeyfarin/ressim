# Single-cell depletion reference case

C12's **second** fixture. The compositional execution plan asks for several matched fixtures and
names *phase-changing single-cell depletion* among them; `1D_COMP` is a displacement and nothing in
it takes a cell down through its own saturation pressure.

| | |
| --- | --- |
| Deck | `DEPLETION.DATA`, derived from [`OPM/opm-tests`](https://github.com/OPM/opm-tests) `compositional/1D_COMP.DATA` — the PROPS section is that deck's verbatim, © 2024 SINTEF Digital / TNO, Open Database License |
| Simulator | `flowexp_comp`, the same build as the 1D case |
| Regenerate | `bash tools/opm_compositional/run-depletion.sh` |
| Verify unchanged | `bash tools/opm_compositional/run-depletion.sh --check` |
| Consumed by | `comp_depletion_*` in `src/lib/ressim/src/compositional/reference_tests.rs` |

## The case

One cell, 100 m × 100 m × 10 m, 100 mD, 10% porosity — a 10 000 m³ pore volume. Initially
single-phase liquid at 150 bar and 150 °C with `z = [0.1, 0.3, 0.6]`, the same mixture as the 1D
case, which saturates between 100 and 125 bar at this temperature. One producer on **rate** control:
30 sm³/day of surface oil, with a 20 bar BHP floor that never binds.

Rate control rather than BHP control is the point. With the withdrawal imposed identically on both
sides, the comparison is of the pressure path and the phase-appearance point, and it does not
inherit the conditioning that makes a near-shut-in well's cumulative a small difference of large
numbers — the thing that keeps `1D_COMP` from meeting the plan's 1% cumulative target.

## The reference run aborts, and that is the finding

`flowexp_comp` depletes the cell cleanly to 111.55 bar single phase, produces **one** report step at
109.10 bar with gas just appeared (`Sg = 0.0036`), and then its own Rachford–Rice stops converging
and the run is abandoned. Every two-phase flash method it offers — `ssi`, `newton`, `ssi-newton` —
fails the same way, and halving the production rate only moves where.

So the fixture is seven report steps rather than twenty, and `run-depletion.sh` checks the **step
count** rather than the exit status: an expected abort must not be indistinguishable from a real
failure. Those seven steps cover the whole single-phase depletion path and the phase appearance
itself, which is what this fixture is for.

ResSim runs the same case past that point without difficulty — its stability test and extended
(negative) Rachford–Rice window exist precisely for near-boundary states.

## What it establishes, and what it opens

| | |
| --- | --- |
| Surface gas/oil **ratio** | agrees to **1.4e-8** — the reference's single-precision floor. The first head-to-head comparison of the two surface flashes on a mixture that genuinely splits |
| Phase appearance | agrees at all seven steps: single phase through 111.55 bar, two-phase at 109.10 |
| Surface volume **scale** | does **not** agree: ResSim needs 4.0% more feed per sm³ of surface oil. Measured by material balance on the reference's own pressures, which needs no assumption about how it meters anything. **Open** — see the C12 record in [`docs/COMPOSITIONAL_VALIDATION.md`](../../../docs/COMPOSITIONAL_VALIDATION.md) |
| Pressure path | separates by 0.23 bar/day, which is that 4% integrated. Timestep-independent to 0.001 bar over a 16-fold change in sub-step |

The scale discrepancy is specific to a **mixture**. For the pure CO2 stream of the 1D case the two
simulators' surface volumes agree to about 0.01%, which is why that case's cumulative-injection
comparison is not affected by it.
