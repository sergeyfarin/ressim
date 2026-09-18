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

## Two decks

`DEPLETION.DATA` is on a surface **oil rate**; `bhp/DEPLETION.DATA` is the same cell against an
**80 bar BHP**. The second exists because the first turned out to be unusable as a trajectory
oracle — `flowexp_comp` meters 30 sm³/day of surface oil as 3.8% fewer moles than its own flash
says that stream is, so a rate-controlled comparison inherits a discrepancy that is the reference's
and not the model's. Nothing in the BHP deck goes through a surface volume.

They also cover different things. The rate-controlled deck brackets the **phase appearance** (and
then the reference aborts); the BHP deck runs the full twenty days two-phase and is where the
**composition drift** and the **connection rate** are compared.

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

## What the BHP deck found

**C12's strongest model-equivalence result** — after a retraction.

The reference takes **one backward-Euler step per report interval** (20 timesteps for 20 report
steps). Run ResSim at the same resolution and the two agree to **0.0074 bar over twenty days**, the
gas saturation to 1.8e-4 and the overall composition to 7.5e-5. Refining ResSim moves it *away*
from the reference, which is what should happen when one side is converged and the other is not;
forcing the reference to sub-step walks it to the same place (`p(1)`: 99.2390 at 1 d → 95.3159 at
0.05 d → 95.0018 at 0.01 d, against ResSim's 95.3200 and 94.9493).

An earlier revision reported a **1.29× producer connection-rate defect** here. There is none: the
1.29 was the cost of comparing an instantaneous rate against a backward-Euler step, whose implicit
rate is the one at the *end* of the step. Evaluated there, the ratio is 1.0000–1.0014.

The full account is in
[`docs/COMPOSITIONAL_C12_FORENSICS.md`](../../../docs/COMPOSITIONAL_C12_FORENSICS.md), including
the term-by-term register — every row now settled against opm-common's own code or output — so the
ruled-out list is not re-tested.

## What the rate deck establishes, and what it opens

| | |
| --- | --- |
| Surface gas/oil **ratio** | agrees to **1.4e-8** — the reference's single-precision floor. The first head-to-head comparison of the two surface flashes on a mixture that genuinely splits |
| Phase appearance | agrees at all seven steps: single phase through 111.55 bar, two-phase at 109.10 |
| Surface volume **scale** | ResSim 236 926 mol/day for 30 sm³/day of surface oil; OPM's own PTFlash at `STCOND` says 236 582 — **0.15%**, the two fluid systems' critical constants |
| The reference's rate control | **Does not converge.** Implied withdrawal 227 888 → 231 373 → 244 887 → 269 638 mol/day as its `TSTEP` is refined 1.0 → 0.05 → 0.01 → 0.0025 d, walking past ResSim's timestep-independent value without settling. **Open, and not ResSim's** |
| Pressure path | separates by 0.23 bar/day. ResSim's is timestep-independent to 0.001 bar over a 16-fold change in sub-step, so there is nothing on ResSim's side to match |

## What the ORAT pressure paths actually caught

They separate, and material balance says why — but the answer is not on ResSim's side.

While the cell is single phase its composition cannot change, so the moles it holds are `PV · c(p)`,
and both implementations agree on `c` to 0.12% along this very path (checked against OPM's own
`ParameterCache::molarVolume`). So the reference really withdrew about 227 900 mol/day for what it
reported as 30 sm³/day of surface oil. Asking OPM's **own** PTFlash what 30 sm³/day of surface oil
is — `ternary_stcond_depletion`, added to the C0 fixture for exactly this — gives 236 582 mol/day.

Two artefacts of the same simulator disagree, and ResSim agrees with the one that is a flash. **And
the simulator's value is not a constant:** refine its `TSTEP` and the implied withdrawal walks past
ResSim's and keeps going, so there is no offset to calibrate out. The 1D case is unaffected — its
injected stream is pure CO2, for which the two agree to about 0.01%, and its wells are
BHP-controlled.

`comp_depletion_reference_withdrawal_disagrees_with_its_own_flash` is where all three numbers are
measured side by side, and it is bounded in both directions so that reconciling them fails rather
than passes silently.
